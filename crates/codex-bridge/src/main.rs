use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{anyhow, Context, Result};
use axum::{extract::State, extract::WebSocketUpgrade, response::IntoResponse, routing::get, Router};
use axum::extract::ws::{Message, WebSocket};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::Command, sync::{broadcast, Mutex}};
use serde_json::Value as JsonValue;
use tracing::{error, info};
use tracing_subscriber::prelude::*;

#[derive(Parser, Debug, Clone)]
#[command(name = "codex-bridge", about = "WebSocket bridge to Codex CLI", version)]
struct Opts {
    /// Bind address for the WebSocket server (e.g., 0.0.0.0:8787)
    #[arg(long, env = "CODEX_BRIDGE_BIND", default_value = "0.0.0.0:8787")]
    bind: String,

    /// Path to the codex binary (falls back to $CODEX_BIN or `codex` in PATH)
    #[arg(long, env = "CODEX_BIN")]
    codex_bin: Option<PathBuf>,

    /// Optional JSON exec args; if empty defaults to: exec --json
    #[arg(long, env = "CODEX_ARGS")] 
    codex_args: Option<String>,

    /// Additional args after `--` are forwarded to codex
    #[arg(trailing_var_arg = true)]
    extra: Vec<String>,
}

struct AppState {
    tx: broadcast::Sender<String>,
    child_stdin: Mutex<Option<tokio::process::ChildStdin>>, // drop after first write to signal EOF
    opts: Opts,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = Opts::parse();

    let (mut child, tx) = spawn_codex(&opts).await?;
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(Some(child.stdin.take().context("child stdin missing")?)),
        opts: opts.clone(),
    });

    // Start readers for stdout/stderr → broadcast + console
    start_stream_forwarders(child, &state.tx).await?;

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    info!("binding" = %opts.bind, "msg" = "codex-bridge listening");
    let listener = tokio::net::TcpListener::bind(&opts.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer())
        .try_init();
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    info!("msg" = "websocket connected");

    // Broadcast reader → socket
    let mut rx = state.tx.subscribe();
    let (mut sink, mut stream) = socket.split();
    let mut sink_task = tokio::spawn(async move {
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
                    info!("msg" = "ws text received", size = t.len());
                    // Ensure we have a live codex stdin; respawn if needed
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn {
                        match spawn_codex_child_only(&stdin_state.opts).await {
                            Ok(mut child) => {
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                } else {
                                    error!("respawned codex missing stdin");
                                }
                                // start forwarding for new child
                                if let Err(e) = start_stream_forwarders(child, &stdin_state.tx).await {
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
                        if !data.ends_with('\n') { data.push('\n'); }
                        if let Err(e) = stdin.write_all(data.as_bytes()).await { 
                            error!(?e, "failed to write to codex stdin");
                            break;
                        }
                        let _ = stdin.flush().await;
                        drop(stdin); // close to send EOF
                    } else {
                        error!("stdin already closed; ignoring input");
                    }
                }
                Message::Binary(b) => {
                    info!("msg" = "ws binary received", size = b.len());
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn {
                        match spawn_codex_child_only(&stdin_state.opts).await {
                            Ok(mut child) => {
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                }
                                if let Err(e) = start_stream_forwarders(child, &stdin_state.tx).await {
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
                        if let Err(e) = stdin.write_all(&b).await { 
                            error!(?e, "failed to write binary to codex stdin");
                            break;
                        }
                        let _ = stdin.flush().await;
                        drop(stdin);
                    } else {
                        error!("stdin already closed; ignoring binary");
                    }
                }
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => {}
            }
        }
    });

    // Await either task end
    tokio::select! {
        _ = (&mut sink_task) => { read_task.abort(); },
        _ = (&mut read_task) => { sink_task.abort(); },
    }
    info!("msg" = "websocket disconnected");
}

struct ChildWithIo {
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

async fn spawn_codex(opts: &Opts) -> Result<(ChildWithIo, broadcast::Sender<String>)> {
    let (bin, args) = build_bin_and_args(opts)?;
    info!("bin" = %bin.display(), "args" = ?args, "msg" = "spawning codex");
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn codex")?;

    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let (tx, _rx) = broadcast::channel::<String>(1024);

    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });

    Ok((ChildWithIo { stdin, stdout, stderr }, tx))
}

async fn spawn_codex_child_only(opts: &Opts) -> Result<ChildWithIo> {
    let (bin, args) = build_bin_and_args(opts)?;
    info!("bin" = %bin.display(), "args" = ?args, "msg" = "respawn codex for new prompt");
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn codex")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });
    Ok(ChildWithIo { stdin, stdout, stderr })
}

fn build_bin_and_args(opts: &Opts) -> Result<(PathBuf, Vec<String>)> {
    let bin = match &opts.codex_bin {
        Some(p) => p.clone(),
        None => which::which("codex").unwrap_or_else(|_| PathBuf::from("codex")),
    };

    let mut args: Vec<String> = if let Some(args_str) = &opts.codex_args {
        shlex::split(args_str).ok_or_else(|| anyhow!("failed to parse CODEX_ARGS"))?
    } else {
        vec!["exec".into(), "--json".into()]
    };
    if !opts.extra.is_empty() { args.extend(opts.extra.clone()); }

    fn contains_flag(args: &[String], short: &str, long: &str) -> bool {
        args.iter().any(|a| a == short || a == long || a.starts_with(&format!("{short}=")) || a.starts_with(&format!("{long}=")))
    }
    fn contains_substring(args: &[String], needle: &str) -> bool {
        args.iter().any(|a| a.contains(needle))
    }

    let mut pre_flags: Vec<String> = Vec::new();
    if !contains_flag(&args, "-m", "--model") {
        pre_flags.push("-m".into());
        pre_flags.push("gpt-5".into());
    }
    if !contains_substring(&args, "model_reasoning_effort=") {
        pre_flags.push("-c".into());
        pre_flags.push("model_reasoning_effort=\"high\"".into());
    }
    if !args.iter().any(|a| a == "--dangerously-bypass-approvals-and-sandbox") {
        pre_flags.push("--dangerously-bypass-approvals-and-sandbox".into());
    }
    if !pre_flags.is_empty() {
        let mut updated = pre_flags;
        updated.extend(args);
        args = updated;
    }

    Ok((bin, args))
}

async fn start_stream_forwarders(mut child: ChildWithIo, tx: &broadcast::Sender<String>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;

    let tx_out = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            println!("{}", log);
            let _ = tx_out.send(line);
        }
        info!("msg" = "stdout stream ended");
    });

    let tx_err = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            eprintln!("{}", log);
            let _ = tx_err.send(line);
        }
        info!("msg" = "stderr stream ended");
    });

    Ok(())
}

fn summarize_exec_delta_for_log(line: &str) -> Option<String> {
    // Try to parse line as JSON and compact large delta arrays for logging only
    let mut root: JsonValue = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return None,
    };

    // Helper to locate nested { msg: { type: ... } }
    fn get_msg_mut(v: &mut JsonValue) -> Option<&mut JsonValue> {
        if let Some(obj) = v.as_object_mut() {
            if let Some(m) = obj.get_mut("msg") { return Some(m); }
        }
        None
    }

    // Check top-level and nested msg without overlapping borrows
    let is_top = root.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta");
    let mut_target: Option<&mut JsonValue> = if is_top {
        Some(&mut root)
    } else {
        let msg_opt = {
            if let Some(obj) = root.as_object_mut() { obj.get_mut("msg") } else { None }
        };
        if let Some(m) = msg_opt {
            if m.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta") { Some(m) } else { None }
        } else { None }
    };
    let tgt = match mut_target { Some(t) => t, None => return None };

    // Replace large fields
    if let Some(arr) = tgt.get_mut("chunk").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }
    if let Some(arr) = tgt.get_mut("chunk_bytes").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk_bytes").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }

    Some(match serde_json::to_string(&root) { Ok(s) => s, Err(_) => return None })
}
