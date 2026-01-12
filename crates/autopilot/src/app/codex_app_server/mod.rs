use std::collections::HashMap;
use std::path::PathBuf;
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

pub(crate) mod types;
pub(crate) use types::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppServerRequestId {
    String(String),
    Integer(i64),
}

#[derive(Debug)]
pub(crate) struct AppServerNotification {
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

#[derive(Debug)]
pub(crate) struct AppServerRequest {
    pub(crate) id: AppServerRequestId,
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

pub(crate) struct AppServerChannels {
    pub(crate) notifications: mpsc::Receiver<AppServerNotification>,
    pub(crate) requests: mpsc::Receiver<AppServerRequest>,
}

pub(crate) struct AppServerConfig {
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) wire_log: Option<AppServerWireLog>,
}

impl Default for AppServerConfig {
    fn default() -> Self {
        Self {
            cwd: None,
            wire_log: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppServerWireLog {
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
    pub(crate) fn new() -> Self {
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
                        let line = serde_json::to_string(&entry).unwrap_or_else(|_| "{}".to_string());
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

    pub(crate) fn set_path(&self, path: PathBuf) {
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

pub(crate) struct AppServerClient {
    transport: AppServerTransport,
    process: Option<Child>,
}

impl AppServerClient {
    pub(crate) async fn spawn(config: AppServerConfig) -> Result<(Self, AppServerChannels)> {
        let command = resolve_app_server_command()
            .context("Unable to find codex app-server executable")?;

        let mut cmd = Command::new(command.program);
        cmd.args(command.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(cwd) = config.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn().context("Failed to spawn codex app-server")?;
        let stdin = child.stdin.take().context("codex app-server stdin missing")?;
        let stdout = child.stdout.take().context("codex app-server stdout missing")?;

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

    pub(crate) fn connect_with_io(
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

    pub(crate) async fn initialize(&self, info: ClientInfo) -> Result<InitializeResponse> {
        let params = InitializeParams { client_info: info };
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

    pub(crate) async fn thread_start(&self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        self.transport.request("thread/start", Some(&params)).await
    }

    pub(crate) async fn thread_resume(&self, params: ThreadResumeParams) -> Result<ThreadResumeResponse> {
        self.transport.request("thread/resume", Some(&params)).await
    }

    pub(crate) async fn thread_archive(&self, params: ThreadArchiveParams) -> Result<ThreadArchiveResponse> {
        self.transport.request("thread/archive", Some(&params)).await
    }

    pub(crate) async fn thread_list(&self, params: ThreadListParams) -> Result<ThreadListResponse> {
        self.transport.request("thread/list", Some(&params)).await
    }

    pub(crate) async fn model_list(&self, params: ModelListParams) -> Result<ModelListResponse> {
        self.transport.request("model/list", Some(&params)).await
    }

    pub(crate) async fn config_read(&self, params: ConfigReadParams) -> Result<ConfigReadResponse> {
        self.transport.request("config/read", Some(&params)).await
    }

    pub(crate) async fn config_value_write(
        &self,
        params: ConfigValueWriteParams,
    ) -> Result<ConfigWriteResponse> {
        self.transport
            .request("config/value/write", Some(&params))
            .await
    }

    pub(crate) async fn config_batch_write(
        &self,
        params: ConfigBatchWriteParams,
    ) -> Result<ConfigWriteResponse> {
        self.transport
            .request("config/batchWrite", Some(&params))
            .await
    }

    pub(crate) async fn account_read(&self, params: GetAccountParams) -> Result<GetAccountResponse> {
        self.transport.request("account/read", Some(&params)).await
    }

    pub(crate) async fn account_login_start(
        &self,
        params: LoginAccountParams,
    ) -> Result<LoginAccountResponse> {
        self.transport
            .request("account/login/start", Some(&params))
            .await
    }

    pub(crate) async fn account_login_cancel(
        &self,
        params: CancelLoginAccountParams,
    ) -> Result<CancelLoginAccountResponse> {
        self.transport
            .request("account/login/cancel", Some(&params))
            .await
    }

    pub(crate) async fn account_logout(&self) -> Result<LogoutAccountResponse> {
        self.transport.request::<Value, _>("account/logout", None).await
    }

    pub(crate) async fn account_rate_limits_read(
        &self,
    ) -> Result<GetAccountRateLimitsResponse> {
        self.transport
            .request::<Value, _>("account/rateLimits/read", None)
            .await
    }

    pub(crate) async fn mcp_server_status_list(
        &self,
        params: ListMcpServerStatusParams,
    ) -> Result<ListMcpServerStatusResponse> {
        self.transport
            .request("mcpServerStatus/list", Some(&params))
            .await
    }

    pub(crate) async fn mcp_server_oauth_login(
        &self,
        params: McpServerOauthLoginParams,
    ) -> Result<McpServerOauthLoginResponse> {
        self.transport
            .request("mcpServer/oauth/login", Some(&params))
            .await
    }

    pub(crate) async fn skills_list(&self, params: SkillsListParams) -> Result<SkillsListResponse> {
        self.transport.request("skills/list", Some(&params)).await
    }

    pub(crate) async fn turn_start(&self, params: TurnStartParams) -> Result<TurnStartResponse> {
        self.transport.request("turn/start", Some(&params)).await
    }

    pub(crate) async fn turn_interrupt(&self, params: TurnInterruptParams) -> Result<TurnInterruptResponse> {
        self.transport.request("turn/interrupt", Some(&params)).await
    }

    pub(crate) async fn review_start(&self, params: ReviewStartParams) -> Result<ReviewStartResponse> {
        self.transport.request("review/start", Some(&params)).await
    }

    pub(crate) async fn command_exec(
        &self,
        params: CommandExecParams,
    ) -> Result<CommandExecResponse> {
        self.transport.request("command/exec", Some(&params)).await
    }

    pub(crate) async fn respond<T>(&self, id: AppServerRequestId, result: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.transport.respond(id, result).await
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[tokio::test]
    async fn initialize_sends_initialized() {
        let (client_stream, server_stream) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);

        let (client, _channels) = AppServerClient::connect_with_io(
            Box::new(client_write),
            Box::new(client_read),
            None,
        );

        let server_task = tokio::spawn(async move {
            let mut reader = BufReader::new(server_read).lines();
            let init_line = reader
                .next_line()
                .await
                .expect("init line read")
                .expect("init line present");
            let init_value: Value = serde_json::from_str(&init_line).expect("init json");
            assert_eq!(
                init_value.get("method").and_then(Value::as_str),
                Some("initialize")
            );
            let id = init_value
                .get("id")
                .cloned()
                .expect("init id");
            let response = serde_json::json!({
                "id": id,
                "result": { "userAgent": "codex-test" }
            });
            server_write
                .write_all(response.to_string().as_bytes())
                .await
                .expect("write response");
            server_write.write_all(b"\n").await.expect("newline");

            let initialized_line = reader
                .next_line()
                .await
                .expect("initialized line read")
                .expect("initialized line present");
            let initialized_value: Value =
                serde_json::from_str(&initialized_line).expect("initialized json");
            assert_eq!(
                initialized_value.get("method").and_then(Value::as_str),
                Some("initialized")
            );
        });

        let info = ClientInfo {
            name: "test-client".to_string(),
            title: Some("Test".to_string()),
            version: "0.0.0".to_string(),
        };
        client.initialize(info).await.expect("initialize ok");
        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn streams_notifications_over_channel() {
        let (client_stream, server_stream) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (_server_read, mut server_write) = tokio::io::split(server_stream);

        let (_client, mut channels) = AppServerClient::connect_with_io(
            Box::new(client_write),
            Box::new(client_read),
            None,
        );

        let server_task = tokio::spawn(async move {
            let messages = vec![
                serde_json::json!({
                    "method": "turn/started",
                    "params": { "threadId": "thr_1", "turn": { "id": "turn_1" } }
                }),
                serde_json::json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "item_1", "delta": "Hello" }
                }),
                serde_json::json!({
                    "method": "turn/completed",
                    "params": { "turn": { "id": "turn_1", "status": "completed" } }
                }),
            ];

            for message in messages {
                server_write
                    .write_all(message.to_string().as_bytes())
                    .await
                    .expect("write notification");
                server_write.write_all(b"\n").await.expect("newline");
            }

        });

        let first = channels.notifications.recv().await.expect("notification");
        assert_eq!(first.method, "turn/started");
        let second = channels.notifications.recv().await.expect("notification");
        assert_eq!(second.method, "item/agentMessage/delta");
        let third = channels.notifications.recv().await.expect("notification");
        assert_eq!(third.method, "turn/completed");

        server_task.await.expect("server task");
    }

    #[tokio::test]
    async fn responds_to_approval_requests() {
        let (client_stream, server_stream) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_stream);
        let (server_read, mut server_write) = tokio::io::split(server_stream);

        let (client, mut channels) = AppServerClient::connect_with_io(
            Box::new(client_write),
            Box::new(client_read),
            None,
        );

        let server_task = tokio::spawn(async move {
            let request = serde_json::json!({
                "id": 42,
                "method": "item/commandExecution/requestApproval",
                "params": {
                    "threadId": "thr_1",
                    "turnId": "turn_1",
                    "itemId": "item_1",
                    "reason": "dangerous"
                }
            });
            server_write
                .write_all(request.to_string().as_bytes())
                .await
                .expect("write request");
            server_write.write_all(b"\n").await.expect("newline");

            let mut reader = BufReader::new(server_read).lines();
            let response_line = reader
                .next_line()
                .await
                .expect("response line read")
                .expect("response line present");
            let response: Value = serde_json::from_str(&response_line).expect("response json");
            assert_eq!(response.get("id").and_then(Value::as_i64), Some(42));
            assert_eq!(
                response
                    .get("result")
                    .and_then(|result| result.get("decision"))
                    .and_then(Value::as_str),
                Some("accept")
            );
        });

        let request = channels.requests.recv().await.expect("approval request");
        client
            .respond(
                request.id,
                &ApprovalResponse {
                    decision: ApprovalDecision::Accept,
                    accept_settings: None,
                },
            )
            .await
            .expect("approval response");

        server_task.await.expect("server task");
    }
}
