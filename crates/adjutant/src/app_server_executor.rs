use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::spawn_blocking;

use agent_client_protocol_schema as acp;

use crate::autopilot_loop::AcpEventSender;
use crate::cli::prompt::{expand_prompt_text_async, format_command_output, truncate_bytes, MAX_COMMAND_BYTES};
use crate::{AdjutantError, Task, TaskResult};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
enum AppServerRequestId {
    String(String),
    Integer(i64),
}

struct AppServerNotification {
    method: String,
    params: Option<Value>,
}

struct AppServerRequest {
    id: AppServerRequestId,
    method: String,
    params: Option<Value>,
}

struct AppServerChannels {
    notifications: mpsc::Receiver<AppServerNotification>,
    requests: mpsc::Receiver<AppServerRequest>,
}

struct AppServerTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    pending_requests: Arc<Mutex<HashMap<AppServerRequestId, oneshot::Sender<Result<Value>>>>>,
    request_counter: AtomicI64,
    reader_task: Option<tokio::task::JoinHandle<()>>,
}

impl AppServerTransport {
    fn new(stdin: ChildStdin, stdout: ChildStdout) -> (Self, AppServerChannels) {
        let stdin = Arc::new(Mutex::new(stdin));
        let pending_requests: Arc<Mutex<HashMap<AppServerRequestId, oneshot::Sender<Result<Value>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (notification_tx, notification_rx) = mpsc::channel(256);
        let (request_tx, request_rx) = mpsc::channel(64);

        let pending_requests_clone = pending_requests.clone();
        let notif_tx = notification_tx.clone();
        let req_tx = request_tx.clone();
        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
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
                let _ = sender.send(Err(anyhow::anyhow!(
                    "App-server connection closed"
                )));
            }
        });

        (
            Self {
                stdin,
                pending_requests,
                request_counter: AtomicI64::new(1),
                reader_task: Some(reader_task),
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

        let result = rx
            .await
            .context("App-server request canceled")??;
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

struct AppServerClient {
    transport: AppServerTransport,
    process: Child,
}

impl AppServerClient {
    async fn spawn(cwd: &Path) -> Result<(Self, AppServerChannels)> {
        let command = resolve_app_server_command()
            .context("Unable to find codex app-server executable")?;

        let mut cmd = Command::new(command.program);
        cmd.args(command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .current_dir(cwd);

        let mut child = cmd.spawn().context("Failed to spawn codex app-server")?;
        let stdin = child.stdin.take().context("codex app-server stdin missing")?;
        let stdout = child.stdout.take().context("codex app-server stdout missing")?;

        let (transport, channels) = AppServerTransport::new(stdin, stdout);

        Ok((Self { transport, process: child }, channels))
    }

    async fn initialize(&self) -> Result<()> {
        let params = InitializeParams {
            client_info: ClientInfo {
                name: "adjutant".to_string(),
                title: Some("Adjutant".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        let response = self.transport.request::<_, InitializeResponse>("initialize", Some(&params)).await;
        match response {
            Ok(_) => {
                self.transport.notify::<Value>("initialized", None).await?;
                Ok(())
            }
            Err(err) => {
                if format!("{err}").contains("Already initialized") {
                    self.transport.notify::<Value>("initialized", None).await?;
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    async fn thread_start(&self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        self.transport.request("thread/start", Some(&params)).await
    }

    async fn turn_start(&self, params: TurnStartParams) -> Result<TurnStartResponse> {
        self.transport.request("turn/start", Some(&params)).await
    }

    async fn turn_interrupt(&self, params: TurnInterruptParams) -> Result<()> {
        self.transport.notify("turn/interrupt", Some(&params)).await
    }

    async fn command_exec(&self, params: CommandExecParams) -> Result<CommandExecResponse> {
        self.transport.request("command/exec", Some(&params)).await
    }

    async fn review_start(&self, params: ReviewStartParams) -> Result<ReviewStartResponse> {
        self.transport.request("review/start", Some(&params)).await
    }

    async fn respond<T>(&self, id: AppServerRequestId, result: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.transport.respond(id, result).await
    }

    async fn shutdown(mut self) -> Result<()> {
        let _ = self.process.kill().await;
        let _ = self.process.wait().await;
        self.transport.shutdown().await
    }
}

struct AppServerCommand {
    program: PathBuf,
    args: Vec<String>,
}

fn resolve_app_server_command() -> Result<AppServerCommand> {
    if let Ok(program) = which::which("codex-app-server") {
        return Ok(AppServerCommand {
            program,
            args: Vec::new(),
        });
    }

    let program = crate::auth::get_codex_path().context("codex executable not found")?;

    Ok(AppServerCommand {
        program,
        args: vec!["app-server".to_string()],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientInfo {
    name: String,
    title: Option<String>,
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResponse {
    user_agent: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AskForApproval {
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NetworkAccess {
    #[default]
    Restricted,
    Enabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum SandboxPolicy {
    DangerFullAccess,
    ReadOnly,
    ExternalSandbox {
        #[serde(default)]
        network_access: NetworkAccess,
    },
    WorkspaceWrite {
        #[serde(default)]
        writable_roots: Vec<String>,
        #[serde(default)]
        network_access: bool,
        #[serde(default)]
        exclude_tmpdir_env_var: bool,
        #[serde(default)]
        exclude_slash_tmp: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReasoningEffort {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ThreadStartParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox: Option<SandboxMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadRef {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadStartResponse {
    thread: ThreadRef,
    model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum UserInput {
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TurnStartParams {
    thread_id: String,
    input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<ReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_policy: Option<SandboxPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnRef {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnStartResponse {
    turn: TurnRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CommandExecParams {
    command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_policy: Option<SandboxPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandExecResponse {
    exit_code: i32,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnInterruptParams {
    thread_id: String,
    turn_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ReviewTarget {
    UncommittedChanges,
    #[serde(rename_all = "camelCase")]
    BaseBranch { branch: String },
    #[serde(rename_all = "camelCase")]
    Commit {
        sha: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Custom { instructions: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewStartParams {
    thread_id: String,
    target: ReviewTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewStartResponse {
    turn: TurnRef,
    review_thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMessageDeltaNotification {
    item_id: String,
    delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReasoningDeltaNotification {
    item_id: String,
    delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandExecutionOutputDeltaNotification {
    item_id: String,
    delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChangeOutputDeltaNotification {
    item_id: String,
    delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemNotification {
    item: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnDiffUpdatedNotification {
    turn_id: String,
    diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnPlanUpdatedNotification {
    #[serde(default)]
    explanation: Option<String>,
    plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnPlanStep {
    step: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadTokenUsageUpdatedNotification {
    token_usage: ThreadTokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadTokenUsage {
    input_tokens: i32,
    cached_input_tokens: i32,
    output_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextCompactedNotification {
    thread_id: String,
    turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnCompletedNotification {
    turn: TurnCompletedInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnCompletedInfo {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    error: Option<TurnError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnError {
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorNotification {
    error: TurnError,
    will_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandExecutionRequestApprovalParams {
    item_id: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChangeRequestApprovalParams {
    item_id: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalResponse {
    decision: ApprovalDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    accept_settings: Option<ApprovalAcceptSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ApprovalDecision {
    Accept,
    Decline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalAcceptSettings {
    for_session: bool,
}

pub struct AppServerExecutor {
    workspace_root: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ApprovalMode {
    AutoAccept,
    Prompt,
    AutoDecline,
}

#[derive(Debug, Clone)]
pub(crate) struct AppServerPromptOptions {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<ReasoningEffort>,
    pub(crate) approval_policy: AskForApproval,
    pub(crate) sandbox_mode: SandboxMode,
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) approval_mode: ApprovalMode,
}

impl AppServerExecutor {
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub async fn execute(&self, task: &Task) -> Result<TaskResult, AdjutantError> {
        self.execute_internal(task, None, None).await
    }

    pub async fn execute_streaming(
        &self,
        task: &Task,
        token_tx: mpsc::UnboundedSender<String>,
        acp_sender: Option<AcpEventSender>,
    ) -> Result<TaskResult, AdjutantError> {
        self.execute_internal(task, Some(token_tx), acp_sender)
            .await
    }

    pub async fn execute_prompt_streaming(
        &self,
        prompt: &str,
        options: AppServerPromptOptions,
        acp_sender: Option<AcpEventSender>,
        interrupt_flag: Option<Arc<AtomicBool>>,
    ) -> Result<TaskResult, AdjutantError> {
        let (client, channels) =
            AppServerClient::spawn(&self.workspace_root)
                .await
                .map_err(|err| AdjutantError::ExecutionFailed(format!(
                    "Failed to spawn codex app-server: {}",
                    err
                )))?;

        let AppServerPromptOptions {
            model,
            effort,
            approval_policy,
            sandbox_mode,
            sandbox_policy,
            approval_mode,
        } = options;

        if let Err(err) = client.initialize().await {
            let _ = client.shutdown().await;
            return Err(AdjutantError::ExecutionFailed(format!(
                "Failed to initialize codex app-server: {}",
                err
            )));
        }

        let sandbox_policy_for_commands = sandbox_policy.clone();
        let client_ref = &client;
        let expanded_prompt = expand_prompt_text_async(prompt, &self.workspace_root, |command, cwd| {
            let sandbox_policy = sandbox_policy_for_commands.clone();
            let client = client_ref;
            async move {
                let command_label = command.clone();
                let response = client
                    .command_exec(CommandExecParams {
                        command: vec!["bash".to_string(), "-lc".to_string(), command],
                        cwd: Some(cwd.to_string_lossy().to_string()),
                        sandbox_policy: Some(sandbox_policy),
                        timeout_ms: None,
                    })
                    .await
                    .map_err(|err| format!("Failed to run command '{}': {}", command_label, err))?;
                let output = format_command_output(
                    response.exit_code,
                    response.stdout.as_bytes(),
                    response.stderr.as_bytes(),
                );
                Ok(truncate_bytes(output, MAX_COMMAND_BYTES))
            }
        })
        .await
        .map_err(|err| AdjutantError::ExecutionFailed(err))?;

        let thread_response = client
            .thread_start(ThreadStartParams {
                model: model.clone(),
                model_provider: None,
                cwd: Some(self.workspace_root.to_string_lossy().to_string()),
                approval_policy: Some(approval_policy),
                sandbox: Some(sandbox_mode),
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex thread: {}",
                err
            )))?;

        let thread_id = thread_response.thread.id.clone();
        let turn_response = client
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![UserInput::Text { text: expanded_prompt }],
                model: model.clone(),
                effort,
                summary: None,
                approval_policy: Some(approval_policy),
                sandbox_policy: Some(sandbox_policy),
                cwd: Some(self.workspace_root.to_string_lossy().to_string()),
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex turn: {}",
                err
            )))?;

        let turn_id = turn_response.turn.id;
        let result = run_app_server_turn_loop(
            client,
            channels,
            acp_sender,
            approval_mode,
            interrupt_flag,
            thread_id,
            turn_id,
        )
        .await;

        result
    }

    pub async fn execute_review_streaming(
        &self,
        target: ReviewTarget,
        delivery: Option<ReviewDelivery>,
        options: AppServerPromptOptions,
        acp_sender: Option<AcpEventSender>,
        interrupt_flag: Option<Arc<AtomicBool>>,
    ) -> Result<TaskResult, AdjutantError> {
        let (client, channels) =
            AppServerClient::spawn(&self.workspace_root)
                .await
                .map_err(|err| AdjutantError::ExecutionFailed(format!(
                    "Failed to spawn codex app-server: {}",
                    err
                )))?;

        let AppServerPromptOptions {
            model,
            effort: _,
            approval_policy,
            sandbox_mode,
            sandbox_policy: _,
            approval_mode,
        } = options;

        if let Err(err) = client.initialize().await {
            let _ = client.shutdown().await;
            return Err(AdjutantError::ExecutionFailed(format!(
                "Failed to initialize codex app-server: {}",
                err
            )));
        }

        let thread_response = client
            .thread_start(ThreadStartParams {
                model: model.clone(),
                model_provider: None,
                cwd: Some(self.workspace_root.to_string_lossy().to_string()),
                approval_policy: Some(approval_policy),
                sandbox: Some(sandbox_mode),
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex thread: {}",
                err
            )))?;

        let thread_id = thread_response.thread.id.clone();
        let review_response = client
            .review_start(ReviewStartParams {
                thread_id,
                target,
                delivery,
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex review: {}",
                err
            )))?;

        let turn_id = review_response.turn.id;
        let review_thread_id = review_response.review_thread_id;
        let result = run_app_server_turn_loop(
            client,
            channels,
            acp_sender,
            approval_mode,
            interrupt_flag,
            review_thread_id,
            turn_id,
        )
        .await;

        result
    }

    async fn execute_internal(
        &self,
        task: &Task,
        token_tx: Option<mpsc::UnboundedSender<String>>,
        acp_sender: Option<AcpEventSender>,
    ) -> Result<TaskResult, AdjutantError> {
        let prompt = task.to_prompt();
        let (client, mut channels) =
            AppServerClient::spawn(&self.workspace_root)
                .await
                .map_err(|err| AdjutantError::ExecutionFailed(format!(
                    "Failed to spawn codex app-server: {}",
                    err
                )))?;

        if let Err(err) = client.initialize().await {
            let _ = client.shutdown().await;
            return Err(AdjutantError::ExecutionFailed(format!(
                "Failed to initialize codex app-server: {}",
                err
            )));
        }

        let thread_response = client
            .thread_start(ThreadStartParams {
                model: None,
                model_provider: None,
                cwd: Some(self.workspace_root.to_string_lossy().to_string()),
                approval_policy: Some(AskForApproval::OnFailure),
                sandbox: Some(SandboxMode::WorkspaceWrite),
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex thread: {}",
                err
            )))?;

        let thread_id = thread_response.thread.id.clone();
        let turn_response = client
            .turn_start(TurnStartParams {
                thread_id: thread_id.clone(),
                input: vec![UserInput::Text { text: prompt }],
                model: None,
                effort: None,
                summary: None,
                approval_policy: None,
                sandbox_policy: None,
                cwd: None,
            })
            .await
            .map_err(|err| AdjutantError::ExecutionFailed(format!(
                "Failed to start codex turn: {}",
                err
            )))?;

        let _turn_id = turn_response.turn.id;
        let mut current_message = String::new();
        let mut final_response = String::new();
        let mut modified_files: HashSet<String> = HashSet::new();
        let mut saw_completion = false;
        let mut had_failure = false;
        let mut error: Option<String> = None;

        while !saw_completion {
            tokio::select! {
                Some(notification) = channels.notifications.recv() => {
                    handle_notification(
                        notification,
                        &token_tx,
                        &acp_sender,
                        &mut current_message,
                        &mut final_response,
                        &mut modified_files,
                        &mut had_failure,
                        &mut error,
                        &mut saw_completion,
                    );
                }
                Some(request) = channels.requests.recv() => {
                    handle_request(&client, request).await;
                }
                else => {
                    break;
                }
            }
        }

        let _ = client.shutdown().await;

        let success = saw_completion && !had_failure;
        let summary = if final_response.is_empty() {
            current_message
        } else {
            final_response
        };

        Ok(TaskResult {
            success,
            summary,
            modified_files: modified_files.into_iter().collect(),
            commit_hash: None,
            error,
            session_id: Some(thread_id),
        })
    }
}

struct AppServerRunState {
    current_message: String,
    final_response: String,
    modified_files: HashSet<String>,
    had_failure: bool,
    error: Option<String>,
    saw_completion: bool,
    command_outputs: HashMap<String, String>,
    file_change_outputs: HashMap<String, String>,
    approval_items: HashMap<String, Value>,
    agent_message_with_delta: HashSet<String>,
    reasoning_with_delta: HashSet<String>,
    diff_tool_ids: HashSet<String>,
    last_turn_diff: Option<String>,
}

impl AppServerRunState {
    fn new() -> Self {
        Self {
            current_message: String::new(),
            final_response: String::new(),
            modified_files: HashSet::new(),
            had_failure: false,
            error: None,
            saw_completion: false,
            command_outputs: HashMap::new(),
            file_change_outputs: HashMap::new(),
            approval_items: HashMap::new(),
            agent_message_with_delta: HashSet::new(),
            reasoning_with_delta: HashSet::new(),
            diff_tool_ids: HashSet::new(),
            last_turn_diff: None,
        }
    }
}

async fn run_app_server_turn_loop(
    client: AppServerClient,
    mut channels: AppServerChannels,
    acp_sender: Option<AcpEventSender>,
    approval_mode: ApprovalMode,
    interrupt_flag: Option<Arc<AtomicBool>>,
    thread_id: String,
    turn_id: String,
) -> Result<TaskResult, AdjutantError> {
    let mut state = AppServerRunState::new();
    let mut interrupted = false;

    while !state.saw_completion {
        if let Some(flag) = interrupt_flag.as_ref() {
            if flag.load(Ordering::Relaxed) && !interrupted {
                let _ = client
                    .turn_interrupt(TurnInterruptParams {
                        thread_id: thread_id.clone(),
                        turn_id: turn_id.clone(),
                    })
                    .await;
                interrupted = true;
            }
        }

        tokio::select! {
            Some(notification) = channels.notifications.recv() => {
                handle_prompt_notification(
                    notification,
                    &acp_sender,
                    &mut state,
                );
            }
            Some(request) = channels.requests.recv() => {
                handle_prompt_request(
                    &client,
                    request,
                    approval_mode,
                    &state.approval_items,
                ).await;
            }
            else => {
                break;
            }
        }
    }

    if let Some(diff) = state.last_turn_diff.take() {
        send_diff_tool_update(&acp_sender, &mut state.diff_tool_ids, &turn_id, diff);
    }

    let _ = client.shutdown().await;

    let success = state.saw_completion && !state.had_failure;
    let summary = if state.final_response.is_empty() {
        state.current_message
    } else {
        state.final_response
    };

    Ok(TaskResult {
        success,
        summary,
        modified_files: state.modified_files.into_iter().collect(),
        commit_hash: None,
        error: state.error,
        session_id: Some(thread_id),
    })
}

fn handle_prompt_notification(
    notification: AppServerNotification,
    acp_sender: &Option<AcpEventSender>,
    state: &mut AppServerRunState,
) {
    match notification.method.as_str() {
        "item/agentMessage/delta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<AgentMessageDeltaNotification>(params) {
                    state.agent_message_with_delta.insert(event.item_id);
                    state.current_message.push_str(&event.delta);
                    send_text_chunk(acp_sender, event.delta, false);
                }
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ReasoningDeltaNotification>(params) {
                    state.reasoning_with_delta.insert(event.item_id);
                    send_text_chunk(acp_sender, event.delta, true);
                }
            }
        }
        "item/commandExecution/outputDelta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<CommandExecutionOutputDeltaNotification>(params) {
                    append_tool_output(&mut state.command_outputs, &event.item_id, &event.delta);
                }
            }
        }
        "item/fileChange/outputDelta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<FileChangeOutputDeltaNotification>(params) {
                    append_tool_output(&mut state.file_change_outputs, &event.item_id, &event.delta);
                }
            }
        }
        "item/started" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ItemNotification>(params) {
                    if let Some(item_type) = item_type(&event.item) {
                        if matches!(item_type, "commandExecution" | "fileChange") {
                            if let Some(id) = item_id(&event.item) {
                                state.approval_items.insert(id.to_string(), event.item.clone());
                            }
                        }
                    }
                    if let Some(sender) = acp_sender {
                        if let Some(tool_call) = tool_call_from_item(&event.item) {
                            sender.send_update(acp::SessionUpdate::ToolCall(tool_call));
                        }
                    }
                }
            }
        }
        "item/completed" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ItemNotification>(params) {
                    handle_prompt_item_completed(event.item, acp_sender, state);
                }
            }
        }
        "turn/diff/updated" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<TurnDiffUpdatedNotification>(params) {
                    state.last_turn_diff = Some(event.diff);
                }
            }
        }
        "turn/plan/updated" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<TurnPlanUpdatedNotification>(params) {
                    if let Some(sender) = acp_sender {
                        let plan = plan_steps_to_acp(&event.plan);
                        sender.send_update(acp::SessionUpdate::Plan(plan));
                    }
                }
            }
        }
        "thread/tokenUsage/updated" => {
            if let Some(params) = notification.params {
                let _ = serde_json::from_value::<ThreadTokenUsageUpdatedNotification>(params);
            }
        }
        "thread/compacted" => {
            if let Some(params) = notification.params {
                let _ = serde_json::from_value::<ContextCompactedNotification>(params);
            }
        }
        "turn/completed" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<TurnCompletedNotification>(params) {
                    if event.turn.status == "failed" {
                        state.had_failure = true;
                        if let Some(err) = event.turn.error {
                            state.error = Some(err.message);
                        }
                    }
                }
            }
            state.saw_completion = true;
        }
        "error" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ErrorNotification>(params) {
                    if !event.will_retry {
                        state.had_failure = true;
                        state.error = Some(event.error.message);
                        state.saw_completion = true;
                    }
                }
            }
        }
        _ => {}
    }
}

fn handle_prompt_item_completed(
    item: Value,
    acp_sender: &Option<AcpEventSender>,
    state: &mut AppServerRunState,
) {
    let Some(item_type) = item_type(&item) else {
        return;
    };
    match item_type {
        "agentMessage" => {
            let Some(id) = item_id(&item) else {
                return;
            };
            if state.agent_message_with_delta.remove(id) {
                return;
            }
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                state.final_response = text.to_string();
                send_text_chunk(acp_sender, text.to_string(), false);
            }
        }
        "reasoning" => {
            let Some(id) = item_id(&item) else {
                return;
            };
            if state.reasoning_with_delta.remove(id) {
                return;
            }
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts));
            let content = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts));
            let text = summary
                .filter(|value| !value.trim().is_empty())
                .or_else(|| content.filter(|value| !value.trim().is_empty()));
            if let Some(text) = text {
                send_text_chunk(acp_sender, text, true);
            }
        }
        "commandExecution" => {
            if let Some(id) = item_id(&item) {
                state.approval_items.remove(id);
                let buffered = state
                    .command_outputs
                    .remove(id)
                    .unwrap_or_default();
                if let Some(update) = tool_call_update_from_item_with_buffers(
                    &item,
                    Some(&buffered),
                    None,
                ) {
                    send_tool_update(acp_sender, update);
                }
                if let Some(status) = item_status(&item) {
                    if is_failure_status(status) {
                        state.had_failure = true;
                    }
                }
            }
        }
        "fileChange" => {
            if let Some(id) = item_id(&item) {
                state.approval_items.remove(id);
                let buffered = state
                    .file_change_outputs
                    .remove(id)
                    .unwrap_or_default();
                if let Some(update) = tool_call_update_from_item_with_buffers(
                    &item,
                    None,
                    Some(&buffered),
                ) {
                    send_tool_update(acp_sender, update);
                }
                if let Some(status) = item_status(&item) {
                    if status == "completed" {
                        for path in extract_file_paths(&item) {
                            state.modified_files.insert(path);
                        }
                    } else if is_failure_status(status) {
                        state.had_failure = true;
                    }
                }
            }
        }
        "mcpToolCall" => {
            if let Some(update) = tool_call_update_from_item(&item) {
                send_tool_update(acp_sender, update);
            }
            if let Some(status) = item_status(&item) {
                if is_failure_status(status) {
                    state.had_failure = true;
                }
            }
        }
        "enteredReviewMode" => {
            if let Some(review) = item.get("review").and_then(Value::as_str) {
                send_text_chunk(acp_sender, format!("Review started: {}", review), false);
            }
        }
        "exitedReviewMode" => {
            if let Some(review) = item.get("review").and_then(Value::as_str) {
                send_text_chunk(acp_sender, format!("Review completed: {}", review), false);
            }
        }
        _ => {}
    }
}

async fn handle_prompt_request(
    client: &AppServerClient,
    request: AppServerRequest,
    approval_mode: ApprovalMode,
    approval_items: &HashMap<String, Value>,
) {
    let AppServerRequest { id, method, params } = request;
    let params_value = params.clone();

    let parsed = match method.as_str() {
        "item/commandExecution/requestApproval" => {
            params
                .and_then(|value| serde_json::from_value::<CommandExecutionRequestApprovalParams>(value).ok())
                .map(|parsed| (ApprovalKind::CommandExecution, parsed.item_id, parsed.reason))
        }
        "item/fileChange/requestApproval" => {
            params
                .and_then(|value| serde_json::from_value::<FileChangeRequestApprovalParams>(value).ok())
                .map(|parsed| (ApprovalKind::FileChange, parsed.item_id, parsed.reason))
        }
        _ => None,
    };

    let Some((kind, item_id, reason)) = parsed else {
        let _ = client
            .respond(
                id,
                &ApprovalResponse {
                    decision: ApprovalDecision::Decline,
                    accept_settings: None,
                },
            )
            .await;
        return;
    };

    let approval_item = approval_items.get(&item_id).cloned();
    let decision = match approval_mode {
        ApprovalMode::AutoAccept => ApprovalChoice {
            decision: ApprovalDecision::Accept,
            accept_settings: None,
        },
        ApprovalMode::AutoDecline => ApprovalChoice {
            decision: ApprovalDecision::Decline,
            accept_settings: None,
        },
        ApprovalMode::Prompt => prompt_for_approval(
            kind,
            &item_id,
            reason.as_deref(),
            approval_item.as_ref(),
            params_value.as_ref(),
        )
        .await,
    };

    let _ = client.respond(id, &ApprovalResponse {
        decision: decision.decision,
        accept_settings: decision.accept_settings,
    }).await;
}

#[derive(Clone, Copy)]
enum ApprovalKind {
    CommandExecution,
    FileChange,
}

struct ApprovalChoice {
    decision: ApprovalDecision,
    accept_settings: Option<ApprovalAcceptSettings>,
}

async fn prompt_for_approval(
    kind: ApprovalKind,
    item_id: &str,
    reason: Option<&str>,
    approval_item: Option<&Value>,
    params: Option<&Value>,
) -> ApprovalChoice {
    let prompt = approval_prompt_message(kind, item_id, reason, approval_item, params);
    let decision = spawn_blocking(move || {
        let mut stderr = io::stderr();
        let _ = writeln!(stderr, "\n{}", prompt);
        let _ = write!(stderr, "Approve? [y]es/[a]llow session/[n]o: ");
        let _ = stderr.flush();

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return ApprovalChoice {
                decision: ApprovalDecision::Decline,
                accept_settings: None,
            };
        }

        match input.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => ApprovalChoice {
                decision: ApprovalDecision::Accept,
                accept_settings: None,
            },
            "a" | "always" => ApprovalChoice {
                decision: ApprovalDecision::Accept,
                accept_settings: Some(ApprovalAcceptSettings { for_session: true }),
            },
            _ => ApprovalChoice {
                decision: ApprovalDecision::Decline,
                accept_settings: None,
            },
        }
    })
    .await
    .unwrap_or(ApprovalChoice {
        decision: ApprovalDecision::Decline,
        accept_settings: None,
    });

    match kind {
        ApprovalKind::FileChange => ApprovalChoice {
            decision: decision.decision,
            accept_settings: None,
        },
        ApprovalKind::CommandExecution => decision,
    }
}

fn approval_prompt_message(
    kind: ApprovalKind,
    item_id: &str,
    reason: Option<&str>,
    approval_item: Option<&Value>,
    params: Option<&Value>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Approval requested ({}).", item_id));
    if let Some(reason) = reason {
        lines.push(format!("Reason: {}", reason));
    }
    match kind {
        ApprovalKind::CommandExecution => {
            let command = params
                .and_then(command_string_from_params)
                .or_else(|| approval_item.and_then(command_string_from_item))
                .unwrap_or_else(|| "unknown command".to_string());
            lines.push(format!("Command: {}", command));
        }
        ApprovalKind::FileChange => {
            let (paths, _, _) = approval_item.map(extract_file_changes).unwrap_or_default();
            if paths.is_empty() {
                lines.push("Files: unknown".to_string());
            } else {
                lines.push(format!("Files: {}", paths.join(", ")));
            }
        }
    }
    lines.join("\n")
}

fn send_text_chunk(acp_sender: &Option<AcpEventSender>, text: String, is_thought: bool) {
    let Some(sender) = acp_sender else {
        return;
    };
    let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(
        acp::TextContent::new(text),
    ));
    let update = if is_thought {
        acp::SessionUpdate::AgentThoughtChunk(chunk)
    } else {
        acp::SessionUpdate::AgentMessageChunk(chunk)
    };
    sender.send_update(update);
}

fn send_tool_update(acp_sender: &Option<AcpEventSender>, update: acp::ToolCallUpdate) {
    if let Some(sender) = acp_sender {
        sender.send_update(acp::SessionUpdate::ToolCallUpdate(update));
    }
}

fn append_tool_output(buffers: &mut HashMap<String, String>, item_id: &str, delta: &str) {
    if delta.is_empty() {
        return;
    }
    buffers.entry(item_id.to_string()).or_default().push_str(delta);
}

fn send_diff_tool_update(
    acp_sender: &Option<AcpEventSender>,
    diff_tool_ids: &mut HashSet<String>,
    turn_id: &str,
    diff: String,
) {
    let Some(sender) = acp_sender else {
        return;
    };
    let tool_use_id = format!("turn-diff-{}", turn_id);
    if diff_tool_ids.insert(tool_use_id.clone()) {
        let tool_call = acp::ToolCall::new(
            acp::ToolCallId::new(tool_use_id.clone()),
            "Diff".to_string(),
        )
        .raw_input(serde_json::json!({ "file_path": "turn diff" }))
        .status(acp::ToolCallStatus::InProgress);
        sender.send_update(acp::SessionUpdate::ToolCall(tool_call));
    }

    let mut fields = acp::ToolCallUpdateFields::new();
    fields = fields
        .status(acp::ToolCallStatus::Completed)
        .raw_output(serde_json::json!({ "content": diff }));
    let mut update = acp::ToolCallUpdate::new(acp::ToolCallId::new(tool_use_id), fields);
    let mut meta = acp::Meta::new();
    meta.insert(
        autopilot_core::ACP_TOOL_NAME_META_KEY.to_string(),
        serde_json::Value::String("Diff".to_string()),
    );
    update.meta = Some(meta);
    sender.send_update(acp::SessionUpdate::ToolCallUpdate(update));
}

fn command_string_from_params(params: &Value) -> Option<String> {
    params
        .get("parsedCmd")
        .and_then(value_to_command_string)
        .or_else(|| params.get("command").and_then(value_to_command_string))
}

fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn handle_notification(
    notification: AppServerNotification,
    token_tx: &Option<mpsc::UnboundedSender<String>>,
    acp_sender: &Option<AcpEventSender>,
    current_message: &mut String,
    final_response: &mut String,
    modified_files: &mut HashSet<String>,
    had_failure: &mut bool,
    error: &mut Option<String>,
    saw_completion: &mut bool,
) {
    match notification.method.as_str() {
        "item/agentMessage/delta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<AgentMessageDeltaNotification>(params) {
                    current_message.push_str(&event.delta);
                    if let Some(tx) = token_tx {
                        let _ = tx.send(event.delta);
                    }
                }
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ReasoningDeltaNotification>(params) {
                    if let Some(sender) = acp_sender {
                        let chunk = acp::ContentChunk::new(acp::ContentBlock::Text(
                            acp::TextContent::new(event.delta),
                        ));
                        sender.send_update(acp::SessionUpdate::AgentThoughtChunk(chunk));
                    }
                }
            }
        }
        "turn/plan/updated" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<TurnPlanUpdatedNotification>(params) {
                    if let Some(sender) = acp_sender {
                        let plan = plan_steps_to_acp(&event.plan);
                        sender.send_update(acp::SessionUpdate::Plan(plan));
                    }
                }
            }
        }
        "item/started" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ItemNotification>(params) {
                    if let Some(item_type) = item_type(&event.item) {
                        if item_type == "agentMessage" {
                            current_message.clear();
                        }
                    }
                    if let Some(sender) = acp_sender {
                        if let Some(tool_call) = tool_call_from_item(&event.item) {
                            sender.send_update(acp::SessionUpdate::ToolCall(tool_call));
                        }
                    }
                }
            }
        }
        "item/completed" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ItemNotification>(params) {
                    if let Some(item_type) = item_type(&event.item) {
                        match item_type {
                            "agentMessage" => {
                                if let Some(text) = event.item.get("text").and_then(Value::as_str) {
                                    *final_response = text.to_string();
                                }
                            }
                            "fileChange" => {
                                if let Some(status) = item_status(&event.item) {
                                    if status == "completed" {
                                        for path in extract_file_paths(&event.item) {
                                            modified_files.insert(path);
                                        }
                                    } else if is_failure_status(status) {
                                        *had_failure = true;
                                    }
                                }
                            }
                            "commandExecution" | "mcpToolCall" => {
                                if let Some(status) = item_status(&event.item) {
                                    if is_failure_status(status) {
                                        *had_failure = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Some(sender) = acp_sender {
                        if let Some(update) = tool_call_update_from_item(&event.item) {
                            sender.send_update(acp::SessionUpdate::ToolCallUpdate(update));
                        }
                    }
                }
            }
        }
        "turn/completed" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<TurnCompletedNotification>(params) {
                    if event.turn.status == "failed" {
                        *had_failure = true;
                        if let Some(err) = event.turn.error {
                            *error = Some(err.message);
                        }
                    }
                }
            }
            *saw_completion = true;
        }
        "error" => {
            if let Some(params) = notification.params {
                if let Ok(event) = serde_json::from_value::<ErrorNotification>(params) {
                    if !event.will_retry {
                        *had_failure = true;
                        *error = Some(event.error.message);
                        *saw_completion = true;
                    }
                }
            }
        }
        _ => {}
    }
}

async fn handle_request(client: &AppServerClient, request: AppServerRequest) {
    let decision = match request.method.as_str() {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            ApprovalDecision::Accept
        }
        _ => ApprovalDecision::Decline,
    };

    let _ = client
        .respond(
            request.id,
            &ApprovalResponse {
                decision,
                accept_settings: None,
            },
        )
        .await;
}

fn plan_steps_to_acp(steps: &[TurnPlanStep]) -> acp::Plan {
    let entries = steps
        .iter()
        .map(|step| {
            acp::PlanEntry::new(
                step.step.clone(),
                acp::PlanEntryPriority::Medium,
                plan_status_to_acp(&step.status),
            )
        })
        .collect();
    acp::Plan::new(entries)
}

fn plan_status_to_acp(status: &str) -> acp::PlanEntryStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => acp::PlanEntryStatus::Pending,
        "inprogress" | "in_progress" | "in-progress" => acp::PlanEntryStatus::InProgress,
        "completed" | "complete" => acp::PlanEntryStatus::Completed,
        _ => acp::PlanEntryStatus::Pending,
    }
}

fn tool_call_from_item(item: &Value) -> Option<acp::ToolCall> {
    let item_id = item_id(item)?;
    let item_type = item_type(item)?;
    match item_type {
        "commandExecution" => {
            let command = command_string_from_item(item).unwrap_or_default();
            Some(
                acp::ToolCall::new(acp::ToolCallId::new(item_id.to_string()), "Bash".to_string())
                    .raw_input(serde_json::json!({ "command": command }))
                    .status(acp::ToolCallStatus::InProgress),
            )
        }
        "fileChange" => {
            let paths = extract_file_paths(item);
            Some(
                acp::ToolCall::new(acp::ToolCallId::new(item_id.to_string()), "Edit".to_string())
                    .raw_input(serde_json::json!({ "files": paths }))
                    .status(acp::ToolCallStatus::InProgress),
            )
        }
        "mcpToolCall" => {
            let server = item.get("server").and_then(Value::as_str).unwrap_or("server");
            let tool = item.get("tool").and_then(Value::as_str).unwrap_or("tool");
            let name = format!("mcp__{}__{}", server, tool);
            let args = item.get("arguments").cloned().unwrap_or(Value::Null);
            Some(
                acp::ToolCall::new(acp::ToolCallId::new(item_id.to_string()), name)
                    .raw_input(args)
                    .status(acp::ToolCallStatus::InProgress),
            )
        }
        "webSearch" => {
            let query = item.get("query").and_then(Value::as_str).unwrap_or("");
            Some(
                acp::ToolCall::new(acp::ToolCallId::new(item_id.to_string()), "WebSearch".to_string())
                    .raw_input(serde_json::json!({ "query": query }))
                    .status(acp::ToolCallStatus::InProgress),
            )
        }
        _ => None,
    }
}

fn tool_call_update_from_item(item: &Value) -> Option<acp::ToolCallUpdate> {
    let item_id = item_id(item)?;
    let status = item_status(item);
    let tool_status = status.and_then(tool_status_from_str);

    let mut fields = acp::ToolCallUpdateFields::default();
    fields.status = tool_status;

    let item_type = item_type(item)?;
    match item_type {
        "commandExecution" => {
            if let Some(output) = command_output_from_item(item) {
                fields.raw_output = Some(output);
            }
        }
        "fileChange" => {
            let paths = extract_file_paths(item);
            if !paths.is_empty() {
                fields.raw_output = Some(serde_json::json!({ "changes": paths }));
            }
        }
        "mcpToolCall" => {
            if let Some(result) = item.get("result") {
                fields.raw_output = Some(result.clone());
            } else if let Some(err) = item.get("error") {
                fields.raw_output = Some(serde_json::json!({ "error": err }));
            }
        }
        _ => {}
    }

    Some(acp::ToolCallUpdate::new(
        acp::ToolCallId::new(item_id.to_string()),
        fields,
    ))
}

fn tool_call_update_from_item_with_buffers(
    item: &Value,
    command_buffer: Option<&str>,
    file_change_buffer: Option<&str>,
) -> Option<acp::ToolCallUpdate> {
    let item_id = item_id(item)?;
    let status = item_status(item);
    let tool_status = status.and_then(tool_status_from_str);

    let mut fields = acp::ToolCallUpdateFields::default();
    fields.status = tool_status;

    let item_type = item_type(item)?;
    match item_type {
        "commandExecution" => {
            if let Some(output) = command_output_from_item_with_buffer(item, command_buffer) {
                fields.raw_output = Some(output);
            }
        }
        "fileChange" => {
            if let Some(output) = file_change_output_from_item_with_buffer(item, file_change_buffer) {
                fields.raw_output = Some(output);
            }
        }
        "mcpToolCall" => {
            if let Some(result) = item.get("result") {
                fields.raw_output = Some(result.clone());
            } else if let Some(err) = item.get("error") {
                fields.raw_output = Some(serde_json::json!({ "error": err }));
            }
        }
        _ => {}
    }

    Some(acp::ToolCallUpdate::new(
        acp::ToolCallId::new(item_id.to_string()),
        fields,
    ))
}

fn item_id(item: &Value) -> Option<&str> {
    item.get("id").and_then(Value::as_str)
}

fn item_type(item: &Value) -> Option<&str> {
    item.get("type").and_then(Value::as_str)
}

fn item_status(item: &Value) -> Option<&str> {
    item.get("status").and_then(Value::as_str)
}

fn is_failure_status(status: &str) -> bool {
    matches!(status, "failed" | "declined")
}

fn tool_status_from_str(status: &str) -> Option<acp::ToolCallStatus> {
    match status {
        "inProgress" => Some(acp::ToolCallStatus::InProgress),
        "completed" => Some(acp::ToolCallStatus::Completed),
        "failed" | "declined" => Some(acp::ToolCallStatus::Failed),
        _ => None,
    }
}

fn extract_file_paths(item: &Value) -> Vec<String> {
    let Some(changes) = item.get("changes").and_then(Value::as_array) else {
        return Vec::new();
    };
    changes
        .iter()
        .filter_map(|change| change.get("path").and_then(Value::as_str))
        .map(|path| path.to_string())
        .collect()
}

fn extract_file_changes(item: &Value) -> (Vec<String>, Option<String>, Option<String>) {
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(changes) = item.get("changes").and_then(Value::as_array) {
        for change in changes {
            if let Some(path) = change.get("path").and_then(Value::as_str) {
                if first_path.is_none() {
                    first_path = Some(path.to_string());
                }
                paths.push(path.to_string());
            }
            if first_diff.is_none() {
                if let Some(diff) = change.get("diff").and_then(Value::as_str) {
                    first_diff = Some(diff.to_string());
                }
            }
        }
    }
    (paths, first_path, first_diff)
}

fn command_string_from_item(item: &Value) -> Option<String> {
    match item.get("command") {
        Some(Value::String(text)) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Some(Value::Array(parts)) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn join_string_array(parts: &[Value]) -> String {
    let mut output = String::new();
    for part in parts {
        if let Some(text) = part.as_str() {
            output.push_str(text);
        }
    }
    output
}

fn command_output_from_item(item: &Value) -> Option<Value> {
    let aggregated = item
        .get("aggregatedOutput")
        .and_then(Value::as_str)
        .unwrap_or("");
    let exit_code = item.get("exitCode").and_then(Value::as_i64);
    if aggregated.is_empty() && exit_code.is_none() {
        return None;
    }
    let mut output = serde_json::json!({ "content": aggregated });
    if let Some(code) = exit_code {
        output["exit_code"] = Value::Number(code.into());
    }
    Some(output)
}

fn command_output_from_item_with_buffer(item: &Value, buffer: Option<&str>) -> Option<Value> {
    let aggregated = item
        .get("aggregatedOutput")
        .and_then(Value::as_str)
        .unwrap_or("");
    let exit_code = item.get("exitCode").and_then(Value::as_i64);
    let fallback = buffer.unwrap_or("");
    let output_text = if !aggregated.trim().is_empty() {
        aggregated
    } else {
        fallback
    };
    if output_text.trim().is_empty() && exit_code.is_none() {
        return None;
    }
    let mut output = serde_json::json!({ "content": output_text });
    if let Some(code) = exit_code {
        output["exit_code"] = Value::Number(code.into());
    }
    Some(output)
}

fn file_change_output_from_item_with_buffer(
    item: &Value,
    buffer: Option<&str>,
) -> Option<Value> {
    let (paths, _, diff) = extract_file_changes(item);
    let diff_text = diff.or_else(|| buffer.map(|value| value.to_string()));
    if let Some(diff_text) = diff_text {
        if !diff_text.trim().is_empty() {
            return Some(serde_json::json!({ "content": diff_text }));
        }
    }
    if !paths.is_empty() {
        return Some(serde_json::json!({ "content": format!("Modified files: {}", paths.join(", ")) }));
    }
    None
}
