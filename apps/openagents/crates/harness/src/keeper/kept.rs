//! Claude Code and Codex under a keeper (#11930).
//!
//! The drivers keep speaking their own protocols over what looks like the
//! agent's stdio: [`launch`] starts the agent under a keeper, and [`attach`]
//! picks up one an earlier engine started. Both return a
//! [`crate::process::RuntimeChild`] whose pipes go through the keeper.
//!
//! - **Acknowledgement.** A frame is acknowledged once the driver has read
//!   the whole line, handled it (for Codex, once its session loop took the
//!   message, see [`relay`]), and handed every event it produced to the
//!   engine. An engine that dies has handled every frame it acknowledged.
//! - **Questions.** A request the agent waits on (a Claude `control_request`,
//!   a Codex server request) is held until the driver writes its answer, so
//!   a later engine asks it again.
//! - **Adoption.** A new engine replays the whole journal through a fresh
//!   driver, so the driver rebuilds its state the normal way. Events from
//!   frames the earlier engine acknowledged are dropped, requests that were
//!   already answered are skipped, and writes that repeat ones the journal
//!   has (Codex's handshake, already-written answers) are not sent again.
//!   Codex request ids continue after the highest one the journal has.
//! - **Leaving.** When the engine quits on purpose ([`super::Detach`]), the
//!   driver's stop and its writes don't reach the agent, so it keeps running.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::pin::Pin;
use std::process::ExitStatus;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{Notify, mpsc, watch};

use openagents_proto::AgentEvent;

use super::{Detach, Frame, FrameBody, KeeperScope, Link, LinkSender, Spawn};
use crate::HarnessError;
use crate::jsonrpc::Incoming;
use crate::process::{Command, RuntimeChild};

/// Set to `0` (or `off`, `false`, `no`) to start Claude Code and Codex
/// without a keeper, as before keepers existed.
pub const ENV: &str = "OPENAGENTS_KEEPERS";

/// The method of the marker the Codex relay passes to the session loop.
const MARK: &str = "openagents/keeperMark";

/// Whether the environment allows keepers.
pub fn allowed_by_env() -> bool {
    allows(std::env::var(ENV).ok().as_deref())
}

fn allows(value: Option<&str>) -> bool {
    !matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("0" | "off" | "false" | "no")
    )
}

/// Which protocol the agent speaks, for the few things the transport reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wire {
    Claude,
    Codex,
}

impl Wire {
    /// A stdout line the agent waits on an answer to.
    fn request_key(self, line: &Value) -> Option<String> {
        match self {
            Wire::Claude => (line["type"] == "control_request")
                .then(|| line["request_id"].as_str().map(str::to_owned))
                .flatten(),
            Wire::Codex => {
                (line["method"].is_string() && is_id(&line["id"])).then(|| line["id"].to_string())
            }
        }
    }

    /// A stdin line that answers one.
    fn answer_key(self, line: &Value) -> Option<String> {
        match self {
            Wire::Claude => (line["type"] == "control_response")
                .then(|| line["response"]["request_id"].as_str().map(str::to_owned))
                .flatten(),
            Wire::Codex => (line.get("method").is_none()
                && is_id(&line["id"])
                && (line.get("result").is_some() || line.get("error").is_some()))
            .then(|| line["id"].to_string()),
        }
    }

    /// A request the driver sends (Codex), by its numeric id.
    fn request_id(self, line: &Value) -> Option<i64> {
        match self {
            Wire::Claude => None,
            Wire::Codex => line["method"]
                .is_string()
                .then(|| line["id"].as_i64())
                .flatten(),
        }
    }

    /// A response to one of the driver's requests (Codex), by its id.
    fn response_id(self, line: &Value) -> Option<i64> {
        match self {
            Wire::Claude => None,
            Wire::Codex => (line.get("method").is_none()
                && (line.get("result").is_some() || line.get("error").is_some()))
            .then(|| response_id(&line["id"]))
            .flatten(),
        }
    }

    /// A stdout line the Codex reader passes to the session loop.
    fn queued(self, line: &Value) -> bool {
        self == Wire::Codex
            && line.is_object()
            && line.get("jsonrpc").is_none_or(|v| v == "2.0")
            && line["method"].is_string()
    }
}

fn is_id(id: &Value) -> bool {
    id.is_number() || id.is_string()
}

fn response_id(id: &Value) -> Option<i64> {
    id.as_i64()
        .or_else(|| id.as_str().and_then(|s| s.parse().ok()))
}

/// The agent's runtime under a keeper: the run behind `adopt`, or a new
/// keeper for `command` when `enabled` and the engine lent a scope. `None`
/// means the caller starts the agent directly.
pub(crate) async fn child(
    command: &Command,
    wire: Wire,
    enabled: bool,
    idle: bool,
    adopt: Option<Link>,
) -> Result<Option<RuntimeChild>, HarnessError> {
    if let Some(link) = adopt {
        return attach(link, wire).await.map(Some).map_err(HarnessError::Io);
    }
    if !enabled {
        return Ok(None);
    }
    match launch(command, wire, idle).await {
        Ok(child) => Ok(child),
        Err(error) => {
            // Never lose the run over its keeper: start the agent directly.
            tracing::warn!(target: "openagents_harness::keeper", %error, "could not start the agent keeper");
            Ok(None)
        }
    }
}

/// Start the command under a keeper when the engine lent a scope. `None`
/// means the caller starts it directly.
async fn launch(command: &Command, wire: Wire, idle: bool) -> io::Result<Option<RuntimeChild>> {
    let Some(scope) = super::current() else {
        return Ok(None);
    };
    let KeeperScope {
        root,
        keeper_exe,
        keeper_args,
        mut meta,
        orphan_after,
        detach,
    } = scope;
    let std = command.as_std();
    if let Some(meta) = meta.as_object_mut() {
        meta.insert("keeperDriver".into(), serde_json::json!({ "idle": idle }));
    }
    let link = super::client::spawn(Spawn {
        keeper_exe,
        keeper_args,
        root,
        program: std.get_program().into(),
        args: std.get_args().map(Into::into).collect(),
        cwd: std.get_current_dir().map(Into::into),
        env: std
            .get_envs()
            .map(|(key, value)| (key.into(), value.map(Into::into)))
            .collect(),
        meta,
        orphan_after,
    })
    .await?;
    Ok(Some(Kept::start(link, wire, detach, None)))
}

/// Whether the run an earlier engine started under this keeper was idle,
/// a session waiting for its first prompt.
pub(crate) fn started_idle(link: &Link) -> bool {
    link.meta["keeperDriver"]["idle"].as_bool().unwrap_or(false)
}

/// Pick up the run behind `link`: reconnect to replay the whole journal.
async fn attach(link: Link, wire: Wire) -> io::Result<RuntimeChild> {
    let detach = super::current()
        .map(|scope| scope.detach)
        .unwrap_or_default();
    let acked = link.acked;
    let dir = link.dir.clone();
    let replay = super::client::connect(&dir, Some(0)).await?;
    // The keeper now streams to the new connection.
    drop(link);
    let scan = Scan::read(&replay.journal_path(), wire);
    Ok(Kept::start(replay, wire, detach, Some((acked, scan))))
}

/// What the journal shows the earlier engines wrote.
#[derive(Default)]
struct Scan {
    answered: HashSet<String>,
    max_request_id: i64,
}

impl Scan {
    fn read(path: &std::path::Path, wire: Wire) -> Self {
        let mut scan = Scan::default();
        let Ok(text) = std::fs::read_to_string(path) else {
            return scan;
        };
        for line in text.lines() {
            let Ok(Frame {
                body: FrameBody::Stdin { line },
                ..
            }) = serde_json::from_str(line)
            else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(key) = wire.answer_key(&value) {
                scan.answered.insert(key);
            }
            if let Some(id) = wire.request_id(&value) {
                scan.max_request_id = scan.max_request_id.max(id);
            }
        }
        scan
    }
}

/// When a frame counts as handled.
#[derive(Debug, Clone, Copy)]
enum Gate {
    /// Once the driver has read it.
    Read,
    /// Once the Codex session loop finished the `n`th message passed to it.
    Message(u64),
    /// Once the Codex session loop takes a marker after the frame was read:
    /// a response's awaiting code has then run.
    Marker,
}

struct Entry {
    seq: u64,
    gate: Gate,
    /// Read by the driver; for [`Gate::Marker`], the marker count to pass.
    read: Option<u64>,
}

#[derive(Default)]
struct State {
    entries: VecDeque<Entry>,
    messages_passed: u64,
    messages_finished: u64,
    markers_taken: u64,
    /// Events handed to the engine, or dropped as already handled.
    handed: u64,
    /// Acknowledge frame `.0` once `.1` events are handed over.
    pending_acks: VecDeque<(u64, u64)>,
    /// Adopting: drop every event until this frame is handled.
    replay_through: Option<u64>,
    /// Then drop events up to this count.
    drop_through: u64,
    /// Requests the agent waits on, by key, and their frames.
    holds: HashMap<String, u64>,
    answered: HashSet<String>,
    events: Option<mpsc::WeakSender<Result<AgentEvent, HarnessError>>>,
    /// Adopting Codex: request ids up to this repeat the journal's.
    repeat_ids_through: Option<i64>,
    /// The highest request id the driver has written (or repeated).
    written_id: i64,
    /// Adopting Codex: the handshake's `initialized` was already sent.
    initialized_repeated: bool,
}

/// The engine's end of an agent under a keeper.
pub(crate) struct Kept {
    wire: Wire,
    sender: LinkSender,
    detach: Detach,
    pid: u32,
    state: Mutex<State>,
    exit: watch::Receiver<Option<ExitStatus>>,
    /// Wakes the frame pump when the driver writes a request.
    written: Notify,
    /// Asks the Codex relay for a marker.
    poke: Notify,
}

impl Kept {
    fn start(
        link: Link,
        wire: Wire,
        detach: Detach,
        adopting: Option<(u64, Scan)>,
    ) -> RuntimeChild {
        let pid = link.pid;
        let (sender, frames) = link.into_parts();
        let mut state = State::default();
        if let Some((acked, scan)) = adopting {
            state.replay_through = (acked > 0).then_some(acked);
            state.answered = scan.answered;
            if wire == Wire::Codex {
                state.repeat_ids_through = Some(scan.max_request_id);
            }
        }
        let (exit_tx, exit) = watch::channel(None);
        let kept = Arc::new(Kept {
            wire,
            sender,
            detach,
            pid,
            state: Mutex::new(state),
            exit,
            written: Notify::new(),
            poke: Notify::new(),
        });
        let (out_tx, out_rx) = mpsc::unbounded_channel();
        let (err_tx, err_rx) = mpsc::unbounded_channel();
        tokio::spawn(pump(kept.clone(), frames, out_tx, err_tx, exit_tx));
        RuntimeChild::kept(
            kept.clone(),
            Box::new(Input {
                kept: kept.clone(),
                buffer: Vec::new(),
            }),
            Box::new(Output {
                kept: Some(kept.clone()),
                rx: out_rx,
                current: None,
            }),
            Box::new(Output {
                kept: None,
                rx: err_rx,
                current: None,
            }),
        )
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn id(&self) -> Option<u32> {
        (self.exit.borrow().is_none() && self.pid != 0).then_some(self.pid)
    }

    pub(crate) fn try_wait(&self) -> Option<ExitStatus> {
        *self.exit.borrow()
    }

    pub(crate) async fn wait(&self) -> io::Result<ExitStatus> {
        let mut exit = self.exit.clone();
        loop {
            if let Some(status) = *exit.borrow() {
                return Ok(status);
            }
            if exit.changed().await.is_err() {
                return Err(io::Error::other("lost the connection to the agent"));
            }
        }
    }

    /// The agent's process group, unless the engine is leaving it running.
    pub(crate) fn signal_target(&self) -> Option<i32> {
        (!self.detach.raised())
            .then(|| self.id())
            .flatten()
            .map(|pid| -(pid as i32))
    }

    /// The run is over here: stop the agent and end its keeper, unless the
    /// engine is leaving it running for the next engine.
    pub(crate) fn shutdown(&self) {
        if !self.detach.raised() {
            let _ = self.sender.finish();
        }
    }

    /// Count the driver's events from `tx`; the stream from `rx` hands them over.
    pub(crate) fn events(
        self: &Arc<Self>,
        tx: &mpsc::Sender<Result<AgentEvent, HarnessError>>,
        rx: mpsc::Receiver<Result<AgentEvent, HarnessError>>,
    ) -> BoxStream<'static, Result<AgentEvent, HarnessError>> {
        self.lock().events = Some(tx.downgrade());
        let kept = self.clone();
        futures::stream::unfold((rx, kept), |(mut rx, kept)| async move {
            loop {
                let event = rx.recv().await?;
                let skip = {
                    let mut state = kept.lock();
                    state.handed += 1;
                    let skip = state.replay_through.is_some() || state.handed <= state.drop_through;
                    let mut ack = None;
                    while let Some(&(seq, need)) = state.pending_acks.front() {
                        if need > state.handed {
                            break;
                        }
                        ack = Some(seq);
                        state.pending_acks.pop_front();
                    }
                    if let Some(seq) = ack {
                        let _ = kept.sender.ack(seq);
                    }
                    skip
                };
                if !skip {
                    return Some((event, (rx, kept)));
                }
            }
        })
        .boxed()
    }

    /// Pass the Codex session loop's messages through a relay that tells
    /// this transport when the loop has handled each one.
    pub(crate) fn relay(
        self: &Arc<Self>,
        from: mpsc::Receiver<Incoming>,
    ) -> mpsc::Receiver<Incoming> {
        let (to, rx) = mpsc::channel(1);
        tokio::spawn(relay(self.clone(), from, to));
        rx
    }

    /// Adopting Codex: the handshake is replayed; new request ids follow the
    /// journal's. Returns the highest id the journal has.
    pub(crate) fn handshake_replayed(&self) -> Option<i64> {
        let through = self.lock().repeat_ids_through.take();
        self.written.notify_waiters();
        through
    }

    /// Frames handled in order, from the front: acknowledge the last one
    /// once its events are handed over.
    fn advance(&self, state: &mut State) {
        let mut done = None;
        while let Some(entry) = state.entries.front() {
            let handled = match (entry.gate, entry.read) {
                (_, None) => false,
                (Gate::Read, Some(_)) => true,
                (Gate::Message(n), Some(_)) => state.messages_finished >= n,
                (Gate::Marker, Some(after)) => state.markers_taken > after,
            };
            if !handled {
                break;
            }
            done = Some(entry.seq);
            state.entries.pop_front();
        }
        let Some(seq) = done else { return };
        let queued = state
            .events
            .as_ref()
            .and_then(mpsc::WeakSender::upgrade)
            .map_or(0, |tx| tx.max_capacity() - tx.capacity()) as u64;
        let need = state.handed + queued;
        if state.replay_through.is_some_and(|through| seq >= through) {
            state.replay_through = None;
            state.drop_through = need;
        }
        if need <= state.handed {
            let _ = self.sender.ack(seq);
        } else {
            state.pending_acks.push_back((seq, need));
        }
    }

    fn read(&self, seq: u64) {
        let mut state = self.lock();
        let markers = state.markers_taken;
        let mut poke = false;
        if let Some(entry) = state.entries.iter_mut().find(|e| e.seq == seq) {
            entry.read = Some(markers);
            poke = matches!(entry.gate, Gate::Marker);
        }
        self.advance(&mut state);
        drop(state);
        if poke {
            self.poke.notify_one();
        }
    }

    fn marker_taken(&self, finished: Option<u64>) {
        let mut state = self.lock();
        state.markers_taken += 1;
        if let Some(n) = finished {
            state.messages_finished = state.messages_finished.max(n);
        }
        self.advance(&mut state);
    }

    /// One line the driver wrote to the agent's stdin.
    fn write(&self, line: String) -> io::Result<()> {
        if self.detach.raised() {
            return Ok(()); // the engine is leaving; the agent keeps running
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            return self.sender.stdin(line);
        };
        let mut state = self.lock();
        if let Some(key) = self.wire.answer_key(&value) {
            let held = state.holds.remove(&key);
            if state.answered.insert(key) {
                self.sender.stdin(line)?;
            }
            if let Some(seq) = held {
                self.sender.release(seq)?;
            }
            return Ok(());
        }
        if let Some(id) = self.wire.request_id(&value) {
            state.written_id = state.written_id.max(id);
            let repeat = state
                .repeat_ids_through
                .is_some_and(|through| id <= through);
            drop(state);
            self.written.notify_waiters();
            return if repeat {
                Ok(())
            } else {
                self.sender.stdin(line)
            };
        }
        if self.wire == Wire::Codex
            && value["method"] == "initialized"
            && state.repeat_ids_through.is_some()
            && !std::mem::replace(&mut state.initialized_repeated, true)
        {
            return Ok(());
        }
        drop(state);
        self.sender.stdin(line)
    }
}

/// Route the keeper's frames to the driver's stdout and stderr.
async fn pump(
    kept: Arc<Kept>,
    mut frames: mpsc::UnboundedReceiver<Frame>,
    out: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    err: mpsc::UnboundedSender<(u64, Vec<u8>)>,
    exit: watch::Sender<Option<ExitStatus>>,
) {
    while let Some(frame) = frames.recv().await {
        let seq = frame.seq;
        match frame.body {
            FrameBody::Stdout { line } => {
                let value = serde_json::from_str::<Value>(line.trim()).ok();
                // Adopting Codex: a response waits until the replayed
                // handshake has sent its request, so the reply finds it.
                if let Some(id) = value.as_ref().and_then(|v| kept.wire.response_id(v)) {
                    loop {
                        let written = kept.written.notified();
                        {
                            let state = kept.lock();
                            if state
                                .repeat_ids_through
                                .is_none_or(|through| id > through || state.written_id >= id)
                            {
                                break;
                            }
                        }
                        written.await;
                    }
                }
                let request = value.as_ref().and_then(|v| kept.wire.request_key(v));
                let gate = match &value {
                    Some(v) if kept.wire.queued(v) => None,
                    Some(v) if kept.wire.response_id(v).is_some() => Some(Gate::Marker),
                    _ => Some(Gate::Read),
                };
                let mut state = kept.lock();
                if let Some(key) = &request
                    && state.answered.contains(key)
                {
                    // Answered by an earlier engine: nothing to ask again.
                    let _ = kept.sender.release(seq);
                    state.entries.push_back(Entry {
                        seq,
                        gate: Gate::Read,
                        read: Some(0),
                    });
                    kept.advance(&mut state);
                    continue;
                }
                if let Some(key) = request {
                    let _ = kept.sender.hold(seq);
                    state.holds.insert(key, seq);
                }
                let gate = gate.unwrap_or_else(|| {
                    state.messages_passed += 1;
                    Gate::Message(state.messages_passed)
                });
                state.entries.push_back(Entry {
                    seq,
                    gate,
                    read: None,
                });
                drop(state);
                let mut bytes = line.into_bytes();
                bytes.push(b'\n');
                if out.send((seq, bytes)).is_err() {
                    return;
                }
            }
            FrameBody::Stderr { line } => {
                {
                    let mut state = kept.lock();
                    state.entries.push_back(Entry {
                        seq,
                        gate: Gate::Read,
                        read: Some(0),
                    });
                    kept.advance(&mut state);
                }
                let mut bytes = line.into_bytes();
                bytes.push(b'\n');
                let _ = err.send((seq, bytes));
            }
            FrameBody::Stdin { .. } => {
                let mut state = kept.lock();
                state.entries.push_back(Entry {
                    seq,
                    gate: Gate::Read,
                    read: Some(0),
                });
                kept.advance(&mut state);
            }
            FrameBody::Exit { code, signal } => {
                use std::os::unix::process::ExitStatusExt;
                let raw = match (code, signal) {
                    (_, Some(signal)) => signal,
                    (Some(code), None) => (code & 0xff) << 8,
                    (None, None) => 1 << 8,
                };
                let _ = exit.send(Some(ExitStatus::from_raw(raw)));
                let mut state = kept.lock();
                state.entries.push_back(Entry {
                    seq,
                    gate: Gate::Read,
                    read: Some(0),
                });
                kept.advance(&mut state);
                break;
            }
        }
    }
    // Closing both channels ends the driver's reads.
}

/// The Codex session loop's messages, each followed by a marker. The loop
/// takes the marker only after it has handled the message before it.
async fn relay(kept: Arc<Kept>, mut from: mpsc::Receiver<Incoming>, to: mpsc::Sender<Incoming>) {
    let mut passed = 0u64;
    loop {
        let message = tokio::select! {
            message = from.recv() => message,
            _ = kept.poke.notified() => {
                if !mark(&to).await {
                    return;
                }
                kept.marker_taken(None);
                continue;
            }
        };
        let Some(message) = message else { return };
        let end = matches!(message, Incoming::Eof);
        if to.send(message).await.is_err() || end {
            return;
        }
        passed += 1;
        if !mark(&to).await {
            return;
        }
        kept.marker_taken(Some(passed));
    }
}

/// Send a marker and wait until the loop has taken it.
async fn mark(to: &mpsc::Sender<Incoming>) -> bool {
    let marker = Incoming::Notification {
        method: MARK.into(),
        params: Value::Null,
    };
    to.send(marker).await.is_ok() && to.reserve().await.is_ok()
}

/// The driver's stdin: each complete line goes to the keeper.
struct Input {
    kept: Arc<Kept>,
    buffer: Vec<u8>,
}

impl AsyncWrite for Input {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.buffer.extend_from_slice(buf);
        while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line[..end]).into_owned();
            if let Err(error) = self.kept.write(line) {
                return Poll::Ready(Err(error));
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if !self.kept.detach.raised() {
            let _ = self.kept.sender.close_stdin();
        }
        Poll::Ready(Ok(()))
    }
}

/// The driver's stdout or stderr, one frame's line at a time. For stdout,
/// asking for more after a whole line means the driver handled it.
struct Output {
    kept: Option<Arc<Kept>>,
    rx: mpsc::UnboundedReceiver<(u64, Vec<u8>)>,
    current: Option<(u64, Vec<u8>, usize)>,
}

impl AsyncRead for Output {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if let Some((seq, bytes, at)) = &mut self.current {
                if *at < bytes.len() {
                    let n = buf.remaining().min(bytes.len() - *at);
                    buf.put_slice(&bytes[*at..*at + n]);
                    *at += n;
                    return Poll::Ready(Ok(()));
                }
                let seq = *seq;
                self.current = None;
                if let Some(kept) = &self.kept {
                    kept.read(seq);
                }
            }
            match self.rx.poll_recv(cx) {
                Poll::Ready(Some(next)) => self.current = Some((next.0, next.1, 0)),
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_environment_turns_keepers_off() {
        assert!(allows(None));
        assert!(allows(Some("1")));
        for off in ["0", "off", "FALSE", " no "] {
            assert!(!allows(Some(off)), "{off}");
        }
    }

    #[test]
    fn requests_and_answers_pair_by_key() {
        let ask = json!({"type":"control_request","request_id":"r1","request":{}});
        let answer = json!({"type":"control_response","response":{"request_id":"r1"}});
        assert_eq!(Wire::Claude.request_key(&ask).as_deref(), Some("r1"));
        assert_eq!(Wire::Claude.answer_key(&answer).as_deref(), Some("r1"));
        assert_eq!(Wire::Claude.answer_key(&ask), None);

        let ask = json!({"jsonrpc":"2.0","id":0,"method":"item/commandExecution/requestApproval"});
        let answer = json!({"jsonrpc":"2.0","id":0,"result":{"decision":"accept"}});
        let ours = json!({"jsonrpc":"2.0","id":4,"method":"turn/start","params":{}});
        assert_eq!(
            Wire::Codex.request_key(&ask),
            Wire::Codex.answer_key(&answer)
        );
        assert_eq!(Wire::Codex.request_id(&ours), Some(4));
        assert_eq!(Wire::Codex.response_id(&answer), Some(0));
        assert!(Wire::Codex.queued(&ask));
        assert!(!Wire::Codex.queued(&answer));
    }
}
