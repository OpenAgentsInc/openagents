use std::{path::PathBuf, process::Stdio, sync::Arc};

use anyhow::{anyhow, Context, Result};
use axum::{extract::State, extract::WebSocketUpgrade, response::IntoResponse, routing::get, Router};
use axum::extract::ws::{Message, WebSocket};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::Command, sync::{broadcast, Mutex}};
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
    child_stdin: Mutex<tokio::process::ChildStdin>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let opts = Opts::parse();

    let (mut child, tx) = spawn_codex(&opts).await?;
    let state = Arc::new(AppState {
        tx,
        child_stdin: Mutex::new(child.stdin.take().context("child stdin missing")?),
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
                    let mut stdin = stdin_state.child_stdin.lock().await;
                    let mut data = t.to_string();
                    if !data.ends_with('\n') { data.push('\n'); }
                    if let Err(e) = stdin.write_all(data.as_bytes()).await { 
                        error!(?e, "failed to write to codex stdin");
                        break;
                    }
                }
                Message::Binary(b) => {
                    let mut stdin = stdin_state.child_stdin.lock().await;
                    if let Err(e) = stdin.write_all(&b).await { 
                        error!(?e, "failed to write binary to codex stdin");
                        break;
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
    // Resolve codex binary path
    let bin = match &opts.codex_bin {
        Some(p) => p.clone(),
        None => which::which("codex").unwrap_or_else(|_| PathBuf::from("codex")),
    };

    // Build args
    let mut args: Vec<String> = if let Some(args_str) = &opts.codex_args {
        shlex::split(args_str).ok_or_else(|| anyhow!("failed to parse CODEX_ARGS"))?
    } else {
        vec!["exec".into(), "--json".into()]
    };
    if !opts.extra.is_empty() { args.extend(opts.extra.clone()); }

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

    // Channel for broadcasting lines to all sockets
    let (tx, _rx) = broadcast::channel::<String>(1024);

    // Also monitor exit
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) => info!(?status, "codex exited"),
            Err(e) => error!(?e, "codex wait failed"),
        }
    });

    Ok((ChildWithIo { stdin, stdout, stderr }, tx))
}

async fn start_stream_forwarders(mut child: ChildWithIo, tx: &broadcast::Sender<String>) -> Result<()> {
    let stdout = child.stdout.take().context("missing stdout")?;
    let stderr = child.stderr.take().context("missing stderr")?;

    let tx_out = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("{}", line);
            let _ = tx_out.send(line);
        }
        info!("msg" = "stdout stream ended");
    });

    let tx_err = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("{}", line);
            let _ = tx_err.send(line);
        }
        info!("msg" = "stderr stream ended");
    });

    Ok(())
}
