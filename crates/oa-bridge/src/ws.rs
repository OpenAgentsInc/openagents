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
    try_finalize_stream_kind,
};
use crate::projects::Project;
use acp_event_translator::translate_codex_event_to_acp_update;
use agent_client_protocol::{SessionId, SessionNotification};
use crate::state::AppState;
use crate::util::{expand_home, now_ms};

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
                                            *stdin_state.current_convex_thread.lock().await = Some(thread_doc_id.clone());
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
                                    *stdin_state.current_convex_thread.lock().await =
                                        Some(thread_doc_id.clone());
                                }
                                // Only resume when we have an explicit id or a captured last thread
                                let last_id = { stdin_state.last_thread_id.lock().await.clone() };
                                let resume_arg = match resume_id.as_deref() {
                                    Some("new") | Some("none") => None,
                                    Some("last") => last_id,
                                    Some(s) if !s.is_empty() => Some(s.to_string()),
                                    _ => last_id,
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
        let tvx = tinyvex::Tinyvex::open(tempfile::NamedTempFile::new().unwrap().path()).unwrap();
        Arc::new(AppState {
            tx,
            child_stdin: Mutex::new(None),
            child_pid: Mutex::new(None),
            opts,
            last_thread_id: Mutex::new(None),
            history: Mutex::new(Vec::new()),
            current_convex_thread: Mutex::new(None),
            stream_track: Mutex::new(std::collections::HashMap::new()),
            convex_ready: std::sync::atomic::AtomicBool::new(true),
            tinyvex: std::sync::Arc::new(tvx),
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
                        let dbg = serde_json::json!({
                                "type": "bridge.codex_event",
                                "event_type": "thread.started",
                                "thread_id": v.get("thread_id").and_then(|x| x.as_str()).or_else(|| v.get("msg").and_then(|m| m.get("thread_id")).and_then(|x| x.as_str())).unwrap_or("")
                            }).to_string();
                        let _ = tx_out.send(dbg);
                    }
                }
                if matches!(t.as_deref(), Some("turn.completed")) {
                    let convex_tid_opt =
                        { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                        { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                    }
                }
                if matches!(t.as_deref(), Some("reasoning.delta"))
                    || matches!(t.as_deref(), Some("reason.delta"))
                {
                    let payload = v
                        .get("payload")
                        .or_else(|| v.get("msg").and_then(|m| m.get("payload")));
                    let txt = payload
                        .and_then(|p| p.get("text"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let convex_tid_opt =
                        { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                        stream_upsert_or_append(&state_for_stdout, &target_tid, "reason", &txt)
                            .await;
                        info!(len=txt.len(), thread=%target_tid, "reason.delta mapped");
                        let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.delta","len":txt.len(),"thread":target_tid}).to_string();
                        let _ = tx_out.send(dbg);
                    }
                }
                if matches!(t.as_deref(), Some("agent_message"))
                    || matches!(t.as_deref(), Some("assistant"))
                    || matches!(t.as_deref(), Some("message"))
                {
                    let payload = v
                        .get("payload")
                        .or_else(|| v.get("msg").and_then(|m| m.get("payload")));
                    let text_owned = payload
                        .and_then(|p| p.get("text"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let convex_tid_opt =
                        { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                        let final_text = text_owned.clone();
                        if !try_finalize_stream_kind(
                            &state_for_stdout,
                            &target_tid,
                            "assistant",
                            &final_text,
                        )
                        .await
                        {
                            // Tinyvex: snapshot handled by finalize path
                            info!(thread=%target_tid, "assistant.finalized created snapshot");
                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.final","thread":target_tid}).to_string();
                            let _ = tx_out.send(dbg);
                            let dbg2 = serde_json::json!({"type":"bridge.assistant_written","thread":target_tid, "len": final_text.len()}).to_string();
                            let _ = tx_out.send(dbg2);
                        } else {
                            info!(thread=%target_tid, "assistant.finalized finalized streamed item");
                        }
                    }
                }
                if matches!(t.as_deref(), Some("reasoning")) {
                    let mut text_owned = v
                        .get("payload")
                        .and_then(|p| p.get("text"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    if text_owned.trim().is_empty() {
                        if let Some(arr) = v
                            .get("payload")
                            .and_then(|p| p.get("summary"))
                            .and_then(|x| x.as_array())
                        {
                            for part in arr {
                                if let Some(t) = part.get("text").and_then(|x| x.as_str()) {
                                    if !text_owned.is_empty() {
                                        text_owned.push('\n');
                                    }
                                    text_owned.push_str(t);
                                }
                            }
                        }
                    }
                    if !text_owned.trim().is_empty() {
                        let convex_tid_opt =
                            { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                            if !try_finalize_stream_kind(
                                &state_for_stdout,
                                &target_tid,
                                "reason",
                                &text_owned,
                            )
                            .await
                            {
                                // Tinyvex: snapshot handled by finalize path if needed
                            }
                            let dbg2 = serde_json::json!({"type":"bridge.reason_written","thread":target_tid}).to_string();
                            let _ = tx_out.send(dbg2);
                        }
                    }
                }
                // Newer shapes: item.delta / item.completed with item.type = agent_message | reasoning
                if let Some(ty) = t.as_deref() {
                    if ty == "item.delta" || ty == "item.completed" {
                        if let Some(item) = v.get("item").or_else(|| v.get("payload").and_then(|p| p.get("item"))) {
                            let kind = item.get("type").and_then(|x| x.as_str()).unwrap_or("");
                            let txt = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
                            let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
                            let target_tid = if let Some(s) = convex_tid_opt { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() };
                            if !target_tid.is_empty() {
                                match kind {
                                    "agent_message" => {
                                        if ty == "item.delta" {
                                            stream_upsert_or_append(&state_for_stdout, &target_tid, "assistant", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.delta","len":txt.len(),"thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        } else {
                                            if !try_finalize_stream_kind(&state_for_stdout, &target_tid, "assistant", txt).await {
                                                // Tinyvex: snapshot handled by finalize path if needed
                                            }
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"assistant.final","thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        }
                                    }
                                    "reasoning" => {
                                        if ty == "item.delta" {
                                            stream_upsert_or_append(&state_for_stdout, &target_tid, "reason", txt).await;
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.delta","len":txt.len(),"thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        } else {
                                            if !try_finalize_stream_kind(&state_for_stdout, &target_tid, "reason", txt).await {
                                                // Tinyvex: snapshot handled by finalize path if needed
                                            }
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.final","thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        }
                                    }
                                    _ => {}
                                }
                                // Mirror ACP update into Tinyvex (no-op for MVP) and optionally emit for debugging
                                if let Some(update) = translate_codex_event_to_acp_update(&v) {
                                    let target_tid = {
                                        let ctid = state_for_stdout.current_convex_thread.lock().await.clone();
                                        if let Some(s) = ctid { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() }
                                    };
                                    if !target_tid.is_empty() {
                                        crate::tinyvex_write::mirror_acp_update_to_convex(&state_for_stdout, &target_tid, &update).await;
                                    }
                                    if std::env::var("BRIDGE_ACP_EMIT").ok().as_deref() == Some("1") {
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
                                    { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                                            let ctid = state_for_stdout.current_convex_thread.lock().await.clone();
                                            if let Some(s) = ctid { s } else { state_for_stdout.last_thread_id.lock().await.clone().unwrap_or_default() }
                                        };
                                        if !target_tid.is_empty() {
                                            crate::tinyvex_write::mirror_acp_update_to_convex(&state_for_stdout, &target_tid, &update).await;
                                        }
                                        if std::env::var("BRIDGE_ACP_EMIT").ok().as_deref() == Some("1") {
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
                            } else if kind == "agent_message" || kind == "reasoning" {
                                // Treat agent final messages and reasoning as chat rows in Tinyvex
                                let text = payload.get("text").and_then(|x| x.as_str()).unwrap_or("");
                                let convex_tid_opt = { state_for_stdout.current_convex_thread.lock().await.clone() };
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
                                    // Map to our Tinyvex kinds: agent_message -> assistant, reasoning -> reason
                                    let tvx_kind = if kind == "agent_message" { "assistant" } else { "reason" };
                                    crate::tinyvex_write::stream_upsert_or_append(&state_for_stdout, &target_tid, tvx_kind, text).await;
                                    let _ = crate::tinyvex_write::try_finalize_stream_kind(&state_for_stdout, &target_tid, tvx_kind, text).await;
                                }
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
