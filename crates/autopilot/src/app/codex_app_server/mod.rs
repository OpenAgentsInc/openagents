use std::collections::HashMap;
use std::path::PathBuf;
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

pub(crate) mod types;
pub(crate) use types::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppServerRequestId {
    String(String),
    Integer(i64),
}

pub(crate) struct AppServerNotification {
    pub(crate) method: String,
    pub(crate) params: Option<Value>,
}

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
}

impl Default for AppServerConfig {
    fn default() -> Self {
        Self { cwd: None }
    }
}

struct AppServerTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    pending_requests: Arc<Mutex<PendingRequests>>,
    request_counter: AtomicI64,
    reader_task: Option<tokio::task::JoinHandle<()>>,
}

type PendingRequest = oneshot::Sender<Result<Value>>;
type PendingRequests = HashMap<AppServerRequestId, PendingRequest>;

impl AppServerTransport {
    fn new(stdin: ChildStdin, stdout: ChildStdout) -> (Self, AppServerChannels) {
        let stdin = Arc::new(Mutex::new(stdin));
        let pending_requests: Arc<Mutex<PendingRequests>> =
            Arc::new(Mutex::new(PendingRequests::new()));
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

pub(crate) struct AppServerClient {
    transport: AppServerTransport,
    process: Child,
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

        let (transport, channels) = AppServerTransport::new(stdin, stdout);

        Ok((Self { transport, process: child }, channels))
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

    pub(crate) async fn turn_start(&self, params: TurnStartParams) -> Result<TurnStartResponse> {
        self.transport.request("turn/start", Some(&params)).await
    }

    pub(crate) async fn turn_interrupt(&self, params: TurnInterruptParams) -> Result<TurnInterruptResponse> {
        self.transport.request("turn/interrupt", Some(&params)).await
    }

    pub(crate) async fn respond<T>(&self, id: AppServerRequestId, result: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.transport.respond(id, result).await
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
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
