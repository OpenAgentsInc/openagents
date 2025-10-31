//! WebSocket server handlers and control routing.
//!
//! This module exposes the Axum `/ws` route and contains the socket read/write
//! loops, control message dispatch, and child/stdout forwarder that maps Codex
//! JSONL into Tinyvex writes via `crate::tinyvex_write`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use axum::{extract::State, extract::WebSocketUpgrade, response::IntoResponse};
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use std::collections::HashMap;
use futures::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{error, info};

// Convex bootstrap removed
use crate::codex_runner::{ChildWithIo, spawn_codex_child_with_prompt};
use crate::controls::{ControlCommand, parse_control_command};
use crate::tinyvex_write::{
    finalize_streaming_for_thread, stream_upsert_or_append, summarize_exec_delta_for_log,
};
use crate::projects::Project;
use acp_event_translator::translate_codex_event_to_acp_update;
use agent_client_protocol::{SessionId, SessionNotification};
use crate::state::AppState;
use crate::util::{expand_home, now_ms};
use crate::watchers::SyncCommand;

/// Axum handler for the `/ws` route. Upgrades to a WebSocket and delegates to `handle_socket`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> axum::response::Response {
    // If a WS token is configured, enforce it before upgrading.
    {
        // Always require a token. The expected token is provisioned at startup and stored in state.
        let expected = state.opts.ws_token.clone().unwrap_or_default();
        let supplied = extract_token(&headers, &params);
        if expected.is_empty() || supplied.as_deref() != Some(expected.as_str()) {
            let body = serde_json::json!({
                "error": "unauthorized",
                "reason": "missing or invalid token",
            })
            .to_string();
            return (StatusCode::UNAUTHORIZED, body).into_response();
        }
    }
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

fn extract_token(headers: &HeaderMap, params: &HashMap<String, String>) -> Option<String> {
    // Prefer Authorization: Bearer <token>
    if let Some(h) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = h.to_str() {
            let prefix = "Bearer ";
            if s.starts_with(prefix) {
                return Some(s[prefix.len()..].to_string());
            }
        }
    }
    // Fall back to `?token=<token>` query parameter
    params.get("token").cloned()
}

/// Per-socket task: splits sink/stream, forwards broadcast lines to client, and
/// processes incoming control messages (interrupt, projects/skills, status, run.submit, etc.).
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    info!("msg" = "websocket connected");
    // Debug visibility: announce client connections on the broadcast feed
    let _ = state.tx.send(serde_json::json!({"type":"bridge.client_connected"}).to_string());

    // Broadcast reader → socket
    let mut rx = state.tx.subscribe();
    let (mut sink, mut stream) = socket.split();
    let history = { state.history.lock().await.clone() };
    let mut sink_task = tokio::spawn(async move {
        for line in history {
            if sink.send(Message::Text(line.into())).await.is_err() {
                return;
            }
        }
        loop {
            match rx.recv().await {
                Ok(line) => {
                    if sink.send(Message::Text(line.into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Socket → child stdin
    let stdin_state = state.clone();
    let mut read_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(t) => {
                    // Optional: echo any inbound text preview for debugging
                    if std::env::var("BRIDGE_DEBUG_WS").ok().as_deref() == Some("1") {
                        let preview: String = if t.len() > 160 { format!("{}…", &t[..160]) } else { t.to_string() };
                        let dbg = serde_json::json!({"type":"bridge.ws_in","preview": preview}).to_string();
                        let _ = stdin_state.tx.send(dbg);
                    }
                    if let Some(cmd) = parse_control_command(&t) {
                        info!(?cmd, "ws control command");
                        // Broadcast control receipt for visibility to connected tails (tricoder)
                        let ctrl: String = if t.len() > 160 {
                            format!("{}…", &t[..160])
                        } else {
                            t.to_string()
                        };
                        let dbg =
                            serde_json::json!({"type":"bridge.control","raw": ctrl}).to_string();
                        let _ = stdin_state.tx.send(dbg);
                        match cmd {
                            ControlCommand::Echo { payload, tag } => {
                                let line = serde_json::json!({
                                    "type": "bridge.echo",
                                    "ts": now_ms(),
                                    "tag": tag,
                                    "payload": payload,
                                }).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::Interrupt => {
                                if let Err(e) = interrupt_running_child(&stdin_state).await {
                                    error!(?e, "failed to interrupt codex child");
                                }
                            }
                            ControlCommand::Projects => match crate::projects::list_projects() {
                                Ok(items) => {
                                    let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                    let _ = stdin_state.tx.send(line);
                                }
                                Err(e) => {
                                    error!(?e, "projects list failed via ws");
                                }
                            },
                            ControlCommand::Skills => {
                                match crate::skills::list_skills() {
                                    Ok(items) => {
                                        let line = serde_json::json!({"type":"bridge.skills","items": items}).to_string();
                                        let _ = stdin_state.tx.send(line);
                                    }
                                    Err(e) => {
                                        error!(?e, "skills list failed via ws");
                                    }
                                }
                            }
                            ControlCommand::BridgeStatus => {
                                let codex_pid = { *stdin_state.child_pid.lock().await };
                                let last_thread_id = { stdin_state.last_thread_id.lock().await.clone() };
                                let bind = stdin_state.opts.bind.clone();
                                let line = serde_json::json!({
                                    "type": "bridge.status",
                                    "bind": bind,
                                    "codex_pid": codex_pid,
                                    "last_thread_id": last_thread_id,
                                    "tinyvex": true
                                }).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::TvxSubscribe { stream, thread_id } => {
                                let limit = 100i64;
                                if stream == "threads" {
                                    match stdin_state.tinyvex.list_threads(limit) {
                                        Ok(rows) => {
                                            let line = serde_json::json!({
                                                "type":"tinyvex.snapshot",
                                                "stream":"threads",
                                                "rows": rows,
                                                "rev": 0
                                            }).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                        Err(e) => { error!(?e, "tinyvex threads snapshot failed"); }
                                    }
                                } else if stream == "messages" {
                                    if let Some(tid) = thread_id.as_deref() {
                                        match stdin_state.tinyvex.list_messages(tid, limit) {
                                            Ok(rows) => {
                                                let line = serde_json::json!({
                                                    "type":"tinyvex.snapshot",
                                                    "stream":"messages",
                                                    "threadId": tid,
                                                    "rows": rows,
                                                    "rev": 0
                                                }).to_string();
                                                let _ = stdin_state.tx.send(line);
                                            }
                                            Err(e) => { error!(?e, "tinyvex messages snapshot failed"); }
                                        }
                                    }
                                }
                            }
                            ControlCommand::TvxQuery { name, args } => {
                                let limit = args.get("limit").and_then(|x| x.as_i64()).unwrap_or(50);
                                if name == "threads.list" {
                                    match stdin_state.tinyvex.list_threads(limit) {
                                        Ok(rows) => {
                                            let line = serde_json::json!({"type":"tinyvex.query_result","name":"threads.list","rows": rows}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                        Err(e) => { error!(?e, "tinyvex threads.list failed"); }
                                    }
                                } else if name == "messages.list" {
                                    if let Some(tid) = args.get("threadId").and_then(|x| x.as_str()) {
                                        match stdin_state.tinyvex.list_messages(tid, limit) {
                                            Ok(rows) => {
                                                let line = serde_json::json!({"type":"tinyvex.query_result","name":"messages.list","threadId": tid, "rows": rows}).to_string();
                                                let _ = stdin_state.tx.send(line);
                                            }
                                            Err(e) => { error!(?e, "tinyvex messages.list failed"); }
                                        }
                                    }
                                } else if name == "messages.tailMany" {
                                    let per = args.get("perThread").and_then(|x| x.as_i64()).unwrap_or(50);
                                    if let Some(ids) = args.get("threadIds").and_then(|x| x.as_array()) {
                                        let mut items: Vec<serde_json::Value> = Vec::new();
                                        for idv in ids {
                                            if let Some(tid) = idv.as_str() {
                                                match stdin_state.tinyvex.list_messages(tid, per) {
                                                    Ok(rows) => items.push(serde_json::json!({"threadId": tid, "rows": rows})),
                                                    Err(e) => { error!(?e, "tinyvex messages.tailMany failed for thread"); items.push(serde_json::json!({"threadId": tid, "rows": []})); }
                                                }
                                            }
                                        }
                                        let line = serde_json::json!({"type":"tinyvex.query_result","name":"messages.tailMany","rows": items}).to_string();
                                        let _ = stdin_state.tx.send(line);
                                    }
                                } else if name == "threads.listSince" {
                                    let updated_after = args.get("updatedAfter").and_then(|x| x.as_i64()).unwrap_or(0);
                                    let lim = args.get("limit").and_then(|x| x.as_i64()).unwrap_or(200);
                                    match stdin_state.tinyvex.list_threads(lim) {
                                        Ok(mut rows) => {
                                            rows.retain(|r| r.updated_at > updated_after);
                                            let line = serde_json::json!({"type":"tinyvex.query_result","name":"threads.listSince","rows": rows}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                        Err(e) => { error!(?e, "tinyvex threads.listSince failed"); }
                                    }
                                } else if name == "messages.since" {
                                    if let Some(tid) = args.get("threadId").and_then(|x| x.as_str()) {
                                        let after_seq = args.get("afterSeq").and_then(|x| x.as_i64());
                                        let after_ts = args.get("afterTs").and_then(|x| x.as_i64());
                                        let lim = args.get("limit").and_then(|x| x.as_i64()).unwrap_or(500);
                                        match stdin_state.tinyvex.list_messages(tid, lim) {
                                            Ok(mut rows) => {
                                                if let Some(s) = after_seq { rows.retain(|r| r.seq.unwrap_or(0) > s); }
                                                if let Some(ts) = after_ts { rows.retain(|r| r.ts > ts); }
                                                let line = serde_json::json!({"type":"tinyvex.query_result","name":"messages.since","threadId": tid, "rows": rows}).to_string();
                                                let _ = stdin_state.tx.send(line);
                                            }
                                            Err(e) => { error!(?e, "tinyvex messages.since failed"); }
                                        }
                                    }
                                } else if name == "threadsAndTails.list" {
                                    let lim = args.get("limit").and_then(|x| x.as_i64()).unwrap_or(50);
                                    let per = args.get("perThreadTail").and_then(|x| x.as_i64()).unwrap_or(50);
                                    match stdin_state.tinyvex.list_threads(lim) {
                                        Ok(rows) => {
                                            let mut tails: Vec<serde_json::Value> = Vec::new();
                                            for r in &rows {
                                                let tid = r.thread_id.as_deref().unwrap_or(&r.id);
                                                match stdin_state.tinyvex.list_messages(tid, per) {
                                                    Ok(m) => tails.push(serde_json::json!({"threadId": tid, "rows": m})),
                                                    Err(_) => tails.push(serde_json::json!({"threadId": tid, "rows": []})),
                                                }
                                            }
                                            let line = serde_json::json!({
                                                "type":"tinyvex.query_result",
                                                "name":"threadsAndTails.list",
                                                "threads": rows,
                                                "tails": tails
                                            }).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                        Err(e) => { error!(?e, "tinyvex threadsAndTails.list failed"); }
                                    }
                                } else if name == "toolCalls.list" {
                                    let tid = args.get("threadId").and_then(|x| x.as_str()).unwrap_or("");
                                    let lim = args.get("limit").and_then(|x| x.as_i64()).unwrap_or(50);
                                    if !tid.is_empty() {
                                        let send_rows = |rows: Vec<tinyvex::ToolCallRow>| {
                                            let line = serde_json::json!({
                                                "type":"tinyvex.query_result",
                                                "name":"toolCalls.list",
                                                "threadId": tid,
                                                "rows": rows
                                            }).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        };
                                        match stdin_state.tinyvex.list_tool_calls(tid, lim) {
                                            Ok(rows) if !rows.is_empty() => send_rows(rows),
                                            _ => {
                                                // On-demand backfill: parse the Codex rollout file for this session id and mirror tool calls
                                                let base = crate::watchers::codex_base_path();
                                                if let Some(path) = crate::history::resolve_session_path(&base, Some(tid), None) {
                                                    if let Ok(f) = std::fs::File::open(&path) {
                                                        use std::io::{BufRead, BufReader};
                                                        let r = BufReader::new(f);
                                                        for line in r.lines().flatten() {
                                                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                                                                if let Some(update) = acp_event_translator::translate_codex_event_to_acp_update(&v) {
                                                                    // Only mirror tool call related updates here; others are already in Tinyvex via tails
                                                                    match update {
                                                                        agent_client_protocol::SessionUpdate::ToolCall(_) | agent_client_protocol::SessionUpdate::ToolCallUpdate(_) => {
                                                                            crate::tinyvex_write::mirror_acp_update_to_tinyvex(&stdin_state, "codex", tid, &update).await;
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // Re-query after backfill
                                                    match stdin_state.tinyvex.list_tool_calls(tid, lim) {
                                                        Ok(rows2) => send_rows(rows2),
                                                        Err(e) => { error!(?e, "tinyvex toolCalls.list after backfill failed"); }
                                                    }
                                                } else {
                                                    // No session file found; reply with empty
                                                    send_rows(Vec::new());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ControlCommand::SyncStatus => {
                                let enabled = stdin_state.sync_enabled.load(std::sync::atomic::Ordering::Relaxed);
                                let two_way = stdin_state.sync_two_way.load(std::sync::atomic::Ordering::Relaxed);
                                let base = crate::watchers::codex_base_path();
                                // Count JSONL files (best-effort)
                                let files = {
                                    let mut count = 0usize;
                                    let mut stack = vec![base.clone()];
                                    while let Some(dir) = stack.pop() {
                                        if let Ok(rd) = std::fs::read_dir(&dir) {
                                            for ent in rd.flatten() {
                                                let p = ent.path();
                                                if p.is_dir() { stack.push(p); }
                                                else if p.extension().and_then(|e| e.to_str()) == Some("jsonl") { count += 1; }
                                            }
                                        }
                                    }
                                    count
                                } as i64;
                                let last = { *stdin_state.sync_last_read_ms.lock().await } as i64;
                                let line = serde_json::json!({
                                    "type":"bridge.sync_status",
                                    "enabled": enabled,
                                    "twoWay": two_way,
                                    "watched": [{"provider":"codex","base": base.display().to_string(), "files": files, "lastRead": last }]
                                }).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::SyncEnable { enabled } => {
                                stdin_state.sync_enabled.store(enabled, std::sync::atomic::Ordering::Relaxed);
                                let tx_opt = stdin_state.sync_cmd_tx.lock().await.clone();
                                if let Some(tx) = tx_opt {
                                    if let Err(err) = tx.send(SyncCommand::Enable(enabled)).await {
                                        tracing::warn!(?err, "sync enable send failed");
                                    }
                                }
                            }
                            ControlCommand::SyncTwoWay { enabled } => {
                                stdin_state.sync_two_way.store(enabled, std::sync::atomic::Ordering::Relaxed);
                                let tx_opt = stdin_state.sync_cmd_tx.lock().await.clone();
                                if let Some(tx) = tx_opt {
                                    if let Err(err) = tx.send(SyncCommand::TwoWay(enabled)).await {
                                        tracing::warn!(?err, "sync twoWay send failed");
                                    }
                                }
                            }
                            ControlCommand::SyncFullRescan => {
                                let tx_opt = stdin_state.sync_cmd_tx.lock().await.clone();
                                if let Some(tx) = tx_opt {
                                    if let Err(err) = tx.send(SyncCommand::FullRescan).await {
                                        tracing::warn!(?err, "sync fullRescan send failed");
                                    }
                                }
                            }
                            ControlCommand::TvxMutate { name, args } => {
                                // Echo the requested mutation so fields are read and visible to clients
                                let _ = stdin_state.tx.send(
                                    serde_json::json!({
                                        "type":"tinyvex.todo",
                                        "op":"mutate",
                                        "name": name,
                                        "args": args,
                                    }).to_string()
                                );
                            }
                            ControlCommand::TvxBackfill => {
                                let _ = stdin_state
                                    .tx
                                    .send(serde_json::json!({"type":"tinyvex.todo","op":"backfill"}).to_string());
                            }
                            ControlCommand::RunSubmit {
                                thread_doc_id,
                                text,
                                project_id,
                                resume_id,
                                provider,
                            } => {
                                // Decide provider (claude_code vs codex) based on project metadata.
                                let provider = provider.or_else(|| project_id.as_ref().and_then(|pid| {
                                    match crate::projects::list_projects() {
                                        Ok(items) => items.into_iter().find(|p| p.id == *pid),
                                        Err(_) => None,
                                    }
                                }).and_then(|p: Project| {
                                    // Interpret agent_file == "claude_code" (convention) as Claude provider
                                    let af = p.agent_file.unwrap_or_default().to_lowercase();
                                    if af == "claude_code" || af == "claude" || af == "claude-code" { Some("claude_code".to_string()) } else { None }
                                }));
                                if let Some(kind) = provider.as_deref() {
                                    if kind == "claude_code" {
                                        let desired_cd = project_id.as_ref().and_then(|pid| {
                                            match crate::projects::list_projects() {
                                                Ok(list) => list
                                                    .into_iter()
                                                    .find(|p| p.id == *pid)
                                                    .map(|p| p.working_dir),
                                                Err(_) => None,
                                            }
                                        }).map(PathBuf::from);
                                        // Remember target Convex thread id for writes
                                        {
                                            *stdin_state.current_thread_doc.lock().await = Some(thread_doc_id.clone());
                                        }
                                        // Record pending user message text so we can emit ACP once session id is known (on Claude init mapping)
                                        {
                                            let mut pending = stdin_state.pending_user_text.lock().await;
                                            pending.insert(thread_doc_id.clone(), text.clone());
                                        }
                                        let st_for = stdin_state.clone();
                                        let thread_for = thread_doc_id.clone();
                                        tokio::spawn(async move {
                                            match crate::claude_runner::spawn_claude_child_with_prompt(&st_for.opts, desired_cd, &text).await {
                                                Ok(child) => {
                                                    if let Err(e) = crate::claude_runner::start_claude_forwarders(child, st_for.clone()).await { tracing::error!(?e, "claude forwarders failed"); }
                                                    let dbg = serde_json::json!({"type":"bridge.run_submit","provider":"claude_code","threadDocId": thread_for, "len": text.len()}).to_string();
                                                    let _ = st_for.tx.send(dbg);
                                                }
                                                Err(e) => tracing::error!(?e, "claude spawn failed"),
                                            }
                                        });
                                        continue;
                                    }
                                }
                                // Map to project working dir
                                let desired_cd = project_id.as_ref().and_then(|pid| {
                                    match crate::projects::list_projects() {
                                        Ok(list) => list
                                            .into_iter()
                                            .find(|p| p.id == *pid)
                                            .map(|p| p.working_dir),
                                        Err(_) => None,
                                    }
                                });
                                // Remember target Convex thread id for writes
                                {
                                    *stdin_state.current_thread_doc.lock().await =
                                        Some(thread_doc_id.clone());
                                }
                                // Record pending user message text so we can emit ACP once session id is known
                                {
                                    let mut pending = stdin_state.pending_user_text.lock().await;
                                    pending.insert(thread_doc_id.clone(), text.clone());
                                }
                                // Emit a synthetic ACP event for user_message_chunk as a bridge.acp once session id is known (on thread.started below)
                                // Determine per-thread resume target
                                let per_thread_last = {
                                    let map = stdin_state.sessions_by_client_doc.lock().await;
                                    map.get(&thread_doc_id).cloned()
                                };
                                let global_last = { stdin_state.last_thread_id.lock().await.clone() };
                                let resume_arg = match resume_id.as_deref() {
                                    Some("new") | Some("none") => None,
                                    Some("last") => per_thread_last.or(global_last),
                                    Some(s) if !s.is_empty() => Some(s.to_string()),
                                    _ => per_thread_last.or(global_last),
                                };
                                let use_resume = resume_arg.is_some();
                                match spawn_codex_child_with_prompt(
                                    &stdin_state.opts,
                                    desired_cd.clone().map(|s| std::path::PathBuf::from(s)),
                                    resume_arg.as_deref(),
                                    &text,
                                    use_resume,
                                )
                                .await
                                {
                                    Ok(child) => {
                                        if let Err(e) =
                                            start_stream_forwarders(child, stdin_state.clone())
                                                .await
                                        {
                                            error!(?e, "run.submit: forwarders failed");
                                        }
                                        let dbg = serde_json::json!({"type":"bridge.run_submit","threadDocId": thread_doc_id, "cd": desired_cd, "resumeId": resume_id, "len": text.len()}).to_string();
                                        let _ = stdin_state.tx.send(dbg);
                                    }
                                    Err(e) => {
                                        error!(?e, "run.submit: spawn failed");
                                    }
                                }
                            }
                            ControlCommand::ProjectSave { project } => {
                                match crate::projects::save_project(&project) {
                                    Ok(_) => {
                                        if let Ok(items) = crate::projects::list_projects() {
                                            let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                    }
                                    Err(e) => {
                                        error!(?e, "project save failed via ws");
                                    }
                                }
                            }
                            ControlCommand::ProjectDelete { id } => {
                                match crate::projects::delete_project(&id) {
                                    Ok(_) => {
                                        if let Ok(items) = crate::projects::list_projects() {
                                            let line = serde_json::json!({"type":"bridge.projects","items": items}).to_string();
                                            let _ = stdin_state.tx.send(line);
                                        }
                                    }
                                    Err(e) => {
                                        error!(?e, "project delete failed via ws");
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    let preview = if t.len() > 180 {
                        format!("{}…", &t[..180].replace('\n', "\\n"))
                    } else {
                        t.replace('\n', "\\n")
                    };
                    info!(
                        "msg" = "ws text received",
                        size = t.len(),
                        preview = preview
                    );
                    let desired_cd = extract_cd_from_ws_payload(&t);
                    let desired_resume = extract_resume_from_ws_payload(&t);
                    info!(?desired_cd, ?desired_resume, msg = "parsed ws preface");
                    // Spawn a child process per prompt, passing the prompt as a positional
                    // arg to avoid ambiguity with `exec resume` and the '-' stdin marker.
                    let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                    let resume_arg: Option<String> = match desired_resume.as_deref() {
                        Some("new") | Some("none") => None,
                        Some("last") => resume_id.clone(),
                        Some(s) if !s.is_empty() => Some(s.to_string()),
                        _ => resume_id.clone(),
                    };
                    let use_resume = resume_arg.is_some();
                    match spawn_codex_child_with_prompt(
                        &stdin_state.opts,
                        desired_cd.clone(),
                        resume_arg.as_deref(),
                        &t,
                        use_resume,
                    )
                    .await
                    {
                        Ok(child) => {
                            // No stdin writing; just start forwarders
                            if let Err(e) = start_stream_forwarders(child, stdin_state.clone()).await {
                                error!(?e, "failed starting forwarders for codex child");
                            }
                        }
                        Err(e) => {
                            error!(?e, "failed to spawn codex with prompt");
                        }
                    }
                }
                Message::Binary(_b) => { /* binary not used */ }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut sink_task) => { read_task.abort(); },
        _ = (&mut read_task) => { sink_task.abort(); },
    }
    info!("msg" = "websocket disconnected");
    let _ = state.tx.send(serde_json::json!({"type":"bridge.client_disconnected"}).to_string());
}

#[cfg(test)]
mod auth_tests {
    use super::*;
    use axum::{routing::get, Router};
    use axum::http; 
    use tokio::sync::{broadcast, Mutex};
    use tokio_tungstenite::tungstenite;
    use std::net::SocketAddr;

    async fn spawn_server(state: Arc<AppState>) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let app = Router::new().route("/ws", get(crate::ws::ws_handler)).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (addr, task)
    }

    fn mk_state_with_token(tok: Option<&str>) -> Arc<AppState> {
        let (tx, _rx) = broadcast::channel(64);
        let opts = crate::Opts {
            bind: "127.0.0.1:0".into(),
            codex_bin: None,
            codex_args: None,
            extra: vec![],
            ws_token: tok.map(|s| s.to_string()),
            claude_bin: None,
            claude_args: None,
        };
        let tvx = std::sync::Arc::new(tinyvex::Tinyvex::open(tempfile::NamedTempFile::new().unwrap().path()).unwrap());
        Arc::new(AppState {
            tx,
            child_stdin: Mutex::new(None),
            child_pid: Mutex::new(None),
            opts,
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_thread_doc: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            pending_user_text: Mutex::new(std::collections::HashMap::new()),
            client_doc_by_session: Mutex::new(std::collections::HashMap::new()),
            sessions_by_client_doc: Mutex::new(std::collections::HashMap::new()),
            bridge_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: tvx.clone(),
            tinyvex_writer: std::sync::Arc::new(tinyvex::Writer::new(tvx.clone())),
            sync_enabled: std::sync::atomic::AtomicBool::new(true),
            sync_two_way: std::sync::atomic::AtomicBool::new(false),
            sync_last_read_ms: Mutex::new(0),
            sync_cmd_tx: Mutex::new(None),
        })
    }

    #[tokio::test]
    async fn rejects_when_token_required_and_missing() {
        let state = mk_state_with_token(Some("secret"));
        let (addr, _task) = spawn_server(state).await;
        let url = format!("ws://{}/ws", addr);
        let res = tokio_tungstenite::connect_async(&url).await;
        assert!(res.is_err(), "expected handshake to fail without token");
    }

    #[tokio::test]
    async fn rejects_when_token_wrong() {
        let state = mk_state_with_token(Some("secret"));
        let (addr, _task) = spawn_server(state).await;
        let url = format!("ws://{}/ws?token=bad", addr);
        let res = tokio_tungstenite::connect_async(&url).await;
        assert!(res.is_err(), "expected handshake to fail with wrong token");
    }

    #[tokio::test]
    async fn accepts_with_query_token() {
        let state = mk_state_with_token(Some("secret"));
        let (addr, _task) = spawn_server(state).await;
        let url = format!("ws://{}/ws?token=secret", addr);
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("handshake ok");
        ws.close(None).await.ok();
    }

    #[tokio::test]
    async fn accepts_with_auth_header() {
        let state = mk_state_with_token(Some("secret"));
        let (addr, _task) = spawn_server(state).await;
        let url = format!("ws://{}/ws", addr);
        let req = tungstenite::client::IntoClientRequest::into_client_request(url).unwrap();
        let mut req: http::Request<()> = req;
        req.headers_mut().insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_str("Bearer secret").unwrap(),
        );
        let (mut ws, _resp) = tokio_tungstenite::connect_async(req)
            .await
            .expect("handshake ok with header");
        ws.close(None).await.ok();
    }

    #[tokio::test]
    async fn rejects_when_state_has_no_token_even_if_client_supplies_one() {
        // Defensive: the production binary always provisions a token at startup,
        // but ensure the handler still rejects when state carries an empty token.
        let state = mk_state_with_token(None);
        let (addr, _task) = spawn_server(state).await;
        // Even with a supplied token, server should reject because expected is empty/open mode is disabled.
        let url = format!("ws://{}/ws?token=anything", addr);
        let res = tokio_tungstenite::connect_async(&url).await;
        assert!(res.is_err(), "expected handshake to fail when server has no token configured");
    }

    // No open mode: token is always required; covered by rejects_when_token_required_and_missing
}

/// Spawn tasks to read Codex stdout/stderr and forward to both console and broadcast channel.
pub async fn start_stream_forwarders(mut child: ChildWithIo, state: Arc<AppState>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;

    let tx_out = state.tx.clone();
    let state_for_stdout = state.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let low = line.trim().to_ascii_lowercase();
            if low.contains("reading prompt from stdin") || low == "no prompt provided via stdin." {
                continue;
            }
            if low.contains("codex_core::exec: exec error") {
                continue;
            }
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            println!("{}", log);
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stdout", raw = %line);
            }
            // Optional raw echo for debugging Codex JSONL mapping
            if std::env::var("BRIDGE_DEBUG_CODEX").ok().as_deref() == Some("1") {
                let snippet = if line.len() > 200 {
                    format!("{}…", &line[..200])
                } else {
                    line.clone()
                };
                let dbg =
                    serde_json::json!({"type":"bridge.codex_raw","line": snippet}).to_string();
                let _ = tx_out.send(dbg);
            }
            if let Ok(v) = serde_json::from_str::<JsonValue>(&line) {
                let t = v
                    .get("type")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| {
                        v.get("msg")
                            .and_then(|m| m.get("type"))
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string())
                    });
                if matches!(t.as_deref(), Some("thread.started")) {
                    let id = v.get("thread_id").and_then(|x| x.as_str()).or_else(|| {
                        v.get("msg")
                            .and_then(|m| m.get("thread_id"))
                            .and_then(|x| x.as_str())
                    });
                    if let Some(val) = id {
                        let _ = state_for_stdout
                            .last_thread_id
                            .lock()
                            .await
                            .insert(val.to_string());
                        info!(thread_id=%val, msg="captured thread id for resume");
                        // Tinyvex path does not need an explicit upsert here; handled on message writes
                        // Broadcast a debug event for visibility in tools
                        let client_doc = { state_for_stdout.current_thread_doc.lock().await.clone() };
                        let dbg = serde_json::json!({
                                "type": "bridge.codex_event",
                                "event_type": "thread.started",
                                "thread_id": v.get("thread_id").and_then(|x| x.as_str()).or_else(|| v.get("msg").and_then(|m| m.get("thread_id")).and_then(|x| x.as_str())).unwrap_or(""),
                                "clientThreadDocId": client_doc
                            }).to_string();
                        let _ = tx_out.send(dbg);
                        // Also emit a dedicated session mapping event for mobile
                        let map_evt = serde_json::json!({
                            "type": "bridge.session_started",
                            "sessionId": val,
                            "clientThreadDocId": client_doc
                        }).to_string();
                        let _ = tx_out.send(map_evt);
                        // Update per-thread mapping for resume on subsequent submits for this client thread
                        if let Some(client_doc_str) = client_doc.clone() {
                            let mut map = state_for_stdout.sessions_by_client_doc.lock().await;
                            map.insert(client_doc_str, val.to_string());
                        }
                        // Also track inverse mapping: session -> client doc for watcher aliasing and hydration
                        if let Some(client_doc_str) = client_doc.clone() {
                            let mut inv = state_for_stdout.client_doc_by_session.lock().await;
                            inv.insert(val.to_string(), client_doc_str.clone());
                        }
                        // Upsert threads row for the client doc id to record resume_id = session id
                        if let Some(client_doc_str) = client_doc.clone() {
                            let now: i64 = now_ms().try_into().unwrap_or(0);
                            let row = tinyvex::ThreadRow {
                                id: client_doc_str.clone(),
                                thread_id: Some(client_doc_str.clone()),
                                title: "Thread".into(),
                                project_id: None,
                                resume_id: Some(val.to_string()),
                                rollout_path: None,
                                source: Some("codex".into()),
                                created_at: now,
                                updated_at: now,
                                message_count: None,
                            };
                            let _ = state_for_stdout.tinyvex.upsert_thread(&row);
                        }
                        // If we have a pending user text for this client thread doc id, emit ACP and write to Tinyvex acp_events
                        if let Some(client_doc_str) = client_doc.clone() {
                            if let Some(user_text) = { state_for_stdout.pending_user_text.lock().await.remove(&client_doc_str) } {
                                let ts_now = now_ms();
                                // Emit bridge.acp
                                let notif = serde_json::json!({
                                    "sessionId": val,
                                    "update": { "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": user_text } }
                                });
                                let _ = tx_out.send(serde_json::json!({"type":"bridge.acp","notification": notif}).to_string());
                                // Persist via Tinyvex mirror so UI history shows user message
                                let update = agent_client_protocol::SessionUpdate::UserMessageChunk(agent_client_protocol::ContentChunk {
                                    content: agent_client_protocol::ContentBlock::Text(agent_client_protocol::TextContent { annotations: None, text: user_text.clone(), meta: None }),
                                    meta: None,
                                });
                                crate::tinyvex_write::mirror_acp_update_to_tinyvex(&state_for_stdout, "codex", &client_doc_str, &update).await;
                                // Also write to unified acp_events log
                                let _ = state_for_stdout.tinyvex.insert_acp_event(Some(&val.to_string()), Some(&client_doc_str), ts_now.try_into().unwrap(), Some(0), "user_message_chunk", Some("user"), Some(&user_text), None, None, None, None, None, None);
                            }
                        }
                    }
                }
                if matches!(t.as_deref(), Some("turn.completed")) {
                    let convex_tid_opt =
                        { state_for_stdout.current_thread_doc.lock().await.clone() };
                    let target_tid = if let Some(s) = convex_tid_opt {
                        s
                    } else {
                        state_for_stdout
                            .last_thread_id
                            .lock()
                            .await
                            .clone()
                            .unwrap_or_default()
                    };
                    if !target_tid.is_empty() {
                        finalize_streaming_for_thread(&state_for_stdout, &target_tid).await;
                    }
                }
                if matches!(t.as_deref(), Some("agent_message.delta"))
                    || matches!(t.as_deref(), Some("assistant.delta"))
                    || matches!(t.as_deref(), Some("message.delta"))
                {
                    // Handle both { type, payload } and { msg: { type, payload } } shapes
                    let payload = v
                        .get("payload")
                        .or_else(|| v.get("msg").and_then(|m| m.get("payload")));
                    let txt = payload
                        .and_then(|p| p.get("text"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let convex_tid_opt =
                        { state_for_stdout.current_thread_doc.lock().await.clone() };
                    let target_tid = if let Some(s) = convex_tid_opt {
                        s
                    } else {
                        state_for_stdout
                            .last_thread_id
                            .lock()
                            .await
                            .clone()
                            .unwrap_or_default()
                    };
                    if !target_tid.is_empty() {
                        stream_upsert_or_append(&state_for_stdout, &target_tid, "assistant", &txt)
                            .await;
                        info!(len=txt.len(), thread=%target_tid, "assistant.delta mapped");
                        let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.delta","len":txt.len(),"thread":target_tid}).to_string();
                        let _ = tx_out.send(dbg);
                        // pending user text handled on thread.started once session id is known
                    }
                }
                if matches!(t.as_deref(), Some("reasoning.delta")) || matches!(t.as_deref(), Some("reason.delta")) {
                    // Ignore legacy reasoning streaming; chat handled via item.* below to avoid duplicates
                }
                if matches!(t.as_deref(), Some("agent_message")) || matches!(t.as_deref(), Some("assistant")) || matches!(t.as_deref(), Some("message")) {
                    // Ignore legacy assistant final; chat handled via item.* below to avoid duplicates
                }
                if matches!(t.as_deref(), Some("reasoning")) {
                    // Ignore legacy reasoning final; chat handled via item.* below to avoid duplicates
                }
                // Newer shapes: item.delta / item.completed with item.type = agent_message | reasoning
                if let Some(ty) = t.as_deref() {
                    if ty == "item.delta" || ty == "item.completed" {
                        if let Some(item) = v.get("item").or_else(|| v.get("payload").and_then(|p| p.get("item"))) {
                            let kind = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
                            let txt = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
                            let convex_tid_opt = { state_for_stdout.current_thread_doc.lock().await.clone() };
                            let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                            if !target_tid.is_empty() {
                                match kind {
                                    "agent_message" => {
                                        if ty == "item.delta" {
                                            stream_upsert_or_append(&state_for_stdout, &target_tid, "assistant", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.delta","len":txt.len(),"thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        } else {
                                            crate::tinyvex_write::finalize_or_snapshot(&state_for_stdout, &target_tid, "assistant", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.final","thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        }
                                        // Emit ACP notification for UI live rendering (Tinyvex already stores chat above)
                                        if let Some(update) = translate_codex_event_to_acp_update(&v) {
                                            let acp_session = state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default();
                                            let notif = SessionNotification { session_id: SessionId(acp_session.clone().into()), update: update.clone(), meta: None };
                                            if let Ok(line) = serde_json::to_string(&serde_json::json!({ "type": "bridge.acp", "notification": notif })) { let _ = tx_out.send(line); }
                                            // Also write to unified acp_events log (audit only)
                                            let ts = now_ms();
                                            let raw_json = Some(v.to_string());
                                            let content_json = v.get("item").and_then(|it| it.get("content")).map(|c| c.to_string());
                                            let locations_json = v.get("item").and_then(|it| it.get("locations")).map(|c| c.to_string());
                                            let text_field = v.get("item").and_then(|it| it.get("text")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let kind_field = v.get("item").and_then(|it| it.get("type")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let status_field = v.get("item").and_then(|it| it.get("status")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let tool_call_id = v.get("item").and_then(|it| it.get("id")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let client_doc_for = { state_for_stdout.current_thread_doc.lock().await.clone() };
                                            let _ = state_for_stdout.tinyvex.insert_acp_event(
                                                Some(&acp_session),
                                                client_doc_for.as_deref(),
                                                ts.try_into().unwrap(),
                                                Some(0),
                                                "agent_message_chunk",
                                                Some("assistant"),
                                                text_field.as_deref(),
                                                tool_call_id.as_deref(),
                                                status_field.as_deref(),
                                                kind_field.as_deref(),
                                                content_json.as_deref(),
                                                locations_json.as_deref(),
                                                raw_json.as_deref(),
                                            );
                                        }
                                    }
                                    "reasoning" => {
                                        if ty == "item.delta" {
                                            stream_upsert_or_append(&state_for_stdout, &target_tid, "reason", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.delta","len":txt.len(),"thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        } else {
                                            crate::tinyvex_write::finalize_or_snapshot(&state_for_stdout, &target_tid, "reason", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.final","thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        }
                                        // Emit ACP notification for live reasoning rendering (no Tinyvex mirror for chat duplicates)
                                        if let Some(update) = translate_codex_event_to_acp_update(&v) {
                                            let acp_session = state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default();
                                            let notif = SessionNotification { session_id: SessionId(acp_session.clone().into()), update: update.clone(), meta: None };
                                            if let Ok(line) = serde_json::to_string(&serde_json::json!({ "type": "bridge.acp", "notification": notif })) { let _ = tx_out.send(line); }
                                            let ts = now_ms();
                                            let raw_json = Some(v.to_string());
                                            let content_json = v.get("item").and_then(|it| it.get("content")).map(|c| c.to_string());
                                            let locations_json = v.get("item").and_then(|it| it.get("locations")).map(|c| c.to_string());
                                            let text_field = v.get("item").and_then(|it| it.get("text")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let kind_field = v.get("item").and_then(|it| it.get("type")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let status_field = v.get("item").and_then(|it| it.get("status")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let tool_call_id = v.get("item").and_then(|it| it.get("id")).and_then(|x| x.as_str()).map(|s| s.to_string());
                                            let client_doc_for = { state_for_stdout.current_thread_doc.lock().await.clone() };
                                            let _ = state_for_stdout.tinyvex.insert_acp_event(
                                                Some(&acp_session),
                                                client_doc_for.as_deref(),
                                                ts.try_into().unwrap(),
                                                Some(0),
                                                "agent_thought_chunk",
                                                None,
                                                text_field.as_deref(),
                                                tool_call_id.as_deref(),
                                                status_field.as_deref(),
                                                kind_field.as_deref(),
                                                content_json.as_deref(),
                                                locations_json.as_deref(),
                                                raw_json.as_deref(),
                                            );
                                        }
                                    }
                                    _ => {}
                                }
                                // Mirror ACP update for tools/plan/state only (avoid duplicating chat streaming)
                                if let Some(update) = translate_codex_event_to_acp_update(&v) {
                                    let is_chat = matches!(
                                        &update,
                                        agent_client_protocol::SessionUpdate::UserMessageChunk(_)
                                            | agent_client_protocol::SessionUpdate::AgentMessageChunk(_)
                                            | agent_client_protocol::SessionUpdate::AgentThoughtChunk(_)
                                    );
                                    if !is_chat {
                                        let target_tid = {
                                            let ctid = state_for_stdout.current_thread_doc.lock().await.clone();
                                            if let Some(s) = ctid { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() }
                                        };
                                        if !target_tid.is_empty() {
                                            crate::tinyvex_write::mirror_acp_update_to_tinyvex(&state_for_stdout, "codex", &target_tid, &update).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Tool rows: command/file/search/mcp/todo via item.*
                if let Some(ty) = t.as_deref() {
                    if ty.starts_with("item.") {
                        if let Some(payload) = v
                            .get("item")
                            .or_else(|| v.get("payload").and_then(|p| p.get("item")))
                        {
                            let kind = payload.get("type").and_then(|x| x.as_str()).unwrap_or("");
                            let map_kind = map_tool_kind(kind);
                            if map_kind.is_some() {
                                let convex_tid_opt =
                                    { state_for_stdout.current_thread_doc.lock().await.clone() };
                                let target_tid = if let Some(s) = convex_tid_opt {
                                    s
                                } else {
                                    state_for_stdout
                                        .last_thread_id
                                        .lock()
                                        .await
                                        .clone()
                                        .unwrap_or_default()
                                };
                                if !target_tid.is_empty() {
                                    // Tinyvex path: tool events can be recorded later; skip heavy writes here
                                    // Mirror ACP (tool/plan/state) into Convex and optionally emit for debugging
                                    if let Some(update) = translate_codex_event_to_acp_update(&v) {
                                        let target_tid = {
                                            let ctid = state_for_stdout.current_thread_doc.lock().await.clone();
                                            if let Some(s) = ctid { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() }
                                        };
                                        if !target_tid.is_empty() {
                                            crate::tinyvex_write::mirror_acp_update_to_tinyvex(&state_for_stdout, "codex", &target_tid, &update).await;
                                        }
                                        {
                                            let acp_session = state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default();
                                            let notif = SessionNotification { session_id: SessionId(acp_session.into()), update, meta: None };
                                            let update_kind = match &notif.update {
                                                agent_client_protocol::SessionUpdate::UserMessageChunk(_) => "user_message_chunk",
                                                agent_client_protocol::SessionUpdate::AgentMessageChunk(_) => "agent_message_chunk",
                                                agent_client_protocol::SessionUpdate::AgentThoughtChunk(_) => "agent_thought_chunk",
                                                agent_client_protocol::SessionUpdate::ToolCall(_) => "tool_call",
                                                agent_client_protocol::SessionUpdate::ToolCallUpdate(_) => "tool_call_update",
                                                agent_client_protocol::SessionUpdate::Plan(_) => "plan",
                                                agent_client_protocol::SessionUpdate::AvailableCommandsUpdate(_) => "available_commands_update",
                                                agent_client_protocol::SessionUpdate::CurrentModeUpdate(_) => "current_mode_update",
                                            };
                                            info!(session_id = %notif.session_id.0, kind = update_kind, "bridge.acp emit");
                                            if let Ok(line) = serde_json::to_string(&serde_json::json!({ "type": "bridge.acp", "notification": notif })) { let _ = tx_out.send(line); }
                                        }
                                    }
                                }
                            } else {
                                // Ignore non-tool item.* rows here; chat is handled by the dedicated paths above.
                            }
                        }
                    }
                }
            }
            // Broadcast raw line
            let _ = tx_out.send(line.clone());
        }
        // Drain stderr to console (quiet by default)
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let quiet = std::env::var("BRIDGE_QUIET_STDERR").ok().map(|v| v != "0").unwrap_or(true);
        while let Ok(Some(line)) = lines.next_line().await {
            if quiet {
                let low = line.to_ascii_lowercase();
                if low.contains("error") || low.contains("warn") {
                    eprintln!("[codex/stderr] {}", line);
                }
                continue;
            }
            eprintln!("[codex/stderr] {}", line);
        }
    });
    Ok(())
}

/// Send SIGINT (or taskkill) to the Codex process group to abort a running turn.
async fn interrupt_running_child(state: &Arc<AppState>) -> Result<()> {
    let pid_opt = { state.child_pid.lock().await.clone() };
    match pid_opt {
        Some(pid) => match send_interrupt_signal(pid) {
            Ok(_) => {
                info!(pid, "sent interrupt signal to codex child");
                Ok(())
            }
            Err(e) => Err(e.context("failed to send interrupt signal to codex child")),
        },
        None => {
            info!("msg" = "no child pid recorded when interrupt requested");
            Ok(())
        }
    }
}

#[cfg(unix)]
/// UNIX: send SIGINT to the whole process group so child and descendants stop.
fn send_interrupt_signal(pid: u32) -> Result<()> {
    use std::io::ErrorKind;
    let pid_i32: i32 = pid.try_into().context("pid out of range for SIGINT")?;
    let target = -pid_i32;
    let res = unsafe { libc::kill(target, libc::SIGINT) };
    if res == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if err.kind() == ErrorKind::NotFound {
        return Ok(());
    }
    Err(anyhow::Error::from(err).context("libc::kill(SIGINT) failed"))
}

#[cfg(windows)]
/// Windows: use `taskkill /T` to terminate the process tree.
fn send_interrupt_signal(pid: u32) -> Result<()> {
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status()
        .context("failed to spawn taskkill for interrupt")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("taskkill exited with status {status:?}"))
    }
}

/// Best‑effort helper to pull a `cd` path out of the first JSON line in a WS payload.
fn extract_cd_from_ws_payload(payload: &str) -> Option<PathBuf> {
    let first_line = payload.lines().next()?.trim();
    if !first_line.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first_line).ok()?;
    let cd = v.get("cd").and_then(|s| s.as_str())?;
    if cd.is_empty() {
        return None;
    }
    Some(expand_home(cd))
}

/// Best‑effort helper to pull a `resume` token out of the first JSON line.
fn extract_resume_from_ws_payload(payload: &str) -> Option<String> {
    let first_line = payload.lines().next()?.trim();
    if !first_line.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first_line).ok()?;
    match v.get("resume") {
        Some(JsonValue::String(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Compute a resume argument for Codex CLI from an optional resume token.
/// Defaults to "last" if none provided to keep threads flowing.
#[allow(dead_code)]
fn compute_resume_arg(resume: Option<&str>) -> Option<String> {
    match resume {
        Some("new") | Some("none") => None,
        Some(s) if !s.is_empty() => Some(s.to_string()),
        _ => Some("last".to_string()),
    }
}

/// Map a raw item type to the internal message kind used in Convex.
fn map_tool_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "command_execution" => Some("cmd"),
        "file_change" => Some("file"),
        "web_search" => Some("search"),
        "mcp_tool_call" => Some("mcp"),
        "todo_list" => Some("todo"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cd_line() {
        let p = extract_cd_from_ws_payload("{\"cd\":\"~/code\"}");
        assert!(p.is_some());
    }

    #[test]
    fn parses_resume_line() {
        assert_eq!(
            extract_resume_from_ws_payload("{\"resume\":\"last\"}"),
            Some("last".into())
        );
        assert!(extract_resume_from_ws_payload("{\"foo\":1}").is_none());
    }

    #[test]
    fn maps_tool_kinds() {
        assert_eq!(map_tool_kind("command_execution"), Some("cmd"));
        assert_eq!(map_tool_kind("file_change"), Some("file"));
        assert_eq!(map_tool_kind("web_search"), Some("search"));
        assert_eq!(map_tool_kind("mcp_tool_call"), Some("mcp"));
        assert_eq!(map_tool_kind("todo_list"), Some("todo"));
        assert_eq!(map_tool_kind("other"), None);
        assert_eq!(map_tool_kind("user_message"), None);
    }

    #[test]
    fn computes_resume_defaults() {
        assert_eq!(compute_resume_arg(None).as_deref(), Some("last"));
        assert_eq!(compute_resume_arg(Some("last")).as_deref(), Some("last"));
        assert_eq!(compute_resume_arg(Some("new")).as_deref(), None);
        assert_eq!(compute_resume_arg(Some("none")).as_deref(), None);
        assert_eq!(compute_resume_arg(Some("abc")).as_deref(), Some("abc"));
    }
}

#[allow(dead_code)]
fn create_demo_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS oa_demo (id INTEGER PRIMARY KEY, k TEXT, v TEXT)",
        rusqlite::params![],
    )?;
    Ok(())
}
#[allow(dead_code)]
fn create_threads_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute("CREATE TABLE IF NOT EXISTS threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL, title TEXT, resume_id TEXT, project_id TEXT, source TEXT, created_at INTEGER, updated_at INTEGER)", rusqlite::params![])?;
    Ok(())
}
#[allow(dead_code)]
fn insert_demo_thread(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    let id = format!("demo-{}", now_ms());
    conn.execute("INSERT OR REPLACE INTO threads (id, rollout_path, title, resume_id, project_id, source, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", rusqlite::params![id, "~/.codex/sessions/demo.jsonl", "Demo Thread", "", "", "demo", now_ms() as i64, now_ms() as i64])?;
    Ok(())
}
