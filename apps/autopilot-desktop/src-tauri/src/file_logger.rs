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
    acp_buffer: Arc<Mutex<HashMap<String, Vec<Value>>>>, // session_id -> events
}

impl FileLogger {
    /// Create a new file logger, creating tmp directory if needed
    pub async fn new() -> Result<Self> {
        // Create tmp directory in project root
        let tmp_dir = get_tmp_dir()?;
        std::fs::create_dir_all(&tmp_dir)
            .context("Failed to create tmp directory")?;

        // Create file paths with timestamps
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let app_server_path = tmp_dir.join(format!("app-server-events_{}.jsonl", timestamp));
        let acp_path = tmp_dir.join(format!("acp-events_{}.jsonl", timestamp));

        eprintln!("Logging app-server events to: {}", app_server_path.display());
        eprintln!("Logging ACP events to: {}", acp_path.display());

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
    pub async fn buffer_app_server_event(&self, event: Value, thread_id: Option<String>) {
        let key = thread_id.unwrap_or_else(|| "default".to_string());
        let mut buffer = self.app_server_buffer.lock().await;
        let count = buffer.entry(key.clone()).or_insert_with(Vec::new).len();
        buffer.get_mut(&key).unwrap().push(event);
        eprintln!("Buffered app-server event for thread '{}' (total: {})", key, count + 1);
    }

    /// Buffer an ACP event (will be flushed when message completes)
    pub async fn buffer_acp_event(&self, event: Value, session_id: Option<String>) {
        let key = session_id.unwrap_or_else(|| "default".to_string());
        let mut buffer = self.acp_buffer.lock().await;
        buffer.entry(key).or_insert_with(Vec::new).push(event);
    }

    /// Flush buffered events for a thread (called when message completes)
    pub async fn flush_app_server_events(&self, thread_id: Option<String>) -> Result<()> {
        let key = thread_id.unwrap_or_else(|| "default".to_string());
        let events = {
            let mut buffer = self.app_server_buffer.lock().await;
            buffer.remove(&key).unwrap_or_default()
        };

        if events.is_empty() {
            eprintln!("No events to flush for thread: {}", key);
            return Ok(());
        }

        eprintln!("FLUSHING {} events for thread: {}", events.len(), key);
        let mut writer = self.app_server_writer.lock().await;
        for event in events {
            let json = serde_json::to_string(&event)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        writer.flush().await?;
        eprintln!("Successfully flushed events for thread: {}", key);
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

        eprintln!("Flushing all {} thread buffers", all_events.len());
        let mut writer = self.app_server_writer.lock().await;
        for (thread_id, events) in all_events {
            eprintln!("Flushing {} events for thread: {}", events.len(), thread_id);
            for event in events {
                let json = serde_json::to_string(&event)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        writer.flush().await?;
        eprintln!("Successfully flushed all app-server events");
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

        eprintln!("Flushing all {} ACP session buffers", all_events.len());
        let mut writer = self.acp_writer.lock().await;
        for (session_id, events) in all_events {
            eprintln!("Flushing {} events for ACP session: {}", events.len(), session_id);
            for event in events {
                let json = serde_json::to_string(&event)?;
                writer.write_all(json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
            }
        }
        writer.flush().await?;
        eprintln!("Successfully flushed all ACP events");
        Ok(())
    }

    /// Check if an event indicates message completion and flush if so
    pub async fn check_and_flush_app_server(&self, event: &Value) -> Result<()> {
        // AppServerEvent has structure: { workspace_id, message: { method, params, ... } }
        let message = event.get("message").or_else(|| Some(event)); // Fallback to event itself if no message wrapper
        
        // Check for completion indicators
        let should_flush = if let Some(msg) = message {
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                eprintln!("Checking completion for method: {}", method);
                // Common completion methods - FIXED: use turn/completed (not turn/complete)
                let is_complete = method == "turn/completed" ||
                method.contains("completed") || 
                method.contains("finished") ||
                method == "turn/end" ||
                method == "turn/complete" ||
                (method == "session/update" && msg.get("params")
                    .and_then(|p| p.get("update"))
                    .and_then(|u| u.get("status"))
                    .and_then(|s| s.as_str())
                    .map(|s| s == "complete" || s == "finished")
                    .unwrap_or(false));
                if is_complete {
                    eprintln!("COMPLETION DETECTED for method: {}", method);
                }
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
            .or_else(|| event.get("params")
                .and_then(|p| p.get("threadId").or_else(|| p.get("thread_id"))))
            .and_then(|t| t.as_str())
            .map(|s| {
                eprintln!("Extracted thread_id: {}", s);
                s.to_string()
            });

        // Always buffer the event first
        self.buffer_app_server_event(event.clone(), thread_id.clone()).await;

        if should_flush {
            // Flush all events for this thread when completion detected
            if let Err(e) = self.flush_app_server_events(thread_id.clone()).await {
                eprintln!("Failed to flush app-server events for thread {:?}: {}", thread_id, e);
            }
        }

        Ok(())
    }

    /// Check if an ACP event indicates message completion and flush if so
    pub async fn check_and_flush_acp(&self, event: &Value) -> Result<()> {
        // AcpEvent has structure: { workspace_id, message: { type, direction, message: {...} } }
        let inner_msg = event.get("message")
            .and_then(|m| m.get("message")); // The actual JSON-RPC message inside
        
        // Extract session_id first (needed for buffering)
        // Try multiple locations: params.sessionId, result.sessionId, or from previous session/new response
        let session_id = inner_msg
            .and_then(|m| {
                // Check params.sessionId first (for notifications like session/update)
                m.get("params")
                    .and_then(|p| p.get("sessionId"))
                    .or_else(|| {
                        // Check result.sessionId for responses
                        m.get("result")
                            .and_then(|r| r.get("sessionId"))
                    })
            })
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());

        // Always buffer the event first
        self.buffer_acp_event(event.clone(), session_id.clone()).await;

        // Check for completion indicators in ACP events
        // NOTE: session/update is NOT a completion event - it's just a status update
        if let Some(msg) = inner_msg {
            // Check for stopReason: "end_turn" at top level (ACP completion indicator in responses)
            // This appears in responses to session/prompt requests (id: 4 in your console)
            if let Some(stop_reason) = msg.get("stopReason").and_then(|s| s.as_str()) {
                if stop_reason == "end_turn" {
                    eprintln!("ACP COMPLETION DETECTED: stopReason=end_turn (response to session/prompt)");
                    // Extract session_id from the request context or use default
                    // For responses, we need to track which session the request was for
                    // For now, flush all buffered events (we'll improve session tracking later)
                    return self.flush_all_acp_events().await;
                }
            }
            
            // Check if this is a response (has "id" but no "method") - might have stopReason
            if msg.get("id").is_some() && msg.get("method").is_none() {
                if let Some(stop_reason) = msg.get("stopReason").and_then(|s| s.as_str()) {
                    if stop_reason == "end_turn" {
                        eprintln!("ACP COMPLETION DETECTED: stopReason=end_turn in response");
                        return self.flush_all_acp_events().await;
                    }
                }
            }
            
            if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                // session/update is never a completion event, just buffer it (already done above)
                if method == "session/update" {
                    return Ok(()); // Don't flush on session/update
                }
                
                // Only flush on actual completion methods
                let is_complete = method.contains("completed") || 
                method.contains("finished") ||
                method == "turn/end" ||
                method == "turn/complete";
                
                if is_complete {
                    eprintln!("ACP COMPLETION DETECTED for method: {}", method);
                    return self.flush_acp_events(session_id).await;
                }
            }
        }

        Ok(())
    }
}

/// Get the tmp directory path (project root / tmp)
fn get_tmp_dir() -> Result<PathBuf> {
    // Try to get project root from CARGO_MANIFEST_DIR or current working directory
    let project_root = if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        // CARGO_MANIFEST_DIR points to src-tauri, so go up one level to get project root
        let mut path = PathBuf::from(manifest_dir);
        path.pop(); // Remove src-tauri to get project root
        eprintln!("Using CARGO_MANIFEST_DIR, project root: {}", path.display());
        path
    } else {
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        eprintln!("Using current directory as project root: {}", cwd.display());
        cwd
    };

    let tmp_dir = project_root.join("tmp");
    eprintln!("Tmp directory will be: {}", tmp_dir.display());
    Ok(tmp_dir)
}
