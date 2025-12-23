//! Autopilot routes

use std::sync::Arc;

use actix_web::{web, HttpResponse};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};
use ui::{FullAutoSwitch, render_line_oob};

use crate::gui::state::{AppState, AutopilotProcess};
use crate::gui::ws::WsBroadcaster;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(dashboard))
        .route("/sessions", web::get().to(sessions_page))
        .route("/metrics", web::get().to(metrics_page));
}

/// Configure API routes (called from main routes)
///
/// Note: Auth removed from toggle - this is a local app, webview is the only client.
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.route("/toggle", web::post().to(toggle_full_auto))
        .route("/status", web::get().to(get_status));
}

/// Toggle full auto mode and return new switch HTML
async fn toggle_full_auto(state: web::Data<AppState>) -> HttpResponse {
    info!("POST /api/autopilot/toggle called");
    let mut full_auto = state.full_auto.write().await;
    *full_auto = !*full_auto;
    let new_state = *full_auto;
    info!("Full auto toggled to: {}", new_state);
    drop(full_auto);

    if new_state {
        // Spawn autopilot process
        if let Err(e) = spawn_autopilot_process(&state).await {
            tracing::error!("Failed to spawn autopilot: {}", e);
            // Revert state
            *state.full_auto.write().await = false;

            // Broadcast error
            state.broadcaster.broadcast(&format!(
                r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-error">Failed to start autopilot: {}</div></div>"#,
                e
            ));

            let switch = FullAutoSwitch::new(false).build();
            return HttpResponse::InternalServerError()
                .content_type("text/html")
                .body(switch.into_string());
        }
    } else {
        // Stop autopilot process
        stop_autopilot_process(&state).await;
    }

    // Return the new switch HTML for HTMX to swap
    // Also include script to show/hide chat pane
    let switch = FullAutoSwitch::new(new_state).build();
    let script = if new_state {
        r#"<script>document.getElementById('chat-pane').classList.remove('hidden')</script>"#
    } else {
        r#"<script>document.getElementById('chat-pane').classList.add('hidden')</script>"#
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(format!("{}{}", switch.into_string(), script))
}

/// Spawn the autopilot process with output streaming
async fn spawn_autopilot_process(state: &web::Data<AppState>) -> anyhow::Result<()> {
    use std::process::Stdio;

    // Check if already running
    {
        let guard = state.autopilot_process.read().await;
        if guard.is_some() {
            anyhow::bail!("Autopilot already running");
        }
    }

    // Get selected agent
    let selected_agent = state.selected_agent.read().await.clone();

    // Build command based on selected agent
    let mut cmd = match selected_agent.as_str() {
        "codex" => {
            // Use Codex CLI
            let codex_path = find_agent_executable("codex")
                .ok_or_else(|| anyhow::anyhow!("Codex not installed. Run: npm install -g @openai/codex"))?;

            let mut c = Command::new(codex_path);
            c.args([
                "exec",
                "--experimental-json",
                "--sandbox", "workspace-write",
            ]);
            c
        }
        _ => {
            // Default to Claude Code
            let claude_path = find_agent_executable("claude")
                .ok_or_else(|| anyhow::anyhow!("Claude Code not installed. Run: npm install -g @anthropic-ai/claude-code"))?;

            let mut c = Command::new(claude_path);
            c.args([
                "--output-format", "stream-json",
                "--verbose",
                "--permission-mode", "bypassPermissions",
                "-p",
                "Begin autonomous work. Call issue_ready to get the first issue, or if none exist, review the active directives and create issues to advance them."
            ]);
            c
        }
    };

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let broadcaster = state.broadcaster.clone();

    // Broadcast startup message to all views with agent name
    let agent_name = match selected_agent.as_str() {
        "codex" => "Codex",
        _ => "Claude Code",
    };
    broadcaster.broadcast(&format!(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-line log-success">{} starting...</div></div>"#, agent_name));
    broadcaster.broadcast(&format!(r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line" style="color: var(--color-green);">{} starting...</div></div>"#, agent_name));
    broadcaster.broadcast(&format!(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-2 text-green text-sm">{} starting...</div></div>"#, agent_name));

    // Spawn output reader task
    let output_task = tokio::spawn(async move {
        read_output_loop(stdout, stderr, broadcaster, &mut shutdown_rx).await;
    });

    // Store in state
    let autopilot = AutopilotProcess {
        child,
        output_task,
        shutdown_tx,
    };
    *state.autopilot_process.write().await = Some(autopilot);

    Ok(())
}

/// Read stdout/stderr and broadcast to WebSocket
async fn read_output_loop(
    stdout: tokio::process::ChildStdout,
    stderr: tokio::process::ChildStderr,
    broadcaster: Arc<WsBroadcaster>,
    shutdown_rx: &mut tokio::sync::mpsc::Receiver<()>,
) {
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                broadcaster.broadcast(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-line" style="color: #888;">Autopilot stopped.</div></div>"#);
                broadcaster.broadcast(r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line" style="color: #888;">Autopilot stopped.</div></div>"#);
                broadcaster.broadcast(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-2 text-muted-foreground text-sm">Autopilot stopped.</div></div>"#);
                break;
            }

            line = stdout_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        // Check if this is a JSON event (stream-json format starts with {)
                        if text.starts_with('{') {
                            // Parse JSON and extract useful info for formatted view
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                // Log the event type for debugging
                                let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
                                debug!("Claude event: type={}", event_type);

                                // Broadcast to JSON view (full JSON)
                                let escaped = html_escape(&text);
                                let json_html = format!(r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line">{}</div></div>"#, escaped);
                                broadcaster.broadcast(&json_html);

                                // Format for human-readable view
                                let formatted = format_claude_event(&json);
                                match formatted {
                                    StreamingOutput::Block(html) => {
                                        let formatted_html = format!(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend">{}</div>"#, html);
                                        broadcaster.broadcast(&formatted_html);
                                    }
                                    StreamingOutput::StartBlock { index } => {
                                        let html = format!(
                                            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div id="stream-block-{}" class="px-3 py-2 text-sm text-foreground whitespace-pre-wrap"></div></div>"#,
                                            index
                                        );
                                        broadcaster.broadcast(&html);
                                    }
                                    StreamingOutput::Delta { index, text } => {
                                        let escaped = html_escape(&text);
                                        let html = format!(
                                            r#"<div id="stream-block-{}" hx-swap-oob="beforeend">{}</div>"#,
                                            index, escaped
                                        );
                                        broadcaster.broadcast(&html);
                                    }
                                    StreamingOutput::EndBlock => {
                                        // No action needed when block ends
                                    }
                                    StreamingOutput::None => {}
                                }

                                // Also show key events in raw view
                                let raw_summary = summarize_claude_event(&json);
                                if !raw_summary.is_empty() {
                                    let raw_html = format!(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-line">{}</div></div>"#, html_escape(&raw_summary));
                                    broadcaster.broadcast(&raw_html);
                                }
                            } else {
                                // Invalid JSON, show as-is
                                let escaped = html_escape(&text);
                                let json_html = format!(r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line">{}</div></div>"#, escaped);
                                broadcaster.broadcast(&json_html);
                            }
                        } else {
                            // Regular rlog output - broadcast to raw RLOG view
                            let escaped = html_escape(&text);
                            let raw_html = format!(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-line">{}</div></div>"#, escaped);
                            broadcaster.broadcast(&raw_html);

                            // Broadcast to formatted view (rendered components)
                            let formatted_html = render_line_oob(&text);
                            broadcaster.broadcast(&formatted_html);
                        }
                    }
                    Ok(None) => {
                        // EOF - process exited
                        broadcaster.broadcast(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-error">Autopilot process exited.</div></div>"#);
                        broadcaster.broadcast(r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line" style="color: var(--color-red);">Autopilot process exited.</div></div>"#);
                        broadcaster.broadcast(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-2 text-red text-sm">Autopilot process exited.</div></div>"#);
                        break;
                    }
                    Err(e) => {
                        broadcaster.broadcast(&format!(
                            r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-error">Read error: {}</div></div>"#,
                            e
                        ));
                        broadcaster.broadcast(&format!(
                            r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line" style="color: var(--color-red);">Read error: {}</div></div>"#,
                            e
                        ));
                        broadcaster.broadcast(&format!(
                            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-2 text-red text-sm">Read error: {}</div></div>"#,
                            e
                        ));
                        break;
                    }
                }
            }

            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        let escaped = html_escape(&text);
                        let raw_html = format!(r#"<div id="chat-content-raw" hx-swap-oob="beforeend"><div class="log-error">{}</div></div>"#, escaped);
                        broadcaster.broadcast(&raw_html);

                        // Also show in JSON view as error
                        let json_html = format!(
                            r#"<div id="chat-content-json" hx-swap-oob="beforeend"><div class="json-line" style="color: var(--color-red);">{}</div></div>"#,
                            escaped
                        );
                        broadcaster.broadcast(&json_html);

                        // Also show in formatted view as error
                        let formatted_html = format!(
                            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-1 text-xs text-red font-mono">{}</div></div>"#,
                            escaped
                        );
                        broadcaster.broadcast(&formatted_html);
                    }
                    Ok(None) => {
                        // EOF on stderr is normal
                    }
                    Err(_) => {
                        // Ignore stderr read errors
                    }
                }
            }
        }
    }
}

/// Stop the autopilot process gracefully
async fn stop_autopilot_process(state: &web::Data<AppState>) {
    let mut guard = state.autopilot_process.write().await;

    if let Some(mut autopilot) = guard.take() {
        // Signal shutdown to output reader
        let _ = autopilot.shutdown_tx.send(()).await;

        // Kill the process (sends SIGKILL on Unix, TerminateProcess on Windows)
        let _ = autopilot.child.kill().await;
        let _ = autopilot.child.wait().await;

        // Wait for output task to finish
        let _ = autopilot.output_task.await;
    }
}

/// Get autopilot process status
async fn get_status(state: web::Data<AppState>) -> HttpResponse {
    let guard = state.autopilot_process.read().await;
    let full_auto = *state.full_auto.read().await;

    let status = serde_json::json!({
        "full_auto": full_auto,
        "running": guard.is_some(),
    });

    HttpResponse::Ok().json(status)
}

/// Escape HTML special characters
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Represents what kind of streaming output to send
enum StreamingOutput {
    /// Start a new streaming text block (creates container)
    StartBlock { index: u64 },
    /// Delta text to append to current block
    Delta { index: u64, text: String },
    /// End the current streaming block
    EndBlock,
    /// A complete HTML block (tool use, result, etc.)
    Block(String),
    /// Nothing to output
    None,
}

/// Format a Claude Code stream-json event for the human-readable view
fn format_claude_event(json: &serde_json::Value) -> StreamingOutput {
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "assistant" => {
            // Complete assistant message (fallback when not streaming)
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                    let mut text_parts: Vec<String> = Vec::new();
                    for item in content {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    if !text_parts.is_empty() {
                        let text = text_parts.join("");
                        return StreamingOutput::Block(format!(
                            r#"<div class="px-3 py-2 text-sm text-foreground whitespace-pre-wrap">{}</div>"#,
                            html_escape(&text)
                        ));
                    }
                }
            }
            StreamingOutput::None
        }
        "content_block_start" => {
            // Start of a new content block - create streaming container
            let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
            StreamingOutput::StartBlock { index }
        }
        "content_block_delta" => {
            // Streaming text delta - append to container
            let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
            if let Some(delta) = json.get("delta") {
                // Handle both "text" and nested structure
                let text = delta.get("text").and_then(|t| t.as_str())
                    .or_else(|| delta.get("partial_json").and_then(|t| t.as_str()));
                if let Some(text) = text {
                    if !text.is_empty() {
                        return StreamingOutput::Delta { index, text: text.to_string() };
                    }
                }
            }
            StreamingOutput::None
        }
        "content_block_stop" => {
            StreamingOutput::EndBlock
        }
        "result" => {
            // Tool use result
            if let Some(subtype) = json.get("subtype").and_then(|v| v.as_str()) {
                if subtype == "success" {
                    return StreamingOutput::Block(
                        r#"<div class="px-3 py-1 text-xs text-green">✓ Tool executed successfully</div>"#.to_string()
                    );
                } else if subtype == "error" {
                    let error = json.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error");
                    return StreamingOutput::Block(format!(
                        r#"<div class="px-3 py-1 text-xs text-red">✗ Error: {}</div>"#,
                        html_escape(error)
                    ));
                }
            }
            StreamingOutput::None
        }
        "tool_use" => {
            // Tool invocation
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                return StreamingOutput::Block(format!(
                    r#"<div class="px-3 py-1 text-xs text-yellow">🔧 Tool: {}</div>"#,
                    html_escape(name)
                ));
            }
            StreamingOutput::None
        }
        _ => StreamingOutput::None,
    }
}

/// Summarize a Claude Code stream-json event for the raw view
fn summarize_claude_event(json: &serde_json::Value) -> String {
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match event_type {
        "assistant" => "[assistant message]".to_string(),
        "content_block_start" => "[content block start]".to_string(),
        "content_block_delta" => String::new(), // Skip deltas in raw view
        "content_block_stop" => "[content block stop]".to_string(),
        "message_start" => "[message start]".to_string(),
        "message_delta" => "[message delta]".to_string(),
        "message_stop" => "[message stop]".to_string(),
        "result" => {
            let subtype = json.get("subtype").and_then(|v| v.as_str()).unwrap_or("unknown");
            format!("[result: {}]", subtype)
        }
        "tool_use" => {
            let name = json.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
            format!("[tool: {}]", name)
        }
        "system" => "[system]".to_string(),
        "user" => "[user]".to_string(),
        other if !other.is_empty() => format!("[{}]", other),
        _ => String::new(),
    }
}

async fn dashboard() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Autopilot</h1><p>Coming soon...</p>")
}

async fn sessions_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Sessions</h1><p>Coming soon...</p>")
}

async fn metrics_page() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body("<h1>Metrics</h1><p>Coming soon...</p>")
}

/// Find an agent executable by name
fn find_agent_executable(name: &str) -> Option<std::path::PathBuf> {
    // Try which first
    if let Ok(path) = which::which(name) {
        return Some(path);
    }

    // Try common locations
    let home = std::env::var("HOME").ok()?;
    let paths = match name {
        "claude" => vec![
            format!("{}/.claude/local/claude", home),
            format!("{}/.npm-global/bin/claude", home),
            format!("{}/.local/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            "/opt/homebrew/bin/claude".to_string(),
        ],
        "codex" => vec![
            format!("{}/.npm-global/bin/codex", home),
            format!("{}/.local/bin/codex", home),
            "/usr/local/bin/codex".to_string(),
            "/opt/homebrew/bin/codex".to_string(),
        ],
        _ => return None,
    };

    for path in &paths {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}
