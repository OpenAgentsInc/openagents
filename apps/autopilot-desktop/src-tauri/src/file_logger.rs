use anyhow::{Context, Result};
// No serde imports needed - we work with Value directly
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;

/// File logger that buffers events and writes them when a message completes
pub struct FileLogger {
    app_server_writer: Arc<Mutex<BufWriter<File>>>,
    acp_writer: Arc<Mutex<BufWriter<File>>>,
    app_server_buffer: Arc<Mutex<HashMap<String, Vec<Value>>>>, // thread_id -> events
    acp_buffer: Arc<Mutex<HashMap<String, Vec<Value>>>>,        // session_id -> events
}

impl FileLogger {
    /// Create a new file logger, creating tmp directory if needed
    pub async fn new() -> Result<Self> {
        // Create tmp directory in project root
        let tmp_dir = get_tmp_dir()?;
        std::fs::create_dir_all(&tmp_dir).context("Failed to create tmp directory")?;

        // Create file paths with timestamps
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let app_server_path = tmp_dir.join(format!("app-server-events_{}.jsonl", timestamp));
        let acp_path = tmp_dir.join(format!("acp-events_{}.jsonl", timestamp));

        if file_logger_verbose() {
            eprintln!(
                "Event logs: app_server={}, acp={}",
                app_server_path.display(),
                acp_path.display()
            );
        }

        // Open files for writing
        let app_server_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&app_server_path)
            .await
            .context("Failed to open app-server events file")?;

        let acp_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&acp_path)
            .await
            .context("Failed to open ACP events file")?;

        let app_server_writer = Arc::new(Mutex::new(BufWriter::new(app_server_file)));
        let acp_writer = Arc::new(Mutex::new(BufWriter::new(acp_file)));

        // Write header lines to ensure files are created immediately
        {
            let mut writer = app_server_writer.lock().await;
            writer.write_all(b"# App-server events log\n").await?;
            writer.flush().await?;
        }
        {
            let mut writer = acp_writer.lock().await;
            writer.write_all(b"# ACP events log\n").await?;
            writer.flush().await?;
        }

        Ok(Self {
            app_server_writer,
            acp_writer,
            app_server_buffer: Arc::new(Mutex::new(HashMap::new())),
            acp_buffer: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Buffer an app-server event (will be flushed when message completes)
    pub async fn buffer_app_server_event(
        &self,
        event: Value,
        thread_id: Option<String>,
    ) -> usize {
        let key = thread_id.unwrap_or_else(|| "default".to_string());
        let mut buffer = self.app_server_buffer.lock().await;
        let events = buffer.entry(key).or_insert_with(Vec::new);
        events.push(event);
        events.len()
    }

    /// Buffer an ACP event (will be flushed when message completes)
    pub async fn buffer_acp_event(&self, event: Value, session_id: Option<String>) -> usize {
        let key = session_id.unwrap_or_else(|| "default".to_string());
        let mut buffer = self.acp_buffer.lock().await;
        let events = buffer.entry(key).or_insert_with(Vec::new);
        events.push(event);
        events.len()
    }

    /// Flush buffered events for a thread (called when message completes)
    pub async fn flush_app_server_events(&self, thread_id: Option<String>) -> Result<()> {
        let key = thread_id.unwrap_or_else(|| "default".to_string());
        let events = {
            let mut buffer = self.app_server_buffer.lock().await;
            buffer.remove(&key).unwrap_or_default()
        };

        if events.is_empty() {
            return Ok(());
        }
        let mut writer = self.app_server_writer.lock().await;
        for event in events {
            let json = serde_json::to_string(&event)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        writer.flush().await?;
        Ok(())
    }

    /// Flush all buffered app-server events (called on disconnect or shutdown)
    #[allow(dead_code)]
    pub async fn flush_all_app_server_events(&self) -> Result<()> {
        let all_events: Vec<(String, Vec<Value>)> = {
            let mut buffer = self.app_server_buffer.lock().await;
            buffer.drain().collect()
        };

        if all_events.is_empty() {
            return Ok(());
        }

        let mut writer = self.app_server_writer.lock().await;
        for (_thread_id, events) in all_events {
            for event in events {
                let json = serde_json::to_string(&event)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        writer.flush().await?;
        Ok(())
    }

    /// Flush buffered events for a session (called when message completes)
    pub async fn flush_acp_events(&self, session_id: Option<String>) -> Result<()> {
        let key = session_id.unwrap_or_else(|| "default".to_string());
        let events = {
            let mut buffer = self.acp_buffer.lock().await;
            buffer.remove(&key).unwrap_or_default()
        };

        if events.is_empty() {
            return Ok(());
        }

        let mut writer = self.acp_writer.lock().await;
        for event in events {
            let json = serde_json::to_string(&event)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        writer.flush().await?;
        Ok(())
    }

    /// Flush all buffered ACP events (called on disconnect or shutdown)
    pub async fn flush_all_acp_events(&self) -> Result<()> {
        let all_events: Vec<(String, Vec<Value>)> = {
            let mut buffer = self.acp_buffer.lock().await;
            buffer.drain().collect()
        };

        if all_events.is_empty() {
            return Ok(());
        }

        let mut writer = self.acp_writer.lock().await;
        for (_session_id, events) in all_events {
            for event in events {
                let json = serde_json::to_string(&event)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        writer.flush().await?;
        Ok(())
    }

    /// Check if an event indicates message completion and flush if so
    pub async fn check_and_flush_app_server(&self, event: &Value) -> Result<()> {
        // AppServerEvent has structure: { workspace_id, message: { method, params, ... } }
        let message = event.get("message").or_else(|| Some(event)); // Fallback to event itself if no message wrapper

        // Check for completion indicators
        let method = message
            .and_then(|msg| msg.get("method").and_then(|m| m.as_str()))
            .unwrap_or("unknown");

        let should_flush = if let Some(msg) = message {
            if !method.is_empty() && method != "unknown" {
                // Common completion methods - FIXED: use turn/completed (not turn/complete)
                let is_complete = method == "turn/completed"
                    || method.contains("completed")
                    || method.contains("finished")
                    || method == "turn/end"
                    || method == "turn/complete"
                    || (method == "session/update"
                        && msg
                            .get("params")
                            .and_then(|p| p.get("update"))
                            .and_then(|u| u.get("status"))
                            .and_then(|s| s.as_str())
                            .map(|s| s == "complete" || s == "finished")
                            .unwrap_or(false));
                is_complete
            } else {
                false
            }
        } else {
            false
        };

        // Extract thread_id from event (check both message.params and event.params)
        // Also check for threadId in nested structures (like turn/started, turn/completed)
        let thread_id = message
            .and_then(|m| {
                // Check params.threadId first
                m.get("params")
                    .and_then(|p| p.get("threadId").or_else(|| p.get("thread_id")))
                    .or_else(|| {
                        // Check params.turn.id for turn events
                        m.get("params")
                            .and_then(|p| p.get("turn"))
                            .and_then(|t| t.get("id"))
                    })
                    .or_else(|| {
                        // Check params.thread.id
                        m.get("params")
                            .and_then(|p| p.get("thread"))
                            .and_then(|t| t.get("id"))
                    })
            })
            .or_else(|| {
                event
                    .get("params")
                    .and_then(|p| p.get("threadId").or_else(|| p.get("thread_id")))
            })
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        // Always buffer the event first
        let buffered = self
            .buffer_app_server_event(event.clone(), thread_id.clone())
            .await;
        if should_log_app_server_event(method, should_flush) {
            let thread_label = thread_id.clone().unwrap_or_else(|| "default".to_string());
            eprintln!(
                "app-server event thread={} method={} buffered={} complete={}",
                thread_label, method, buffered, should_flush
            );
        }

        if should_flush {
            // Flush all events for this thread when completion detected
            if let Err(e) = self.flush_app_server_events(thread_id.clone()).await {
                eprintln!(
                    "Failed to flush app-server events for thread {:?}: {}",
                    thread_id, e
                );
            }
        }

        Ok(())
    }

    /// Check if an ACP event indicates message completion and flush if so
    pub async fn check_and_flush_acp(&self, event: &Value) -> Result<()> {
        // AcpEvent has structure: { workspace_id, message: { type, direction, message: {...} } }
        let inner_msg = event.get("message").and_then(|m| m.get("message")); // The actual JSON-RPC message inside

        // Extract session_id first (needed for buffering)
        // Try multiple locations: params.sessionId, result.sessionId, or from previous session/new response
        let session_id = inner_msg
            .and_then(|m| {
                // Check params.sessionId first (for notifications like session/update)
                m.get("params")
                    .and_then(|p| p.get("sessionId"))
                    .or_else(|| {
                        // Check result.sessionId for responses
                        m.get("result").and_then(|r| r.get("sessionId"))
                    })
            })
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        let buffered = self
            .buffer_acp_event(event.clone(), session_id.clone())
            .await;

        // Check for completion indicators in ACP events
        // NOTE: session/update is NOT a completion event - it's just a status update
        let mut flush_mode = "false";
        if let Some(msg) = inner_msg {
            // Check for stopReason: "end_turn" at top level (ACP completion indicator in responses)
            // This appears in responses to session/prompt requests (id: 4 in your console)
            if let Some(stop_reason) = msg.get("stopReason").and_then(|s| s.as_str()) {
                if stop_reason == "end_turn" {
                    flush_mode = "all";
                    // Extract session_id from the request context or use default
                    // For responses, we need to track which session the request was for
                    // For now, flush all buffered events (we'll improve session tracking later)
                    if should_log_acp_event("response", flush_mode) {
                        eprintln!(
                            "acp event session={} method=response buffered={} complete={}",
                            session_id.clone().unwrap_or_else(|| "default".to_string()),
                            buffered,
                            flush_mode
                        );
                    }
                    return self.flush_all_acp_events().await;
                }
            }

            // Check if this is a response (has "id" but no "method") - might have stopReason
            if msg.get("id").is_some() && msg.get("method").is_none() {
                if let Some(stop_reason) = msg.get("stopReason").and_then(|s| s.as_str()) {
                    if stop_reason == "end_turn" {
                        flush_mode = "all";
                        if should_log_acp_event("response", flush_mode) {
                            eprintln!(
                                "acp event session={} method=response buffered={} complete={}",
                                session_id.clone().unwrap_or_else(|| "default".to_string()),
                                buffered,
                                flush_mode
                            );
                        }
                        return self.flush_all_acp_events().await;
                    }
                }
            }

            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                // session/update is never a completion event, just buffer it (already done above)
                if method == "session/update" {
                    if should_log_acp_event(method, flush_mode) {
                        eprintln!(
                            "acp event session={} method={} buffered={} complete={}",
                            session_id.clone().unwrap_or_else(|| "default".to_string()),
                            method,
                            buffered,
                            flush_mode
                        );
                    }
                    return Ok(()); // Don't flush on session/update
                }

                // Only flush on actual completion methods
                let is_complete = method.contains("completed")
                    || method.contains("finished")
                    || method == "turn/end"
                    || method == "turn/complete";

                if is_complete {
                    flush_mode = "true";
                    if should_log_acp_event(method, flush_mode) {
                        eprintln!(
                            "acp event session={} method={} buffered={} complete={}",
                            session_id.clone().unwrap_or_else(|| "default".to_string()),
                            method,
                            buffered,
                            flush_mode
                        );
                    }
                    return self.flush_acp_events(session_id).await;
                }

                if should_log_acp_event(method, flush_mode) {
                    eprintln!(
                        "acp event session={} method={} buffered={} complete={}",
                        session_id.clone().unwrap_or_else(|| "default".to_string()),
                        method,
                        buffered,
                        flush_mode
                    );
                }
                return Ok(());
            }
        }

        if should_log_acp_event("unknown", flush_mode) {
            eprintln!(
                "acp event session={} method=unknown buffered={} complete={}",
                session_id.unwrap_or_else(|| "default".to_string()),
                buffered,
                flush_mode
            );
        }
        Ok(())
    }
}

/// Get the tmp directory path (project root / tmp)
fn get_tmp_dir() -> Result<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        let cwd_name = cwd.file_name().and_then(|name| name.to_str());
        if cwd_name == Some("src-tauri") {
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join("tmp"));
            }
            candidates.push(cwd.join("tmp"));
        } else {
            candidates.push(cwd.join("tmp"));
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join("tmp"));
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(repo_root) = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .and_then(|path| path.parent())
    {
        candidates.push(repo_root.join("apps").join("autopilot-desktop").join("tmp"));
    }

    let tmp_dir = candidates
        .into_iter()
        .find(|path| path.parent().is_some())
        .context("Failed to resolve tmp directory")?;

    Ok(tmp_dir)
}

fn file_logger_verbose() -> bool {
    std::env::var_os("OA_FILE_LOGGER_VERBOSE").is_some()
}

fn should_log_app_server_event(method: &str, is_complete: bool) -> bool {
    if is_complete {
        return true;
    }
    if file_logger_verbose() {
        return true;
    }
    if method.contains("delta") || method.contains("mcp_") {
        return false;
    }
    matches!(
        method,
        "codex/connected"
            | "thread/started"
            | "turn/started"
            | "turn/completed"
            | "codex/event/task_started"
            | "codex/event/task_complete"
            | "codex/event/user_message"
    )
}

fn should_log_acp_event(method: &str, flush_mode: &str) -> bool {
    if flush_mode != "false" {
        return true;
    }
    if file_logger_verbose() {
        return true;
    }
    matches!(method, "session/prompt" | "turn/started" | "turn/completed")
}
