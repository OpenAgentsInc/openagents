//! Claude Code CLI process management and stream forwarder.
//!
//! Spawns the Claude Code CLI and forwards its JSON event stream through the
//! in-repo translator to ACP `SessionUpdate`s, then mirrors into Convex.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;
use agent_client_protocol::{SessionUpdate, ContentChunk, ContentBlock, TextContent};

use crate::state::AppState;
use std::sync::Arc;

/// Wrapper for a spawned Claude child process with stdout/stderr handles.
pub struct ClaudeChild {
    #[allow(dead_code)]
    pub pid: u32,
    pub stdout: Option<tokio::process::ChildStdout>,
    pub stderr: Option<tokio::process::ChildStderr>,
}

/// Spawn a short‑lived Claude child for one prompt.
pub async fn spawn_claude_child_with_prompt(
    opts: &crate::Opts,
    workdir_override: Option<PathBuf>,
    prompt: &str,
) -> Result<ClaudeChild> {
    let (bin, mut args) = build_bin_and_args(opts)?;
    // Insert prompt right after -p/--print when present (per headless docs),
    // otherwise append as positional argument.
    let mut placed = false;
    for i in 0..args.len() {
        if args[i] == "-p" || args[i] == "--print" {
            args.insert(i + 1, prompt.to_string());
            placed = true;
            break;
        }
    }
    if !placed { args.push(prompt.to_string()); }
    let workdir = workdir_override.unwrap_or_else(|| detect_repo_root(None));
    info!("bin" = %bin.display(), "args" = ?args, "workdir" = %workdir.display(), "msg" = "spawn claude with positional prompt");
    let mut command = Command::new(&bin);
    command
        .current_dir(&workdir)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            let res = libc::setpgid(0, 0);
            if res != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("failed to spawn claude")?;
    let pid = child.id().context("claude child pid missing")?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => tracing::info!(?status, "claude exited"),
            Err(e) => tracing::error!(?e, "claude wait failed"),
        }
    });
    Ok(ClaudeChild { pid, stdout, stderr })
}

fn build_bin_and_args(opts: &crate::Opts) -> Result<(PathBuf, Vec<String>)> {
    let bin = resolve_claude_bin(opts)?;
    if let Some(cli_args) = opts.claude_args.clone() {
        let args = cli_args.split_whitespace().map(|s| s.to_string()).collect();
        return Ok((bin, args));
    }
    // Default headless per docs: non-interactive print with streaming JSON output.
    // We'll parse each stdout line as JSON.
    let args = vec!["-p".to_string(), "--output-format".to_string(), "stream-json".to_string()];
    Ok((bin, args))
}

fn resolve_claude_bin(opts: &crate::Opts) -> Result<PathBuf> {
    if let Some(bin) = opts.claude_bin.clone() { return Ok(bin); }
    if let Ok(env) = std::env::var("CLAUDE_BIN") { return Ok(PathBuf::from(env)); }
    which::which("claude").map_err(|e| anyhow::anyhow!("claude binary not found in PATH: {e}"))
}

fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &Path) -> bool { p.join("expo").is_dir() && p.join("crates").is_dir() }
    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) { return cur; }
        if let Some(parent) = cur.parent() { cur = parent.to_path_buf(); } else { break; }
    }
    original
}

/// Read Claude stdout lines, translate to ACP, and mirror into Convex.
pub async fn start_claude_forwarders(mut child: ClaudeChild, state: Arc<AppState>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;
    let tx_out = state.tx.clone();

    // stderr task (surface errors to clients as structured events)
    let tx_err = state.tx.clone();
    let state_err = state.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let s = line.trim();
            if s.is_empty() { continue; }
            // Emit a generic error envelope so clients can render it
            let _ = tx_err.send(
                serde_json::json!({
                    "type": "error",
                    "provider": "claude_code",
                    "message": s,
                })
                .to_string(),
            );
            // Also emit as an ACP agent_message so the thread UI shows it
            let update = SessionUpdate::AgentMessageChunk(ContentChunk {
                content: ContentBlock::Text(TextContent { annotations: None, text: s.to_string(), meta: None }),
                meta: None,
            });
            let target_tid = {
                if let Some(ctid) = state_err.current_convex_thread.lock().await.clone() { ctid } else { state_err.last_thread_id.lock().await.clone().unwrap_or_default() }
            };
            if !target_tid.is_empty() {
                crate::tinyvex_write::mirror_acp_update_to_convex(&state_err, &target_tid, &update).await;
            }
            if let Ok(line) = serde_json::to_string(&serde_json::json!({"type":"bridge.acp","notification":{"sessionId": target_tid, "update": update}})) {
                let _ = tx_err.send(line);
            }
        }
    });

    // stdout task: JSON events
    let state_for = state.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let s = line.trim();
            if s.is_empty() { continue; }
            if let Ok(v) = serde_json::from_str::<JsonValue>(s) {
                    if let Some(update) = acp_event_translator::translate_claude_event_to_acp_update(&v) {
                    // Debug: emit a concise marker for tests
                    let kind = match &update {
                        agent_client_protocol::SessionUpdate::UserMessageChunk(_) => "user_message_chunk",
                        agent_client_protocol::SessionUpdate::AgentMessageChunk(_) => "agent_message_chunk",
                        agent_client_protocol::SessionUpdate::AgentThoughtChunk(_) => "agent_thought_chunk",
                        agent_client_protocol::SessionUpdate::ToolCall(_) => "tool_call",
                        agent_client_protocol::SessionUpdate::ToolCallUpdate(_) => "tool_call_update",
                        agent_client_protocol::SessionUpdate::Plan(_) => "plan",
                        agent_client_protocol::SessionUpdate::AvailableCommandsUpdate(_) => "available_commands_update",
                        agent_client_protocol::SessionUpdate::CurrentModeUpdate(_) => "current_mode_update",
                    };
                    let _ = tx_out.send(serde_json::json!({"type":"bridge.acp_seen","kind": kind}).to_string());
                    // Determine thread id target from current_convex_thread
                    let target_tid = {
                        if let Some(ctid) = state_for.current_convex_thread.lock().await.clone() { ctid } else { state_for.last_thread_id.lock().await.clone().unwrap_or_default() }
                    };
                    if !target_tid.is_empty() {
                        crate::tinyvex_write::mirror_acp_update_to_convex(&state_for, &target_tid, &update).await;
                    }
                    if let Ok(line) = serde_json::to_string(&serde_json::json!({"type":"bridge.acp","notification":{"sessionId": target_tid, "update": update}})) { let _ = tx_out.send(line); }
                }
            } else {
                // Non-JSON line on stdout: surface as an error so the UI shows it
                let _ = tx_out.send(
                    serde_json::json!({
                        "type": "error",
                        "provider": "claude_code",
                        "message": s,
                    })
                    .to_string(),
                );
                // Also emit agent message ACP
                let update = SessionUpdate::AgentMessageChunk(ContentChunk {
                    content: ContentBlock::Text(TextContent { annotations: None, text: s.to_string(), meta: None }),
                    meta: None,
                });
                let target_tid = {
                    if let Some(ctid) = state_for.current_convex_thread.lock().await.clone() { ctid } else { state_for.last_thread_id.lock().await.clone().unwrap_or_default() }
                };
                if !target_tid.is_empty() {
                    crate::tinyvex_write::mirror_acp_update_to_convex(&state_for, &target_tid, &update).await;
                }
                if let Ok(line) = serde_json::to_string(&serde_json::json!({"type":"bridge.acp","notification":{"sessionId": target_tid, "update": update}})) { let _ = tx_out.send(line); }
            }
        }
    });
    Ok(())
}
