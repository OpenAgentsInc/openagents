use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

use agent_client_protocol_schema as acp;

use crate::autopilot_loop::AcpEventSender;
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
    _params: Option<Value>,
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
                                        _params: params,
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

    let program = codex_agent_sdk::transport::find_codex_executable()
        .context("codex executable not found")?;

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
enum AskForApproval {
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
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
struct ItemNotification {
    item: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnPlanUpdatedNotification {
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
struct TurnCompletedNotification {
    turn: TurnCompletedInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnCompletedInfo {
    id: String,
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
struct ApprovalResponse {
    decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ApprovalDecision {
    Accept,
    Decline,
}

pub struct AppServerExecutor {
    workspace_root: PathBuf,
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
            &ApprovalResponse { decision },
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
