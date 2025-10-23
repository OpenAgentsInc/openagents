#[cfg(unix)]
use std::convert::TryInto;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result, anyhow};
use axum::extract::ws::{Message, WebSocket};
use axum::{
    Router, extract::State, extract::WebSocketUpgrade, response::IntoResponse, routing::get,
};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde_json::Value as JsonValue;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, broadcast},
};
use tracing::{error, info};
use tracing_subscriber::prelude::*;

mod history;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "codex-bridge",
    about = "WebSocket bridge to Codex CLI",
    version
)]
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

const MAX_HISTORY_LINES: usize = 2000;

struct AppState {
    tx: broadcast::Sender<String>,
    child_stdin: Mutex<Option<tokio::process::ChildStdin>>, // drop after first write to signal EOF
    child_pid: Mutex<Option<u32>>,
    opts: Opts,
    // Track last seen session id so we can resume on subsequent prompts
    last_thread_id: Mutex<Option<String>>,
    // Replay buffer for new websocket clients
    history: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = Opts::parse();

    let (mut child, tx) = spawn_codex(&opts).await?;
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(Some(child.stdin.take().context("child stdin missing")?)),
        child_pid: Mutex::new(Some(child.pid)),
        opts: opts.clone(),
        last_thread_id: Mutex::new(None),
        history: Mutex::new(Vec::new()),
    });

    // Start readers for stdout/stderr → broadcast + console
    start_stream_forwarders(child, state.clone()).await?;

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/history", get(history::history_handler))
        .route("/session", get(history::session_handler))
        .route("/thread", get(history::thread_handler))
        .with_state(state);

    info!("binding" = %opts.bind, "msg" = "codex-bridge listening (routes: /ws, /history, /thread [/session alias])");
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
                    if let Some(cmd) = parse_control_command(&t) {
                        info!(?cmd, "ws control command");
                        match cmd {
                            ControlCommand::Interrupt => {
                                if let Err(e) = interrupt_running_child(&stdin_state).await {
                                    error!(?e, "failed to interrupt codex child");
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
                    let desired_resume = extract_resume_from_ws_payload(&t); // "last" | session id | "new"/"none" (start fresh)
                    info!(?desired_cd, ?desired_resume, msg = "parsed ws preface");
                    // Ensure we have a live codex stdin; respawn if needed
                    let need_respawn = { stdin_state.child_stdin.lock().await.is_none() };
                    if need_respawn || desired_cd.is_some() {
                        // Decide on resume: prefer explicit resume id from preface; otherwise use last captured thread id
                        let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                        let resume_arg: Option<String> = match desired_resume.as_deref() {
                            Some("new") | Some("none") => None,
                            Some("last") => resume_id.clone(),
                            Some(s) if !s.is_empty() => Some(s.to_string()),
                            _ => resume_id.clone(),
                        };
                        // If we already have a stdin but need to honor a cd, close it to end the previous child
                        if !need_respawn && desired_cd.is_some() {
                            let mut g = stdin_state.child_stdin.lock().await;
                            let _ = g.take(); // drop to close stdin and let old child exit
                        }
                        match spawn_codex_child_only_with_dir(
                            &stdin_state.opts,
                            desired_cd.clone(),
                            resume_arg.as_deref(),
                        )
                        .await
                        {
                            Ok(mut child) => {
                                {
                                    let mut pid_lock = stdin_state.child_pid.lock().await;
                                    *pid_lock = Some(child.pid);
                                }
                                if let Some(stdin) = child.stdin.take() {
                                    *stdin_state.child_stdin.lock().await = Some(stdin);
                                } else {
                                    error!("respawned codex missing stdin");
                                }
                                // start forwarding for new child
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
                        // Always-resume mode: no injection fallback
                        if !data.ends_with('\n') {
                            data.push('\n');
                        }
                        let write_preview = if data.len() > 160 {
                            format!("{}…", &data[..160].replace('\n', "\\n"))
                        } else {
                            data.replace('\n', "\\n")
                        };
                        info!(
                            "msg" = "writing to child stdin",
                            bytes = write_preview.len(),
                            preview = write_preview
                        );
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
                        let resume_id = { stdin_state.last_thread_id.lock().await.clone() };
                        match spawn_codex_child_only_with_dir(
                            &stdin_state.opts,
                            None,
                            resume_id.as_deref(),
                        )
                        .await
                        {
                            Ok(mut child) => {
                                {
                                    let mut pid_lock = stdin_state.child_pid.lock().await;
                                    *pid_lock = Some(child.pid);
                                }
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
    pid: u32,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
}

async fn spawn_codex(opts: &Opts) -> Result<(ChildWithIo, broadcast::Sender<String>)> {
    let (bin, args) = build_bin_and_args(opts)?; // initial spawn: never add resume here
    let workdir = detect_repo_root(None);
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "spawning codex"
    );
    let mut command = Command::new(&bin);
    command
        .current_dir(&workdir)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            // Put the child in its own process group so we can signal the group
            let res = libc::setpgid(0, 0);
            if res != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("failed to spawn codex")?;

    let pid = child.id().context("child pid missing")?;
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

    Ok((
        ChildWithIo {
            pid,
            stdin,
            stdout,
            stderr,
        },
        tx,
    ))
}

// Note: single-purpose respawns are handled by spawn_codex_child_only_with_dir.

async fn spawn_codex_child_only_with_dir(
    opts: &Opts,
    workdir_override: Option<PathBuf>,
    resume_id: Option<&str>,
) -> Result<ChildWithIo> {
    let (bin, mut args) = build_bin_and_args(opts)?;
    // Attach resume args when requested; automatically fall back if the CLI
    // doesn't support exec resume on this machine.
    if let Some(rid) = resume_id {
        let supports = cli_supports_resume(&bin);
        if supports {
            if rid == "last" {
                info!(msg = "enabling resume --last");
                args.push("resume".into());
                args.push("--last".into());
                // No positional dash: exec reads from stdin when no prompt arg is provided
            } else {
                info!(resume = rid, msg = "enabling resume by id");
                args.push("resume".into());
                args.push(rid.into());
                // No positional dash: exec reads from stdin when no prompt arg is provided
            }
        } else {
            info!(
                requested = rid,
                msg = "exec resume not supported by codex binary; spawning without resume"
            );
        }
    }
    let workdir = workdir_override.unwrap_or_else(|| detect_repo_root(None));
    info!(
        "bin" = %bin.display(),
        "args" = ?args,
        "workdir" = %workdir.display(),
        "msg" = "respawn codex for new prompt"
    );
    let mut command = Command::new(&bin);
    command
        .current_dir(&workdir)
        .args(&args)
        .stdin(Stdio::piped())
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
    let mut child = command.spawn().context("failed to spawn codex")?;
    let pid = child.id().context("child pid missing")?;
    let stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });
    Ok(ChildWithIo {
        pid,
        stdin,
        stdout,
        stderr,
    })
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
    // Do not attach resume here; we add it per-message when respawning after a
    // prior thread id is known.
    if !opts.extra.is_empty() {
        args.extend(opts.extra.clone());
    }

    fn contains_flag(args: &[String], short: &str, long: &str) -> bool {
        args.iter().any(|a| {
            a == short
                || a == long
                || a.starts_with(&format!("{short}="))
                || a.starts_with(&format!("{long}="))
        })
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
    if !args
        .iter()
        .any(|a| a == "--dangerously-bypass-approvals-and-sandbox")
    {
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
    // Strict detection: only treat as supported if `codex exec resume --help` succeeds.
    match std::process::Command::new(bin)
        .args(["exec", "resume", "--help"]) // exists only on resume-capable builds
        .output()
    {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

async fn start_stream_forwarders(mut child: ChildWithIo, state: Arc<AppState>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;

    let tx_out = state.tx.clone();
    let state_for_stdout = state.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // Drop noisy CLI line "Reading prompt from stdin..." before any logging/broadcast
            if line.trim().to_ascii_lowercase().contains("reading prompt from stdin") {
                continue;
            }
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            println!("{}", log);
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stdout", raw = %line);
            }
            // Try to capture thread id for resume
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
                    }
                }
                // no-op for agent_message in always-resume mode
            }
            {
                let mut history = state_for_stdout.history.lock().await;
                history.push(line.clone());
                if history.len() > MAX_HISTORY_LINES {
                    let drop = history.len() - MAX_HISTORY_LINES;
                    history.drain(0..drop);
                }
            }
            let _ = tx_out.send(line);
        }
        info!("msg" = "stdout stream ended");
    });

    let tx_err = state.tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // Drop noisy CLI line "Reading prompt from stdin..." before any logging/broadcast
            if line.trim().to_ascii_lowercase().contains("reading prompt from stdin") {
                continue;
            }
            let log = summarize_exec_delta_for_log(&line).unwrap_or_else(|| line.clone());
            eprintln!("{}", log);
            if line.contains("\"sandbox\"") || line.contains("sandbox") {
                info!("msg" = "observed sandbox string from stderr", raw = %line);
            }
            {
                let mut history = state.history.lock().await;
                history.push(line.clone());
                if history.len() > MAX_HISTORY_LINES {
                    let drop = history.len() - MAX_HISTORY_LINES;
                    history.drain(0..drop);
                }
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

    // Check top-level and nested msg without overlapping borrows
    let is_top = root.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta");
    let mut_target: Option<&mut JsonValue> = if is_top {
        Some(&mut root)
    } else {
        let msg_opt = {
            if let Some(obj) = root.as_object_mut() {
                obj.get_mut("msg")
            } else {
                None
            }
        };
        if let Some(m) = msg_opt {
            if m.get("type").and_then(|t| t.as_str()) == Some("exec_command_output_delta") {
                Some(m)
            } else {
                None
            }
        } else {
            None
        }
    };
    let tgt = match mut_target {
        Some(t) => t,
        None => return None,
    };

    // Replace large fields
    if let Some(arr) = tgt.get_mut("chunk").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }
    if let Some(arr) = tgt.get_mut("chunk_bytes").and_then(|v| v.as_array_mut()) {
        let len = arr.len();
        *tgt.get_mut("chunk_bytes").unwrap() = JsonValue::String(format!("[{} elements]", len));
    }

    Some(match serde_json::to_string(&root) {
        Ok(s) => s,
        Err(_) => return None,
    })
}

#[derive(Debug, Clone, Copy)]
enum ControlCommand {
    Interrupt,
}

fn parse_control_command(payload: &str) -> Option<ControlCommand> {
    let mut lines = payload.lines();
    let first = lines.next()?.trim();
    if lines.next().is_some() {
        return None;
    }
    if !first.starts_with('{') {
        return None;
    }
    let v: JsonValue = serde_json::from_str(first).ok()?;
    let control = v.get("control").and_then(|c| c.as_str())?;
    match control {
        "interrupt" => Some(ControlCommand::Interrupt),
        _ => None,
    }
}

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
fn send_interrupt_signal(pid: u32) -> Result<()> {
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status()
        .context("failed to spawn taskkill for interrupt")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("taskkill exited with status {status:?}"))
    }
}

/// Detect the repository root directory so Codex runs from the right place.
/// Heuristics:
/// - Prefer the nearest ancestor that contains both `expo/` and `crates/` directories.
/// - If not found, fall back to the process current_dir.
fn detect_repo_root(start: Option<PathBuf>) -> PathBuf {
    fn is_repo_root(p: &Path) -> bool {
        p.join("expo").is_dir() && p.join("crates").is_dir()
    }

    let mut cur =
        start.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let original = cur.clone();
    loop {
        if is_repo_root(&cur) {
            return cur;
        }
        if !cur.pop() {
            // reached filesystem root
            return original;
        }
    }
}

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
    Some(PathBuf::from(cd))
}

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
