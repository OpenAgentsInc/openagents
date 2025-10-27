//! WebSocket server handlers and control routing.
//!
//! This module exposes the Axum `/ws` route and contains the socket read/write
//! loops, control message dispatch, and child/stdout forwarder that maps Codex
//! JSONL into Convex writes via `crate::convex_write`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::ws::{Message, WebSocket};
use axum::{extract::State, extract::WebSocketUpgrade, response::IntoResponse};
use futures::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

use crate::bootstrap::{convex_health, default_convex_db};
use crate::codex_runner::{ChildWithIo, spawn_codex_child_only_with_dir};
use crate::controls::{ControlCommand, parse_control_command};
use crate::convex_write::{
    finalize_streaming_for_thread, stream_upsert_or_append, summarize_exec_delta_for_log,
    try_finalize_stream_kind,
};
use crate::state::AppState;
use crate::util::{expand_home, list_sqlite_tables, now_ms};

/// Axum handler for the `/ws` route. Upgrades to a WebSocket and delegates to `handle_socket`.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
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
                                let last_thread_id =
                                    { stdin_state.last_thread_id.lock().await.clone() };
                                let bind = stdin_state.opts.bind.clone();
                                let convex_url =
                                    format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let convex_healthy =
                                    convex_health(&convex_url).await.unwrap_or(false);
                                let line = serde_json::json!({
                                    "type": "bridge.status",
                                    "bind": bind,
                                    "codex_pid": codex_pid,
                                    "last_thread_id": last_thread_id,
                                    "convex_url": convex_url,
                                    "convex_healthy": convex_healthy
                                })
                                .to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexStatus => {
                                let url =
                                    format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let db = stdin_state
                                    .opts
                                    .convex_db
                                    .clone()
                                    .unwrap_or_else(crate::bootstrap::default_convex_db);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = if healthy {
                                    list_sqlite_tables(&db).unwrap_or_default()
                                } else {
                                    Vec::new()
                                };
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateDemo => {
                                let db = stdin_state
                                    .opts
                                    .convex_db
                                    .clone()
                                    .unwrap_or_else(default_convex_db);
                                let _ = create_demo_table(&db);
                                let url =
                                    format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateThreads => {
                                let db = stdin_state
                                    .opts
                                    .convex_db
                                    .clone()
                                    .unwrap_or_else(default_convex_db);
                                let _ = create_threads_table(&db);
                                let url =
                                    format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexCreateDemoThread => {
                                let db = stdin_state
                                    .opts
                                    .convex_db
                                    .clone()
                                    .unwrap_or_else(default_convex_db);
                                let _ = create_threads_table(&db);
                                let _ = insert_demo_thread(&db);
                                let url =
                                    format!("http://127.0.0.1:{}", stdin_state.opts.convex_port);
                                let healthy = convex_health(&url).await.unwrap_or(false);
                                let tables = list_sqlite_tables(&db).unwrap_or_default();
                                let line = serde_json::json!({"type":"bridge.convex_status","healthy": healthy, "url": url, "db": db, "tables": tables}).to_string();
                                let _ = stdin_state.tx.send(line);
                            }
                            ControlCommand::ConvexBackfill => {
                                let base =
                                    std::env::var("CODEXD_HISTORY_DIR").ok().unwrap_or_else(|| {
                                        std::env::var("HOME")
                                            .map(|h| format!("{}/.codex/sessions", h))
                                            .unwrap_or_else(|_| ".".into())
                                    });
                                let limit = 400usize;
                                match crate::history::scan_history(
                                    std::path::Path::new(&base),
                                    limit,
                                ) {
                                    Ok(items) => {
                                        use convex::{ConvexClient, Value};
                                        use std::collections::BTreeMap;
                                        let url = format!(
                                            "http://127.0.0.1:{}",
                                            stdin_state.opts.convex_port
                                        );
                                        let mut client = match ConvexClient::new(&url).await {
                                            Ok(c) => c,
                                            Err(e) => {
                                                error!(
                                                    ?e,
                                                    "convex client init failed for backfill"
                                                );
                                                let line = serde_json::json!({"type":"bridge.convex_backfill","status":"error","error":"convex init failed"}).to_string();
                                                let _ = stdin_state.tx.send(line);
                                                continue;
                                            }
                                        };
                                        for h in items.clone() {
                                            if let Some(path) = crate::history::resolve_session_path(
                                                std::path::Path::new(&base),
                                                Some(&h.id),
                                                Some(&h.path),
                                            ) {
                                                if let Ok(th) = crate::history::parse_thread(
                                                    std::path::Path::new(&path),
                                                ) {
                                                    let resume_id = th
                                                        .resume_id
                                                        .clone()
                                                        .unwrap_or(h.id.clone());
                                                    let title = th.title.clone();
                                                    let started_ms = th.started_ts.map(|t| t * 1000)
                                                        .or_else(|| crate::history::derive_started_ts_from_path(std::path::Path::new(&path)).map(|t| t * 1000))
                                                        .unwrap_or_else(|| now_ms());
                                                    let mut targs: BTreeMap<String, Value> =
                                                        BTreeMap::new();
                                                    targs.insert(
                                                        "threadId".into(),
                                                        Value::from(resume_id.clone()),
                                                    );
                                                    targs.insert(
                                                        "resumeId".into(),
                                                        Value::from(resume_id.clone()),
                                                    );
                                                    targs.insert(
                                                        "title".into(),
                                                        Value::from(title.clone()),
                                                    );
                                                    targs.insert(
                                                        "createdAt".into(),
                                                        Value::from(started_ms as f64),
                                                    );
                                                    targs.insert(
                                                        "updatedAt".into(),
                                                        Value::from(started_ms as f64),
                                                    );
                                                    let _ = client
                                                        .mutation("threads:upsertFromStream", targs)
                                                        .await;
                                                    for it in th.items {
                                                        if it.kind == "message" {
                                                            let role = it
                                                                .role
                                                                .as_deref()
                                                                .unwrap_or("assistant");
                                                            let text = it.text;
                                                            let mut margs: BTreeMap<String, Value> =
                                                                BTreeMap::new();
                                                            margs.insert(
                                                                "threadId".into(),
                                                                Value::from(resume_id.clone()),
                                                            );
                                                            margs.insert(
                                                                "role".into(),
                                                                Value::from(role),
                                                            );
                                                            margs.insert(
                                                                "kind".into(),
                                                                Value::from("message"),
                                                            );
                                                            margs.insert(
                                                                "text".into(),
                                                                Value::from(text),
                                                            );
                                                            margs.insert(
                                                                "ts".into(),
                                                                Value::from(now_ms() as f64),
                                                            );
                                                            let _ = client
                                                                .mutation("messages:create", margs)
                                                                .await;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        let _ = stdin_state.tx.send(serde_json::json!({"type":"bridge.convex_backfill","status":"enqueued","count": items.len()}).to_string());
                                    }
                                    Err(e) => {
                                        error!(?e, "backfill scan failed");
                                    }
                                }
                            }
                            ControlCommand::RunSubmit {
                                thread_doc_id,
                                text,
                                project_id,
                                resume_id,
                            } => {
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
                                {
                                    *stdin_state.current_convex_thread.lock().await =
                                        Some(thread_doc_id.clone());
                                }
                                let resume_arg = compute_resume_arg(resume_id.as_deref());
                                // The app already persisted the user message via runs:enqueue; avoid duplicating here.
                                match spawn_codex_child_only_with_dir(
                                    &stdin_state.opts,
                                    desired_cd.clone().map(|s| std::path::PathBuf::from(s)),
                                    resume_arg.as_deref(),
                                )
                                .await
                                {
                                    Ok(mut child) => {
                                        if let Some(stdin) = child.stdin.take() {
                                            *stdin_state.child_stdin.lock().await = Some(stdin);
                                        }
                                        if let Err(e) =
                                            start_stream_forwarders(child, stdin_state.clone())
                                                .await
                                        {
                                            error!(?e, "run.submit: forwarders failed");
                                        }
                                        let mut cfg = serde_json::json!({ "sandbox": "danger-full-access", "approval": "never" });
                                        if let Some(cd) = desired_cd.as_deref() {
                                            let cdv: serde_json::Value =
                                                serde_json::Value::String(cd.to_owned());
                                            cfg["cd"] = cdv;
                                        }
                                        if let Some(pid) = project_id.as_deref() {
                                            cfg["project"] = serde_json::json!({ "id": pid });
                                        }
                                        let payload = format!("{}\n{}\n", cfg.to_string(), text);
                                        if let Some(mut stdin) =
                                            stdin_state.child_stdin.lock().await.take()
                                        {
                                            if let Err(e) =
                                                stdin.write_all(payload.as_bytes()).await
                                            {
                                                error!(?e, "run.submit: write failed");
                                            }
                                            let _ = stdin.flush().await;
                                            drop(stdin);
                                            let dbg = serde_json::json!({"type":"bridge.run_submit","threadDocId": thread_doc_id, "cd": desired_cd, "resumeId": resume_id, "len": text.len()}).to_string();
                                            let _ = stdin_state.tx.send(dbg);
                                        }
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
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn || desired_cd.is_some() {
                        let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                        let resume_arg: Option<String> = match desired_resume.as_deref() {
                            Some("new") | Some("none") => None,
                            Some("last") => resume_id.clone(),
                            Some(s) if !s.is_empty() => Some(s.to_string()),
                            _ => resume_id.clone(),
                        };
                        match spawn_codex_child_only_with_dir(
                            &stdin_state.opts,
                            desired_cd.clone(),
                            resume_arg.as_deref(),
                        )
                        .await
                        {
                            Ok(mut child) => {
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                }
                                if let Err(e) =
                                    start_stream_forwarders(child, stdin_state.clone()).await
                                {
                                    error!(?e, "failed starting forwarders for respawned codex");
                                }
                            }
                            Err(e) => {
                                error!(?e, "failed to respawn codex");
                            }
                        }
                    }
                    let mut guard = stdin_state.child_stdin.lock().await;
                    if let Some(mut stdin) = guard.take() {
                        let mut data = t.to_string();
                        let json_first_line = data
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim_start()
                            .starts_with('{');
                        if !json_first_line {
                            data.push('\n');
                        }
                        let preview = if data.len() > 240 {
                            format!("{}…", &data[..240].replace('\n', "\\n"))
                        } else {
                            data.replace('\n', "\\n")
                        };
                        info!(
                            "msg" = "ws write to stdin",
                            size = data.len(),
                            preview = preview
                        );
                        if let Err(e) = stdin.write_all(data.as_bytes()).await {
                            error!(?e, "failed to write to codex stdin");
                            break;
                        }
                        let _ = stdin.flush().await;
                        drop(stdin);
                    } else {
                        error!("stdin already closed; ignoring input");
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
                        let convex_tid_opt =
                            { state_for_stdout.current_convex_thread.lock().await.clone() };
                        use convex::{ConvexClient, Value};
                        use std::collections::BTreeMap;
                        let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                        if let Ok(mut client) = ConvexClient::new(&url).await {
                            let mut args: BTreeMap<String, Value> = BTreeMap::new();
                            if let Some(ctid) = convex_tid_opt.as_deref() {
                                args.insert("threadId".into(), Value::from(ctid));
                            }
                            args.insert("resumeId".into(), Value::from(val));
                            args.insert("title".into(), Value::from("Thread"));
                            args.insert("createdAt".into(), Value::from(now_ms() as f64));
                            args.insert("updatedAt".into(), Value::from(now_ms() as f64));
                            let _ = client.mutation("threads:upsertFromStream", args).await;
                        }
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
                            use convex::{ConvexClient, Value};
                            use std::collections::BTreeMap;
                            let url =
                                format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                            if let Ok(mut client) = ConvexClient::new(&url).await {
                                let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                args.insert("threadId".into(), Value::from(target_tid.clone()));
                                args.insert("role".into(), Value::from("assistant"));
                                args.insert("kind".into(), Value::from("message"));
                                args.insert("text".into(), Value::from(final_text.clone()));
                                args.insert("ts".into(), Value::from(now_ms() as f64));
                                let _ = client.mutation("messages:create", args).await;
                            }
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
                                use convex::{ConvexClient, Value};
                                use std::collections::BTreeMap;
                                let url = format!(
                                    "http://127.0.0.1:{}",
                                    state_for_stdout.opts.convex_port
                                );
                                if let Ok(mut client) = ConvexClient::new(&url).await {
                                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                    args.insert("threadId".into(), Value::from(target_tid.clone()));
                                    args.insert("kind".into(), Value::from("reason"));
                                    args.insert("text".into(), Value::from(text_owned));
                                    args.insert("ts".into(), Value::from(now_ms() as f64));
                                    let _ = client.mutation("messages:create", args).await;
                                }
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
                                                use convex::{ConvexClient, Value}; use std::collections::BTreeMap;
                                                let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                                                if let Ok(mut client) = ConvexClient::new(&url).await {
                                                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                                    args.insert("threadId".into(), Value::from(target_tid.clone()));
                                                    args.insert("role".into(), Value::from("assistant"));
                                                    args.insert("kind".into(), Value::from("message"));
                                                    args.insert("text".into(), Value::from(txt));
                                                    args.insert("ts".into(), Value::from(now_ms() as f64));
                                                    let _ = client.mutation("messages:create", args).await;
                                                }
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
                                                use convex::{ConvexClient, Value}; use std::collections::BTreeMap;
                                                let url = format!("http://127.0.0.1:{}", state_for_stdout.opts.convex_port);
                                                if let Ok(mut client) = ConvexClient::new(&url).await {
                                                    let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                                    args.insert("threadId".into(), Value::from(target_tid.clone()));
                                                    args.insert("kind".into(), Value::from("reason"));
                                                    args.insert("text".into(), Value::from(txt));
                                                    args.insert("ts".into(), Value::from(now_ms() as f64));
                                                    let _ = client.mutation("messages:create", args).await;
                                                }
                                            }
                                            let dbg = serde_json::json!({"type":"bridge.codex_event","event_type":"reason.final","thread":target_tid}).to_string(); let _ = tx_out.send(dbg);
                                        }
                                    }
                                    _ => {}
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
                            if let Some(k) = map_kind {
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
                                    let payload_str = payload.to_string();
                                    use convex::{ConvexClient, Value};
                                    use std::collections::BTreeMap;
                                    let url = format!(
                                        "http://127.0.0.1:{}",
                                        state_for_stdout.opts.convex_port
                                    );
                                    if let Ok(mut client) = ConvexClient::new(&url).await {
                                        let mut args: BTreeMap<String, Value> = BTreeMap::new();
                                        args.insert(
                                            "threadId".into(),
                                            Value::from(target_tid.clone()),
                                        );
                                        args.insert("kind".into(), Value::from(k));
                                        args.insert("text".into(), Value::from(payload_str));
                                        args.insert("ts".into(), Value::from(now_ms() as f64));
                                        let _ = client.mutation("messages:create", args).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Broadcast raw line
            let _ = tx_out.send(line.clone());
        }
        // Drain stderr to console
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
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

fn create_demo_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS oa_demo (id INTEGER PRIMARY KEY, k TEXT, v TEXT)",
        rusqlite::params![],
    )?;
    Ok(())
}
fn create_threads_table(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    conn.execute("CREATE TABLE IF NOT EXISTS threads (id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL, title TEXT, resume_id TEXT, project_id TEXT, source TEXT, created_at INTEGER, updated_at INTEGER)", rusqlite::params![])?;
    Ok(())
}
fn insert_demo_thread(db_path: &PathBuf) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    let id = format!("demo-{}", now_ms());
    conn.execute("INSERT OR REPLACE INTO threads (id, rollout_path, title, resume_id, project_id, source, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", rusqlite::params![id, "~/.codex/sessions/demo.jsonl", "Demo Thread", "", "", "demo", now_ms() as i64, now_ms() as i64])?;
    Ok(())
}
