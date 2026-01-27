use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::file_logger::FileLogger;

/// ACP event that will be sent to the frontend
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AcpEvent {
    pub workspace_id: String,
    pub message: Value,
}

/// Callback for forwarding ACP events to unified event stream
pub type AcpEventCallback = Arc<dyn Fn(&AcpEvent) + Send + Sync>;

/// ACP connection that manages the codex-acp process and captures raw events
/// For Phase 1, we're just capturing raw JSON-RPC messages without using the ACP library
pub struct AcpConnection {
    workspace_id: String,
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    app: AppHandle,
    session_id: Arc<Mutex<Option<String>>>, // Store the ACP session ID
    file_logger: Arc<FileLogger>,
    // Track request IDs to session IDs for proper event flushing
    request_to_session: Arc<Mutex<std::collections::HashMap<u64, String>>>,
    // Optional callback for forwarding events to unified stream
    event_callback: Arc<Mutex<Option<AcpEventCallback>>>,
}

impl AcpConnection {
    /// Spawn an ACP process and capture all raw JSON-RPC messages
    pub async fn new(
        workspace_id: String,
        workspace_path: &Path,
        command: String,
        args: Vec<String>,
        env: std::collections::HashMap<String, String>,
        app: AppHandle,
    ) -> Result<Self> {
        // Create file logger for ACP events
        let file_logger = Arc::new(FileLogger::new().await?);

        tracing::info!(command = %command, args = ?args, "Spawning ACP agent");

        let mut child = Command::new(&command);
        child
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(workspace_path);

        // Pass through environment variables
        for (key, value) in env {
            child.env(key, value);
        }

        // Ensure path is inherited
        if let Ok(path) = std::env::var("PATH") {
            child.env("PATH", path);
        }

        let mut child = child
            .spawn()
            .context(format!("Failed to spawn ACP agent: {}", command))?;

        let stdin = child.stdin.take().context("Failed to take stdin")?;
        let stdout = child.stdout.take().context("Failed to take stdout")?;
        let stderr = child.stderr.take().context("Failed to take stderr")?;

        // Track request IDs to session IDs
        let request_to_session: Arc<Mutex<std::collections::HashMap<u64, String>>> =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        let request_to_session_for_capture = request_to_session.clone();

        // Event callback for forwarding to unified stream
        let event_callback_for_stdout: Arc<Mutex<Option<AcpEventCallback>>> =
            Arc::new(Mutex::new(None));
        let event_callback_for_stdout_clone = event_callback_for_stdout.clone();

        // Capture stdout (JSON-RPC messages) and extract session ID from responses
        let app_clone = app.clone();
        let workspace_id_clone = workspace_id.clone();
        let session_id_clone = Arc::new(Mutex::new(None::<String>)); // Will be set from session_id field
        let session_id_for_capture = session_id_clone.clone();
        let file_logger_clone = file_logger.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    line.clear();
                    continue;
                }

                // Try to parse as JSON-RPC message
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(value) => {
                        // Extract session ID from session/new response
                        if let Some(result) = value.get("result")
                            && let Some(session_id) =
                                result.get("sessionId").and_then(|s| s.as_str())
                            {
                                let mut stored = session_id_for_capture.lock().await;
                                *stored = Some(session_id.to_string());
                                tracing::debug!(
                                    session_id = %session_id,
                                    "Stored ACP session ID"
                                );
                            }

                        // Track responses: if this is a response (has id, no method), check if we have a session for this request
                        if let Some(response_id) = value.get("id").and_then(|id| id.as_u64()) {
                            // Check if this response has stopReason (completion indicator)
                            if let Some(stop_reason) =
                                value.get("stopReason").and_then(|s| s.as_str())
                                && stop_reason == "end_turn" {
                                    // Look up which session this request was for
                                    let session_id = {
                                        let mut req_map =
                                            request_to_session_for_capture.lock().await;
                                        req_map.remove(&response_id)
                                    };
                                    if let Some(sid) = session_id {
                                        tracing::info!(
                                            request_id = response_id,
                                            session_id = %sid,
                                            "ACP completion: stopReason=end_turn"
                                        );
                                        // Trigger flush for this specific session
                                        if let Err(e) = file_logger_clone
                                            .flush_acp_events(Some(sid.clone()))
                                            .await
                                        {
                                            tracing::warn!(
                                                session_id = %sid,
                                                error = %e,
                                                "Failed to flush ACP events for session"
                                            );
                                        }
                                    } else {
                                        tracing::info!(
                                            request_id = response_id,
                                            "ACP completion: stopReason=end_turn without session, flushing all"
                                        );
                                        // Fallback: flush all if we can't find the session
                                        if let Err(e) =
                                            file_logger_clone.flush_all_acp_events().await
                                        {
                                            tracing::warn!(
                                                error = %e,
                                                "Failed to flush all ACP events"
                                            );
                                        }
                                    }
                                }
                        }

                        let event = AcpEvent {
                            workspace_id: workspace_id_clone.clone(),
                            message: json!({
                                "type": "acp/raw_message",
                                "direction": "incoming",
                                "message": value,
                            }),
                        };
                        let _ = app_clone.emit("acp-event", &event);

                        // Forward to unified event stream if callback is set
                        {
                            let callback_guard = event_callback_for_stdout.lock().await;
                            if let Some(callback) = callback_guard.as_ref() {
                                callback(&event);
                            }
                        }

                        // Buffer event and flush when message completes
                        let event_value = serde_json::to_value(&event).unwrap_or_default();
                        if let Err(e) = file_logger_clone.check_and_flush_acp(&event_value).await {
                            tracing::warn!(error = %e, "Failed to buffer/flush ACP event");
                        }
                    }
                    Err(_) => {
                        // Emit as raw text if not valid JSON
                        let event = AcpEvent {
                            workspace_id: workspace_id_clone.clone(),
                            message: json!({
                                "type": "acp/raw_output",
                                "direction": "incoming",
                                "text": trimmed,
                            }),
                        };
                        let _ = app_clone.emit("acp-event", &event);

                        // Buffer event and flush when message completes
                        let event_value = serde_json::to_value(&event).unwrap_or_default();
                        if let Err(e) = file_logger_clone.check_and_flush_acp(&event_value).await {
                            tracing::warn!(error = %e, "Failed to buffer/flush ACP event");
                        }
                    }
                }

                line.clear();
            }
        });

        // Capture stderr (logs/errors)
        let app_clone2 = app.clone();
        let workspace_id_clone2 = workspace_id.clone();
        let file_logger_for_stderr = file_logger.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();

            while let Ok(n) = reader.read_line(&mut line).await {
                if n == 0 {
                    break;
                }

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    line.clear();
                    continue;
                }

                let event = AcpEvent {
                    workspace_id: workspace_id_clone2.clone(),
                    message: json!({
                        "type": "acp/stderr",
                        "text": trimmed,
                    }),
                };
                let _ = app_clone2.emit("acp-event", &event);

                // Buffer stderr events (they don't indicate completion, so just buffer)
                let event_value = serde_json::to_value(&event).unwrap_or_default();
                if let Err(e) = file_logger_for_stderr
                    .check_and_flush_acp(&event_value)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to buffer ACP event");
                }

                line.clear();
            }
        });

        // Send initialization message
        let init_message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0",
                "clientCapabilities": {
                    "fs": {
                        "readTextFile": false,
                        "writeTextFile": false
                    },
                    "terminal": false
                },
                "clientInfo": {
                    "name": "autopilot",
                    "version": "0.1.0"
                }
            }
        });

        // Write initialization
        use tokio::io::AsyncWriteExt;
        let stdin_arc = Arc::new(Mutex::new(stdin));
        let stdin_for_init = stdin_arc.clone();
        let init_json = serde_json::to_string(&init_message)?;
        {
            let mut stdin_handle = stdin_for_init.lock().await;
            stdin_handle.write_all(init_json.as_bytes()).await?;
            stdin_handle.write_all(b"\n").await?;
            stdin_handle.flush().await?;
        }

        // Emit the initialization message we sent
        let event = AcpEvent {
            workspace_id: workspace_id.clone(),
            message: json!({
                "type": "acp/raw_message",
                "direction": "outgoing",
                "message": init_message,
            }),
        };
        let _ = app.emit("acp-event", &event);

        // Buffer initialization event (not part of message streaming)
        let event_value = serde_json::to_value(&event).unwrap_or_default();
        if let Err(e) = file_logger.check_and_flush_acp(&event_value).await {
            tracing::warn!(error = %e, "Failed to buffer ACP event");
        }

        // Emit a connection status event
        let status_event = AcpEvent {
            workspace_id: workspace_id.clone(),
            message: json!({
                "type": "acp/status",
                "status": "connected",
                "workspaceId": workspace_id,
            }),
        };
        let _ = app.emit("acp-event", &status_event);

        // Buffer status event (not part of message streaming)
        let event_value = serde_json::to_value(&status_event).unwrap_or_default();
        if let Err(e) = file_logger.check_and_flush_acp(&event_value).await {
            tracing::warn!(error = %e, "Failed to buffer ACP event");
        }

        tracing::info!(
            workspace_id = %workspace_id,
            "ACP connection initialized"
        );

        Ok(Self {
            workspace_id,
            child: Arc::new(Mutex::new(child)),
            stdin: stdin_arc,
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(2)), // Start at 2 since 1 was used for init
            app,
            session_id: session_id_clone,
            file_logger,
            request_to_session,
            event_callback: event_callback_for_stdout_clone,
        })
    }

    /// Get the current ACP session ID
    pub async fn get_session_id(&self) -> Option<String> {
        self.session_id.lock().await.clone()
    }

    /// Set callback for forwarding events to unified stream
    pub async fn set_event_callback(&self, callback: AcpEventCallback) {
        *self.event_callback.lock().await = Some(callback);
    }

    /// Send a JSON-RPC request to codex-acp
    pub async fn send_request(&self, method: &str, params: Value) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Handle null params - convert to empty object or omit
        let params_value = if params.is_null() {
            json!({})
        } else {
            params.clone()
        };

        // Track session ID for this request (if it's session/prompt)
        if method == "session/prompt"
            && let Some(session_id) = params_value.get("sessionId").and_then(|s| s.as_str()) {
                let mut req_map = self.request_to_session.lock().await;
                req_map.insert(id, session_id.to_string());
                tracing::debug!(
                    request_id = %id,
                    session_id = %session_id,
                    "Tracking request id for session"
                );
            }

        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params_value,
        });

        let message_json = serde_json::to_string(&message)?;

        // Write to stdin
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(message_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        // Emit the outgoing message
        let event = AcpEvent {
            workspace_id: self.workspace_id.clone(),
            message: json!({
                "type": "acp/raw_message",
                "direction": "outgoing",
                "message": message,
            }),
        };
        let _ = self.app.emit("acp-event", &event);

        // Buffer outgoing event
        let event_value = serde_json::to_value(&event).unwrap_or_default();
        if let Err(e) = self.file_logger.check_and_flush_acp(&event_value).await {
            tracing::warn!(error = %e, "Failed to buffer ACP event");
        }

        Ok(())
    }

    /// Send a JSON-RPC notification to codex-acp
    #[expect(dead_code)]
    pub async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        let message = if let Some(params) = params {
            json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            })
        } else {
            json!({
                "jsonrpc": "2.0",
                "method": method,
            })
        };

        let message_json = serde_json::to_string(&message)?;

        // Write to stdin
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(message_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        // Emit the outgoing message
        let event = AcpEvent {
            workspace_id: self.workspace_id.clone(),
            message: json!({
                "type": "acp/raw_message",
                "direction": "outgoing",
                "message": message,
            }),
        };
        let _ = self.app.emit("acp-event", &event);

        // Buffer outgoing event
        let event_value = serde_json::to_value(&event).unwrap_or_default();
        if let Err(e) = self.file_logger.check_and_flush_acp(&event_value).await {
            tracing::warn!(error = %e, "Failed to buffer ACP event");
        }

        Ok(())
    }

    /// Kill the ACP process
    pub async fn kill(&self) -> Result<()> {
        let mut child = self.child.lock().await;
        child.kill().await.context("Failed to kill ACP agent")?;
        Ok(())
    }
}
