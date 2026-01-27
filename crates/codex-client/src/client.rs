//! Codex app-server JSON-RPC client.

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

use crate::types::*;

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

    pub async fn initialize(&self, info: ClientInfo) -> Result<InitializeResponse> {
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

    pub async fn thread_start(&self, params: ThreadStartParams) -> Result<ThreadStartResponse> {
        self.transport.request("thread/start", Some(&params)).await
    }

    pub async fn thread_resume(&self, params: ThreadResumeParams) -> Result<ThreadResumeResponse> {
        self.transport.request("thread/resume", Some(&params)).await
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

    pub async fn model_list(&self, params: ModelListParams) -> Result<ModelListResponse> {
        self.transport.request("model/list", Some(&params)).await
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

    pub async fn skills_list(&self, params: SkillsListParams) -> Result<SkillsListResponse> {
        self.transport.request("skills/list", Some(&params)).await
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

    pub async fn review_start(&self, params: ReviewStartParams) -> Result<ReviewStartResponse> {
        self.transport.request("review/start", Some(&params)).await
    }

    pub async fn command_exec(&self, params: CommandExecParams) -> Result<CommandExecResponse> {
        self.transport.request("command/exec", Some(&params)).await
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

const CODEX_PATHS: &[&str] = &[
    ".npm-global/bin/codex",
    ".local/bin/codex",
    "node_modules/.bin/codex",
    "/usr/local/bin/codex",
    "/opt/homebrew/bin/codex",
];

fn resolve_app_server_command() -> Result<AppServerCommand> {
    if let Ok(program) = which::which("codex-app-server") {
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
    if let Ok(path) = which::which("codex") {
        return Ok(path);
    }

    if let Some(home) = dirs::home_dir() {
        for path in CODEX_PATHS {
            let full_path = if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                home.join(path)
            };
            if full_path.exists() && full_path.is_file() {
                return Ok(full_path);
            }
        }
    }

    Err(anyhow::anyhow!("codex executable not found"))
}

/// Check if Codex app-server is available on this system.
pub fn is_codex_available() -> bool {
    which::which("codex-app-server").is_ok() || which::which("codex").is_ok() || {
        if let Some(home) = dirs::home_dir() {
            CODEX_PATHS.iter().any(|path| {
                let full_path = if path.starts_with('/') {
                    PathBuf::from(path)
                } else {
                    home.join(path)
                };
                full_path.exists() && full_path.is_file()
            })
        } else {
            false
        }
    }
}
