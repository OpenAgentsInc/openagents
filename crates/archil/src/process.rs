//! The sandbox process API over the WebSocket connection.
//!
//! [`Sandbox::run`](crate::Sandbox::run) starts a process and
//! [`Sandbox::attach`](crate::Sandbox::attach) reattaches to one. Both return
//! a [`SandboxProcess`]: a handle backed by a background task that owns the
//! socket. Output arrives as binary frames (`0x01` stdout / `0x02` stderr, a
//! big-endian `u64` byte offset, then the payload) and can be consumed three
//! ways: collected into [`SandboxProcess::stdout`] / [`stderr`](SandboxProcess::stderr),
//! pushed to an [`OutputHandler`], or pulled with
//! [`SandboxProcess::next_output`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{Error, Result};
use crate::http::{Http, Req, enc};
use crate::retry::{MAX_RETRIES, RetryMode, retry_sleep};

/// Lifecycle state of a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

/// Which output stream a chunk came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStream {
    Stdout,
    Stderr,
}

/// A chunk of process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub stream: ProcessStream,
    /// Byte offset of `data` within its stream's combined cursor space.
    pub offset: u64,
    pub data: Bytes,
}

/// Callback invoked for every output chunk, in order.
pub type OutputHandler = Arc<dyn Fn(&ProcessOutput) + Send + Sync>;

/// Terminal mode for a started process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    /// `true` enables a PTY with the default size; `false` is sent verbatim.
    Enabled(bool),
    /// A PTY of the given size; unset dimensions default to 80x24.
    Size {
        cols: Option<u16>,
        rows: Option<u16>,
    },
}

impl Terminal {
    pub fn size(cols: u16, rows: u16) -> Self {
        Self::Size {
            cols: Some(cols),
            rows: Some(rows),
        }
    }
    fn wire(self) -> serde_json::Value {
        match self {
            Self::Enabled(b) => serde_json::Value::Bool(b),
            Self::Size { cols, rows } => {
                serde_json::json!({ "cols": cols.unwrap_or(80), "rows": rows.unwrap_or(24) })
            }
        }
    }
}

/// Options for [`Sandbox::run`](crate::Sandbox::run) and
/// [`Sandbox::exec`](crate::Sandbox::exec).
#[derive(Clone)]
pub struct ProcessStartOptions {
    /// Absolute working directory inside the sandbox.
    pub cwd: Option<String>,
    /// Enables a PTY. PTY output is merged into stdout and stderr stays empty.
    pub terminal: Option<Terminal>,
    pub env: BTreeMap<String, String>,
    pub timeout_seconds: Option<u64>,
    /// Called for every output chunk.
    pub on_output: Option<OutputHandler>,
    /// Retain decoded stdout and stderr on the handle and result (default true).
    pub collect_output: bool,
    /// Also queue chunks for [`SandboxProcess::next_output`] (default false).
    pub stream_output: bool,
}

impl Default for ProcessStartOptions {
    fn default() -> Self {
        Self {
            cwd: None,
            terminal: None,
            env: BTreeMap::new(),
            timeout_seconds: None,
            on_output: None,
            collect_output: true,
            stream_output: false,
        }
    }
}

impl std::fmt::Debug for ProcessStartOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessStartOptions")
            .field("cwd", &self.cwd)
            .field("terminal", &self.terminal)
            .field("env", &self.env)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("on_output", &self.on_output.is_some())
            .field("collect_output", &self.collect_output)
            .field("stream_output", &self.stream_output)
            .finish()
    }
}

impl ProcessStartOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
    pub fn terminal(mut self, terminal: Terminal) -> Self {
        self.terminal = Some(terminal);
        self
    }
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
    pub fn timeout_seconds(mut self, secs: u64) -> Self {
        self.timeout_seconds = Some(secs);
        self
    }
    pub fn on_output(mut self, handler: impl Fn(&ProcessOutput) + Send + Sync + 'static) -> Self {
        self.on_output = Some(Arc::new(handler));
        self
    }
    pub fn collect_output(mut self, v: bool) -> Self {
        self.collect_output = v;
        self
    }
    pub fn stream_output(mut self, v: bool) -> Self {
        self.stream_output = v;
        self
    }
}

/// Options for [`Sandbox::attach`](crate::Sandbox::attach).
#[derive(Clone)]
pub struct ProcessConnectOptions {
    /// Resume output from this cursor (from [`SandboxProcess::cursor`]).
    pub offset: u64,
    pub on_output: Option<OutputHandler>,
    pub collect_output: bool,
    pub stream_output: bool,
}

impl Default for ProcessConnectOptions {
    fn default() -> Self {
        Self {
            offset: 0,
            on_output: None,
            collect_output: true,
            stream_output: false,
        }
    }
}

impl std::fmt::Debug for ProcessConnectOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessConnectOptions")
            .field("offset", &self.offset)
            .field("on_output", &self.on_output.is_some())
            .field("collect_output", &self.collect_output)
            .field("stream_output", &self.stream_output)
            .finish()
    }
}

impl ProcessConnectOptions {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }
    pub fn on_output(mut self, handler: impl Fn(&ProcessOutput) + Send + Sync + 'static) -> Self {
        self.on_output = Some(Arc::new(handler));
        self
    }
    pub fn collect_output(mut self, v: bool) -> Self {
        self.collect_output = v;
        self
    }
    pub fn stream_output(mut self, v: bool) -> Self {
        self.stream_output = v;
        self
    }
}

/// Terminal state of a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessResult {
    /// Never [`ProcessStatus::Running`].
    pub status: ProcessStatus,
    pub exit_code: Option<i64>,
    pub exit_reason: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

// ------------------------------------------------------------------ wire

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Started {
        process_id: String,
    },
    Attached {
        process_id: String,
    },
    Exit {
        status: ProcessStatus,
        cursor: u64,
        exit_code: Option<i64>,
        exit_reason: Option<String>,
    },
    Killed,
    Resized,
    Error {
        error: String,
        message: String,
    },
    #[serde(other)]
    Other,
}

enum Cmd {
    Input(Bytes),
    Text(String),
    Close,
}

const STDIN_CHUNK_BYTES: usize = 1024 * 1024;

// ------------------------------------------------------------------ utf8

/// Streaming UTF-8 decoder: holds back an incomplete trailing sequence until
/// the next chunk and replaces invalid bytes with U+FFFD.
#[derive(Default)]
struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    fn decode(&mut self, input: &[u8]) -> String {
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(input);
        let mut out = String::new();
        let mut pos = 0;
        while pos < buf.len() {
            match std::str::from_utf8(&buf[pos..]) {
                Ok(s) => {
                    out.push_str(s);
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    out.push_str(
                        std::str::from_utf8(&buf[pos..pos + valid]).expect("valid prefix"),
                    );
                    match e.error_len() {
                        Some(n) => {
                            out.push('\u{FFFD}');
                            pos += valid + n;
                        }
                        None => {
                            self.pending = buf[pos + valid..].to_vec();
                            break;
                        }
                    }
                }
            }
        }
        out
    }

    fn flush(&mut self) -> String {
        if self.pending.is_empty() {
            String::new()
        } else {
            self.pending.clear();
            "\u{FFFD}".to_string()
        }
    }
}

// ------------------------------------------------------------------ shared state

struct State {
    id: String,
    cursor: u64,
    status: ProcessStatus,
    stdout: String,
    stderr: String,
    result: Option<ProcessResult>,
    closed: bool,
    connected: bool,
    connection_error: Option<String>,
    stdin_closed: bool,
    collect: bool,
    out_decoder: Utf8Decoder,
    err_decoder: Utf8Decoder,
}

struct Shared {
    state: Mutex<State>,
    changed: watch::Sender<u64>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
    fn notify(&self) {
        self.changed.send_modify(|v| *v += 1);
    }
}

impl State {
    fn flush_decoders(&mut self) {
        if !self.collect {
            return;
        }
        let out = self.out_decoder.flush();
        let err = self.err_decoder.flush();
        self.stdout.push_str(&out);
        self.stderr.push_str(&err);
    }

    fn handle_output(&mut self, bytes: &[u8]) -> Result<Option<ProcessOutput>> {
        if bytes.len() < 9 || (bytes[0] != 1 && bytes[0] != 2) {
            return Err(Error::process("Invalid process output frame"));
        }
        let offset = u64::from_be_bytes(bytes[1..9].try_into().expect("8 bytes"));
        let payload = &bytes[9..];
        let end = offset + payload.len() as u64;
        if end <= self.cursor {
            return Ok(None);
        }
        if offset > self.cursor {
            self.flush_decoders();
            self.cursor = offset;
        }
        let skip = self.cursor.saturating_sub(offset) as usize;
        let unread = &payload[skip..];
        let unread_offset = offset + skip as u64;
        self.cursor = end;
        let stream = if bytes[0] == 1 {
            ProcessStream::Stdout
        } else {
            ProcessStream::Stderr
        };
        if self.collect {
            match stream {
                ProcessStream::Stdout => {
                    let s = self.out_decoder.decode(unread);
                    self.stdout.push_str(&s);
                }
                ProcessStream::Stderr => {
                    let s = self.err_decoder.decode(unread);
                    self.stderr.push_str(&s);
                }
            }
        }
        Ok(Some(ProcessOutput {
            stream,
            offset: unread_offset,
            data: Bytes::copy_from_slice(unread),
        }))
    }

    fn finish(
        &mut self,
        status: ProcessStatus,
        cursor: u64,
        exit_code: Option<i64>,
        exit_reason: Option<String>,
    ) {
        if self.result.is_some() {
            return;
        }
        self.status = status;
        self.stdin_closed = true;
        self.flush_decoders();
        self.cursor = self.cursor.max(cursor);
        self.result = Some(ProcessResult {
            status,
            exit_code,
            exit_reason,
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
        });
    }
}

// ------------------------------------------------------------------ connector

/// Obtains short-lived WebSocket URLs for one sandbox.
#[derive(Clone)]
pub(crate) struct Connector {
    pub http: Arc<Http>,
    pub sandbox_id: String,
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

impl Connector {
    async fn url(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Conn {
            url: String,
        }
        let req = Req::new(
            reqwest::Method::POST,
            format!("/api/sandboxes/{}/connections", enc(&self.sandbox_id)),
            RetryMode::Transient,
        );
        Ok(self.http.call::<Conn>(req).await?.url)
    }

    /// Open a process socket, requesting a fresh URL per attempt and retrying
    /// with backoff.
    async fn open(&self) -> Result<Socket> {
        let mut attempt = 0;
        loop {
            let url = self.url().await?;
            match tokio_tungstenite::connect_async(url).await {
                Ok((socket, _)) => return Ok(socket),
                Err(cause) => {
                    if attempt >= MAX_RETRIES {
                        return Err(Error::Process(format!(
                            "Process connection failed after {} attempts: {cause}",
                            attempt + 1
                        )));
                    }
                }
            }
            retry_sleep(attempt).await;
            attempt += 1;
        }
    }

    /// One-shot control request (`kill` / `resize`) on a short-lived socket.
    async fn control(&self, request: serde_json::Value, expected: &str) -> Result<()> {
        let kind = request["type"].as_str().unwrap_or("control").to_string();
        let mut socket = self.open().await?;
        socket
            .send(Message::Text(request.to_string().into()))
            .await
            .map_err(|_| Error::process(format!("Process {kind} request failed")))?;
        let outcome = loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    break match serde_json::from_str::<Event>(&text) {
                        Ok(Event::Error { error, message }) => {
                            Err(Error::process(format!("{error}: {message}")))
                        }
                        Ok(Event::Killed) if expected == "killed" => Ok(()),
                        Ok(Event::Resized) if expected == "resized" => Ok(()),
                        Ok(_) => {
                            let got = serde_json::from_str::<serde_json::Value>(&text)
                                .ok()
                                .and_then(|v| v["type"].as_str().map(str::to_string))
                                .unwrap_or_default();
                            Err(Error::process(format!(
                                "Expected {expected}, received {got}"
                            )))
                        }
                        Err(e) => Err(Error::process(e.to_string())),
                    };
                }
                Some(Ok(Message::Close(_))) | None => {
                    break Err(Error::process(format!(
                        "Process {kind} connection closed before confirmation"
                    )));
                }
                Some(Err(_)) => {
                    break Err(Error::process(format!("Process {kind} request failed")));
                }
                Some(Ok(_)) => continue,
            }
        };
        let _ = socket.close(None).await;
        outcome
    }
}

// ------------------------------------------------------------------ SandboxProcess

/// Handle to a process running in a sandbox.
///
/// Dropping the handle closes the client connection without terminating the
/// process; use [`kill`](Self::kill) to terminate it.
pub struct SandboxProcess {
    shared: Arc<Shared>,
    cmd_tx: mpsc::UnboundedSender<Cmd>,
    output_rx: Option<mpsc::UnboundedReceiver<ProcessOutput>>,
    connector: Connector,
}

impl std::fmt::Debug for SandboxProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.shared.lock();
        f.debug_struct("SandboxProcess")
            .field("id", &s.id)
            .field("status", &s.status)
            .field("cursor", &s.cursor)
            .finish()
    }
}

pub(crate) enum ConnectRequest {
    Start {
        command: String,
        options: ProcessStartOptions,
    },
    Attach {
        process_id: String,
        options: ProcessConnectOptions,
    },
}

impl SandboxProcess {
    pub(crate) async fn connect(connector: Connector, request: ConnectRequest) -> Result<Self> {
        let (wire, id, cursor, handler, collect, stream, expected) = match request {
            ConnectRequest::Start { command, options } => {
                let mut wire = serde_json::json!({
                    "type": "start",
                    "command": command,
                    "env": options.env,
                });
                if let Some(cwd) = &options.cwd {
                    wire["cwd"] = cwd.clone().into();
                }
                if let Some(t) = options.terminal {
                    wire["terminal"] = t.wire();
                }
                if let Some(t) = options.timeout_seconds {
                    wire["timeout_seconds"] = t.into();
                }
                (
                    wire,
                    String::new(),
                    0,
                    options.on_output,
                    options.collect_output,
                    options.stream_output,
                    "started",
                )
            }
            ConnectRequest::Attach {
                process_id,
                options,
            } => (
                serde_json::json!({ "type": "attach", "process_id": process_id, "offset": options.offset }),
                process_id,
                options.offset,
                options.on_output,
                options.collect_output,
                options.stream_output,
                "attached",
            ),
        };

        let mut socket = connector.open().await?;
        let (changed, _) = watch::channel(0u64);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                id,
                cursor,
                status: ProcessStatus::Running,
                stdout: String::new(),
                stderr: String::new(),
                result: None,
                closed: false,
                connected: true,
                connection_error: None,
                stdin_closed: false,
                collect,
                out_decoder: Utf8Decoder::default(),
                err_decoder: Utf8Decoder::default(),
            }),
            changed,
        });
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (out_tx, output_rx) = if stream {
            let (tx, rx) = mpsc::unbounded_channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let (ready_tx, ready_rx) = oneshot::channel();

        if socket
            .send(Message::Text(wire.to_string().into()))
            .await
            .is_err()
        {
            return Err(Error::process("Process connection failed"));
        }
        tokio::spawn(socket_task(
            socket,
            shared.clone(),
            cmd_rx,
            ready_tx,
            expected,
            out_tx,
            handler,
        ));
        match ready_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(Error::process("Process connection closed before ready")),
        }
        Ok(Self {
            shared,
            cmd_tx,
            output_rx,
            connector,
        })
    }

    /// Process id assigned by the runtime.
    pub fn id(&self) -> String {
        self.shared.lock().id.clone()
    }

    /// Output cursor: pass it to [`Sandbox::attach`](crate::Sandbox::attach)
    /// to resume without losing or repeating output.
    pub fn cursor(&self) -> u64 {
        self.shared.lock().cursor
    }

    pub fn status(&self) -> ProcessStatus {
        self.shared.lock().status
    }

    /// Collected stdout so far (empty when `collect_output` is off).
    pub fn stdout(&self) -> String {
        self.shared.lock().stdout.clone()
    }

    /// Collected stderr so far (empty when `collect_output` is off).
    pub fn stderr(&self) -> String {
        self.shared.lock().stderr.clone()
    }

    /// Whether the socket is currently open.
    pub fn connected(&self) -> bool {
        let s = self.shared.lock();
        s.connected && !s.closed
    }

    /// The next output chunk, or `None` once the process has exited or the
    /// connection has ended and the queue is drained. Requires `stream_output(true)`; otherwise returns
    /// `None` immediately.
    pub async fn next_output(&mut self) -> Option<ProcessOutput> {
        self.output_rx.as_mut()?.recv().await
    }

    fn open_socket_check(&self) -> Result<()> {
        let s = self.shared.lock();
        if s.status != ProcessStatus::Running {
            return Err(Error::process("Process has exited"));
        }
        if !s.connected || s.closed {
            return Err(Error::process("Process is disconnected"));
        }
        Ok(())
    }

    /// Write to the process's stdin (split into 1 MiB frames).
    pub async fn send_input(&self, data: impl AsRef<[u8]>) -> Result<()> {
        if self.shared.lock().stdin_closed {
            return Err(Error::process("Process stdin is closed"));
        }
        self.open_socket_check()?;
        let data = data.as_ref();
        for chunk in data.chunks(STDIN_CHUNK_BYTES) {
            self.cmd_tx
                .send(Cmd::Input(Bytes::copy_from_slice(chunk)))
                .map_err(|_| Error::process("Process is disconnected"))?;
        }
        Ok(())
    }

    /// Resize the PTY. Only meaningful for processes started with a terminal.
    pub async fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let id = {
            let s = self.shared.lock();
            if s.status != ProcessStatus::Running {
                return Err(Error::process("Process has exited"));
            }
            s.id.clone()
        };
        self.connector
            .control(serde_json::json!({ "type": "resize", "process_id": id, "cols": cols, "rows": rows }), "resized")
            .await
    }

    /// Close stdin. Piped (non-PTY) processes only.
    pub async fn close_stdin(&self) -> Result<()> {
        if self.shared.lock().stdin_closed {
            return Ok(());
        }
        self.open_socket_check()?;
        self.cmd_tx
            .send(Cmd::Text(r#"{"type":"close_stdin"}"#.to_string()))
            .map_err(|_| Error::process("Process is disconnected"))?;
        self.shared.lock().stdin_closed = true;
        Ok(())
    }

    /// Close this client connection without terminating the process.
    pub async fn disconnect(&self) {
        {
            let mut s = self.shared.lock();
            if s.closed {
                return;
            }
            s.connected = false;
        }
        let _ = self.cmd_tx.send(Cmd::Close);
        let mut rx = self.shared.changed.subscribe();
        loop {
            if self.shared.lock().closed {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Wait for the process to exit and return its result.
    pub async fn wait(&self) -> Result<ProcessResult> {
        let mut rx = self.shared.changed.subscribe();
        loop {
            {
                let s = self.shared.lock();
                if let Some(r) = &s.result {
                    return Ok(r.clone());
                }
                if s.closed {
                    return Err(Error::process(
                        s.connection_error
                            .clone()
                            .unwrap_or_else(|| "Process connection closed before exit".into()),
                    ));
                }
            }
            if rx.changed().await.is_err() {
                return Err(Error::process("Process connection closed before exit"));
            }
        }
    }

    /// Terminate the process. A no-op once it has exited.
    pub async fn kill(&self) -> Result<()> {
        let id = {
            let s = self.shared.lock();
            if s.result.is_some() {
                return Ok(());
            }
            s.id.clone()
        };
        self.connector
            .control(
                serde_json::json!({ "type": "kill", "process_id": id }),
                "killed",
            )
            .await
    }
}

impl Drop for SandboxProcess {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(Cmd::Close);
    }
}

async fn socket_task(
    mut socket: Socket,
    shared: Arc<Shared>,
    mut cmd_rx: mpsc::UnboundedReceiver<Cmd>,
    ready_tx: oneshot::Sender<Result<()>>,
    expected: &'static str,
    mut out_tx: Option<mpsc::UnboundedSender<ProcessOutput>>,
    handler: Option<OutputHandler>,
) {
    let mut ready_tx = Some(ready_tx);
    let fail_ready = |ready: &mut Option<oneshot::Sender<Result<()>>>, msg: String| {
        if let Some(tx) = ready.take() {
            let _ = tx.send(Err(Error::Process(msg)));
        }
    };
    loop {
        tokio::select! {
            msg = socket.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match handle_event(&shared, &text, expected) {
                            Ok(Outcome::Ready) => {
                                if let Some(tx) = ready_tx.take() { let _ = tx.send(Ok(())); }
                            }
                            // No output follows an exit, so end the output queue now
                            // even though the socket stays open until disconnect.
                            Ok(Outcome::Exited) => { out_tx = None; }
                            Ok(Outcome::Nothing) => {}
                            Err(e) => {
                                let msg = e.to_string();
                                shared.lock().connection_error = Some(msg.clone());
                                fail_ready(&mut ready_tx, msg);
                                let _ = socket.close(None).await;
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        let result = shared.lock().handle_output(&bytes);
                        match result {
                            Ok(Some(output)) => {
                                if let Some(h) = &handler { h(&output); }
                                if let Some(tx) = &out_tx { let _ = tx.send(output); }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                let msg = e.to_string();
                                shared.lock().connection_error = Some(msg.clone());
                                fail_ready(&mut ready_tx, msg);
                                let _ = socket.close(None).await;
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => {
                        let msg = "Process connection failed".to_string();
                        shared.lock().connection_error = Some(msg.clone());
                        fail_ready(&mut ready_tx, msg);
                        break;
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(Cmd::Input(b)) => {
                        if socket.send(Message::Binary(b)).await.is_err() { break; }
                    }
                    Some(Cmd::Text(t)) => {
                        if socket.send(Message::Text(t.into())).await.is_err() { break; }
                    }
                    Some(Cmd::Close) | None => {
                        let _ = socket.close(None).await;
                        break;
                    }
                }
            }
        }
    }
    {
        let mut s = shared.lock();
        s.closed = true;
        s.connected = false;
    }
    fail_ready(
        &mut ready_tx,
        "Process connection closed before ready".to_string(),
    );
    shared.notify();
}

/// What a JSON event meant for the connection.
enum Outcome {
    Ready,
    Exited,
    Nothing,
}

/// Apply a JSON event.
fn handle_event(shared: &Shared, text: &str, expected: &str) -> Result<Outcome> {
    let event: Event = serde_json::from_str(text).map_err(|e| Error::process(e.to_string()))?;
    let mut s = shared.lock();
    match event {
        ref ev @ (Event::Started { .. } | Event::Attached { .. }) => {
            let (got, process_id) = match ev {
                Event::Started { process_id } => ("started", process_id.clone()),
                Event::Attached { process_id } => ("attached", process_id.clone()),
                _ => unreachable!("matched above"),
            };
            if got != expected {
                return Err(Error::process(format!(
                    "Expected {expected}, received {got}"
                )));
            }
            if !s.id.is_empty() && s.id != process_id {
                return Err(Error::process("Runtime returned a different process ID"));
            }
            s.id = process_id;
            drop(s);
            shared.notify();
            Ok(Outcome::Ready)
        }
        Event::Exit {
            status,
            cursor,
            exit_code,
            exit_reason,
        } => {
            s.finish(status, cursor, exit_code, exit_reason);
            drop(s);
            shared.notify();
            Ok(Outcome::Exited)
        }
        Event::Error { error, message } => Err(Error::process(format!("{error}: {message}"))),
        Event::Killed | Event::Resized | Event::Other => Ok(Outcome::Nothing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_holds_back_split_sequences() {
        let mut d = Utf8Decoder::default();
        let euro = "€".as_bytes();
        assert_eq!(d.decode(&euro[..2]), "");
        assert_eq!(d.decode(&euro[2..]), "€");
        assert_eq!(d.decode(b"a\xffb"), "a\u{FFFD}b");
        assert_eq!(d.decode(&euro[..1]), "");
        assert_eq!(d.flush(), "\u{FFFD}");
    }
}
