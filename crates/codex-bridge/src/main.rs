use std::{path::{Path, PathBuf}, process::Stdio, sync::Arc};

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
                    let preview = if t.len() > 180 { format!("{}…", &t[..180].replace('\n', "\\n")) } else { t.replace('\n', "\\n") };
                    info!("msg" = "ws text received", size = t.len(), preview = preview);
                    let desired_cd = extract_cd_from_ws_payload(&t);
                    // Ensure we have a live codex stdin; respawn if needed
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn || desired_cd.is_some() {
                        // If we already have a stdin but need to honor a cd, close it to end the previous child
                        if !need_respawn && desired_cd.is_some() {
                            let mut g = stdin_state.child_stdin.lock().await;
                            let _ = g.take(); // drop to close stdin and let old child exit
                        }
                        match spawn_codex_child_only_with_dir(&stdin_state.opts, desired_cd.clone()).await {
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
                        let write_preview = if data.len() > 160 { format!("{}…", &data[..160].replace('\n', "\\n")) } else { data.replace('\n', "\\n") };
                        info!("msg" = "writing to child stdin", bytes = write_preview.len(), preview = write_preview);
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
                        match spawn_codex_child_only_with_dir(&stdin_state.opts, None).await {
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
    let workdir = detect_repo_root(None);
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "spawning codex"
    );
    let mut child = Command::new(bin)
        .current_dir(&workdir)
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
    let workdir = detect_repo_root(None);
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "respawn codex for new prompt"
    );
    let mut child = Command::new(bin)
        .current_dir(&workdir)
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

async fn spawn_codex_child_only_with_dir(opts: &Opts, workdir_override: Option<PathBuf>) -> Result<ChildWithIo> {
    let (bin, args) = build_bin_and_args(opts)?;
    let workdir = workdir_override.unwrap_or_else(|| detect_repo_root(None));
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "respawn codex for new prompt"
    );
    let mut child = Command::new(bin)
        .current_dir(&workdir)
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
    // Ensure we resume the most recent session instead of starting fresh each time,
    // but only if the installed CLI supports the `resume` subcommand.
    if !args.iter().any(|a| a == "resume") && cli_supports_resume(&bin) {
        args.push("resume".into());
        args.push("--last".into());
    }
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
    // Ensure explicit sandbox + approvals flags so the CLI reports the correct state
    if !contains_flag(&args, "-s", "--sandbox") {
        pre_flags.push("-s".into());
        pre_flags.push("danger-full-access".into());
    }
    // Do not add explicit approvals when using bypass; the bypass implies no approvals
    // Strongly hint full disk access and override default preset via config
    if !contains_substring(&args, "sandbox_permissions=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"sandbox_permissions=["disk-full-access"]"#.into());
    }
    if !contains_substring(&args, "sandbox_mode=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"sandbox_mode="danger-full-access""#.into());
    }
    if !contains_substring(&args, "approval_policy=") {
        pre_flags.push("-c".into());
        pre_flags.push(r#"approval_policy="never""#.into());
    }

    if !pre_flags.is_empty() {
        let mut updated = pre_flags;
        updated.extend(args);
        args = updated;
    }

    Ok((bin, args))
}

fn cli_supports_resume(bin: &PathBuf) -> bool {
    let out = std::process::Command::new(bin)
        .args(["exec", "--help"])
        .output();
    match out {
        Ok(o) => {
            let help = String::from_utf8_lossy(&o.stdout);
            help.contains("Resume") || help.contains("resume --last") || help.contains("resume")
        }
        Err(_) => false,
    }
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
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stdout", raw = %line);
            }
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
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stderr", raw = %line);
            }
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

/// Detect the repository root directory so Codex runs from the right place.
/// Heuristics:
/// - Prefer the nearest ancestor that contains both `expo/` and `crates/` directories.
/// - If not found, fall back to the process current_dir.
fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &Path) -> bool {
        p.join("expo").is_dir() && p.join("crates").is_dir()
    }

    let mut cur = start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) {
            return cur;
        }
        if !cur.pop() { // reached filesystem root
            return original;
        }
    }
}

fn extract_cd_from_ws_payload(payload: &str) -> Option<PathBuf> {
    let first_line = payload.lines().next()?.trim();
    if !first_line.starts_with('{') { return None; }
    let v: JsonValue = serde_json::from_str(first_line).ok()?;
    let cd = v.get("cd").and_then(|s| s.as_str())?;
    if cd.is_empty() { return None; }
    Some(PathBuf::from(cd))
}
