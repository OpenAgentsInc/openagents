//! Autopilot routes

use std::sync::Arc;

use actix_web::{web, HttpResponse};
use maud::html;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, info};
use ui::acp::atoms::{ToolKind, ToolStatus};
use ui::acp::organisms::{AssistantMessage, ToolCallCard};
use ui::FullAutoSwitch;

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
            let error_html = html! {
                div class="mb-4 p-3 border-l-2 border-red" {
                    span class="text-red text-sm font-mono" { "Failed to start autopilot: " (e.to_string()) }
                }
            };
            state.broadcaster.broadcast(&format!(
                r#"<div id="chat-content-formatted" hx-swap-oob="beforeend">{}</div>"#,
                error_html.into_string()
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

    // Broadcast startup message with ACP styling
    let agent_name = match selected_agent.as_str() {
        "codex" => "Codex",
        _ => "Claude Code",
    };
    let startup_html = html! {
        div class="mb-4 p-3 border-l-2 border-green" {
            span class="text-green text-sm font-mono" { (agent_name) " starting..." }
        }
    };
    broadcaster.broadcast(&format!(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend">{}</div>"#, startup_html.into_string()));

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
                broadcaster.broadcast(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="mb-4 p-3 border-l-2 border-muted"><span class="text-muted-foreground text-sm font-mono">Autopilot stopped.</span></div></div>"#);
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

                                // Format for human-readable ACP view
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
                            }
                            // Invalid JSON is silently ignored - only valid stream-json is displayed
                        } else {
                            // Non-JSON output (rare) - show as plain text
                            let escaped = html_escape(&text);
                            let html = format!(
                                r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-1 text-xs text-muted-foreground font-mono">{}</div></div>"#,
                                escaped
                            );
                            broadcaster.broadcast(&html);
                        }
                    }
                    Ok(None) => {
                        // EOF - process exited
                        broadcaster.broadcast(r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="mb-4 p-3 border-l-2 border-muted"><span class="text-muted-foreground text-sm font-mono">Autopilot process exited.</span></div></div>"#);
                        break;
                    }
                    Err(e) => {
                        broadcaster.broadcast(&format!(
                            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="mb-4 p-3 border-l-2 border-red"><span class="text-red text-sm font-mono">Read error: {}</span></div></div>"#,
                            e
                        ));
                        break;
                    }
                }
            }

            line = stderr_reader.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        // Show stderr as error message in formatted view
                        let escaped = html_escape(&text);
                        broadcaster.broadcast(&format!(
                            r#"<div id="chat-content-formatted" hx-swap-oob="beforeend"><div class="px-3 py-1 text-xs text-red font-mono">{}</div></div>"#,
                            escaped
                        ));
                    }
                    Ok(None) | Err(_) => {
                        // EOF or error on stderr is normal
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

/// Map Claude tool names to ACP ToolKind
fn tool_name_to_kind(name: &str) -> ToolKind {
    match name.to_lowercase().as_str() {
        "read" | "read_file" | "view" => ToolKind::Read,
        "edit" | "edit_file" | "write" | "write_file" | "str_replace_editor" => ToolKind::Edit,
        "delete" | "delete_file" | "rm" => ToolKind::Delete,
        "bash" | "execute" | "run" | "shell" | "terminal" | "command" => ToolKind::Execute,
        "grep" | "search" | "glob" | "find" | "ripgrep" | "list_files" => ToolKind::Search,
        "think" | "thinking" | "reason" => ToolKind::Think,
        "fetch" | "web" | "http" | "curl" | "web_search" | "webfetch" => ToolKind::Fetch,
        "todowrite" | "task" | "todo" => ToolKind::Other,
        _ => ToolKind::Other,
    }
}

/// Format a Claude Code stream-json event for the human-readable view using ACP components
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
                        let entry_id = format!("assistant-{}", chrono::Utc::now().timestamp_millis());
                        let msg = AssistantMessage::new(&entry_id)
                            .text(html_escape(&text))
                            .build();
                        return StreamingOutput::Block(msg.into_string());
                    }
                }
            }
            StreamingOutput::None
        }
        "content_block_start" => {
            // Start of a new content block - create streaming container
            let index = json.get("index").and_then(|i| i.as_u64()).unwrap_or(0);

            // Check if this is a tool_use block
            if let Some(content_block) = json.get("content_block") {
                if let Some(block_type) = content_block.get("type").and_then(|t| t.as_str()) {
                    if block_type == "tool_use" {
                        let name = content_block.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                        let tool_kind = tool_name_to_kind(name);
                        let entry_id = format!("tool-{}", index);
                        let card = ToolCallCard::new(tool_kind, name, &entry_id)
                            .status(ToolStatus::Running)
                            .build();
                        return StreamingOutput::Block(card.into_string());
                    }
                }
            }

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
            // Tool use result - render with ACP status badge style
            if let Some(subtype) = json.get("subtype").and_then(|v| v.as_str()) {
                let result_html = html! {
                    div class="ml-4 mb-2 py-1 text-xs font-mono" {
                        @if subtype == "success" {
                            span class="text-green" { "[+] Success" }
                        } @else if subtype == "error" {
                            @let error = json.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error");
                            span class="text-red" { "[x] Error: " (error) }
                        } @else {
                            span class="text-muted-foreground" { "[*] " (subtype) }
                        }
                    }
                };
                return StreamingOutput::Block(result_html.into_string());
            }
            StreamingOutput::None
        }
        "tool_use" => {
            // Tool invocation - use ACP ToolCallCard
            if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                let tool_kind = tool_name_to_kind(name);
                let tool_id = json.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");
                let entry_id = format!("tool-{}", tool_id);

                // Get input if available for display
                let input_preview = json.get("input")
                    .map(|i| {
                        let s = i.to_string();
                        if s.len() > 200 { format!("{}...", &s[..200]) } else { s }
                    });

                let mut card = ToolCallCard::new(tool_kind, name, &entry_id)
                    .status(ToolStatus::Running);

                if let Some(preview) = input_preview {
                    card = card.content(html_escape(&preview));
                }

                return StreamingOutput::Block(card.build().into_string());
            }
            StreamingOutput::None
        }
        "user" => {
            // User message
            if let Some(message) = json.get("message") {
                if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                    let user_html = html! {
                        div class="mb-4 p-3 bg-card border border-border" {
                            div class="text-xs text-muted-foreground mb-1 font-mono" { "[USER]" }
                            div class="text-sm text-foreground whitespace-pre-wrap" { (content) }
                        }
                    };
                    return StreamingOutput::Block(user_html.into_string());
                }
            }
            StreamingOutput::None
        }
        "system" => {
            // System message
            if let Some(content) = json.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_str()) {
                let sys_html = html! {
                    div class="mb-2 px-3 py-1 text-xs text-muted-foreground font-mono border-l-2 border-muted" {
                        "[SYSTEM] " (if content.len() > 100 { &content[..100] } else { content }) "..."
                    }
                };
                return StreamingOutput::Block(sys_html.into_string());
            }
            StreamingOutput::None
        }
        _ => StreamingOutput::None,
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
