//! Claude Code, Codex, and ACP agents under a keeper (#11930).
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
//!   Codex and ACP request ids continue after the highest one the journal
//!   has.
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

/// The method of the marker that ends a [`Kept::flush`].
const FLUSHED: &str = "openagents/keeperFlushed";

/// Whether an incoming notification is one of the relay's markers, which a
/// session loop skips.
pub(crate) fn is_marker(method: &str) -> bool {
    method == MARK || method == FLUSHED
}

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
    /// Agent Client Protocol: JSON-RPC 2.0 like Codex, with no
    /// `initialized` notification, and a turn that ends with the response
    /// to its long-lived `session/prompt`.
    Acp,
    /// Pi's JSONL RPC: `{"type", "id"}` commands whose responses share one
    /// ordered channel with its events, and `extension_ui_request` dialogs.
    Pi,
    /// The Cursor SDK shim's JSONL: `{"op"}` lines in, `{"ev"}` frames out,
    /// and nothing the shim waits on an answer to.
    Cursor,
}

/// The prefix of Pi request ids; the number after it counts up.
const PI_ID: &str = "openagents-";

impl Wire {
    fn json_rpc(self) -> bool {
        matches!(self, Wire::Codex | Wire::Acp)
    }

    /// Adopting replays the driver's handshake against the journal.
    fn replays(self) -> bool {
        !matches!(self, Wire::Claude | Wire::Cursor)
    }

    /// A stdout line the agent waits on an answer to.
    fn request_key(self, line: &Value) -> Option<String> {
        match self {
            Wire::Claude => (line["type"] == "control_request")
                .then(|| line["request_id"].as_str().map(str::to_owned))
                .flatten(),
            Wire::Codex | Wire::Acp => {
                (line["method"].is_string() && is_id(&line["id"])).then(|| line["id"].to_string())
            }
            Wire::Cursor => None,
            Wire::Pi => (line["type"] == "extension_ui_request"
                && matches!(
                    line["method"].as_str(),
                    Some("select" | "confirm" | "input" | "editor")
                )
                && is_id(&line["id"]))
            .then(|| line["id"].to_string()),
        }
    }

    /// A stdin line that answers one.
    fn answer_key(self, line: &Value) -> Option<String> {
        match self {
            Wire::Claude => (line["type"] == "control_response")
                .then(|| line["response"]["request_id"].as_str().map(str::to_owned))
                .flatten(),
            Wire::Codex | Wire::Acp => (line.get("method").is_none()
                && is_id(&line["id"])
                && (line.get("result").is_some() || line.get("error").is_some()))
            .then(|| line["id"].to_string()),
            Wire::Pi => (line["type"] == "extension_ui_response" && is_id(&line["id"]))
                .then(|| line["id"].to_string()),
            Wire::Cursor => None,
        }
    }

    /// A request the driver sends (Codex, ACP, Pi), by its number.
    fn request_id(self, line: &Value) -> Option<i64> {
        match self {
            Wire::Claude | Wire::Cursor => None,
            Wire::Codex | Wire::Acp => line["method"]
                .is_string()
                .then(|| line["id"].as_i64())
                .flatten(),
            Wire::Pi => (line["type"].is_string() && line["type"] != "extension_ui_response")
                .then(|| pi_id(&line["id"]))
                .flatten(),
        }
    }

    /// A response to one of the driver's requests (Codex, ACP, Pi), by its id.
    fn response_id(self, line: &Value) -> Option<i64> {
        match self {
            Wire::Claude | Wire::Cursor => None,
            Wire::Codex | Wire::Acp => (line.get("method").is_none()
                && (line.get("result").is_some() || line.get("error").is_some()))
            .then(|| response_id(&line["id"]))
            .flatten(),
            Wire::Pi => (line["type"] == "response")
                .then(|| pi_id(&line["id"]))
                .flatten(),
        }
    }

    /// A stdout line the reader passes to the session loop: Codex and ACP
    /// messages that aren't responses, and every Pi frame.
    fn queued(self, line: &Value) -> bool {
        match self {
            Wire::Pi => line.is_object(),
            _ => {
                self.json_rpc()
                    && line.is_object()
                    && line.get("jsonrpc").is_none_or(|v| v == "2.0")
                    && line["method"].is_string()
            }
        }
    }
}

fn is_id(id: &Value) -> bool {
    id.is_number() || id.is_string()
}

fn pi_id(id: &Value) -> Option<i64> {
    id.as_str()?.strip_prefix(PI_ID)?.parse().ok()
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
    /// The last request of each method, with its id and params.
    requests: HashMap<String, (i64, Value)>,
    /// The first line the driver wrote.
    opening: Option<Value>,
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
            if scan.opening.is_none() {
                scan.opening = Some(value.clone());
            }
            if let Some(key) = wire.answer_key(&value) {
                scan.answered.insert(key);
            }
            if let Some(id) = wire.request_id(&value) {
                scan.max_request_id = scan.max_request_id.max(id);
                // Pi names its commands by `type`.
                if let Some(method) = value["method"].as_str().or(value["type"].as_str()) {
                    scan.requests
                        .insert(method.to_owned(), (id, value["params"].clone()));
                }
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
    /// Adopting: the highest request id in the journal's frames so far.
    journal_id: i64,
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
    /// Asks the relay to pass every message it holds now ([`Kept::flush`]).
    flush: Notify,
    /// Adopting: the last request of each method the journal has.
    journaled: HashMap<String, (i64, Value)>,
    /// Adopting: the first line the earlier engine wrote.
    opening: Option<Value>,
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
        let mut journaled = HashMap::new();
        let mut opening = None;
        if let Some((acked, scan)) = adopting {
            state.replay_through = (acked > 0).then_some(acked);
            state.answered = scan.answered;
            if wire.replays() {
                state.repeat_ids_through = Some(scan.max_request_id);
            }
            journaled = scan.requests;
            opening = scan.opening;
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
            flush: Notify::new(),
            journaled,
            opening,
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

    /// Pass the session loop's messages through a relay that tells this
    /// transport when the loop has handled each one.
    pub(crate) fn relay<T: Relayed>(
        self: &Arc<Self>,
        from: mpsc::Receiver<T>,
    ) -> mpsc::Receiver<T> {
        let (to, rx) = mpsc::channel(1);
        tokio::spawn(relay(self.clone(), from, to));
        rx
    }

    /// Adopting Codex or ACP: the handshake is replayed; new request ids
    /// follow the journal's. Returns the highest id the journal has.
    pub(crate) fn handshake_replayed(&self) -> Option<i64> {
        let through = self.lock().repeat_ids_through.take();
        self.written.notify_waiters();
        through
    }

    /// Wait until the driver's writes reached request id `id`: a JSON-RPC
    /// client writes from a task of its own.
    pub(crate) async fn written_through(&self, id: i64) {
        loop {
            let written = self.written.notified();
            if self.lock().written_id >= id || self.exit.borrow().is_some() {
                return;
            }
            written.await;
        }
    }

    /// Adopting: the first line the earlier engine wrote to the agent.
    pub(crate) fn opening(&self) -> Option<&Value> {
        self.opening.as_ref()
    }

    /// Adopting: the last `method` request an earlier engine sent, with its
    /// id and params.
    pub(crate) fn journaled(&self, method: &str) -> Option<(i64, Value)> {
        self.journaled.get(method).cloned()
    }

    /// Every message the reader had queued for the session loop before now,
    /// in order. The relay hands messages over one at a time, so a loop that
    /// drains its channel with `try_recv` (ACP, when a response ends a turn)
    /// takes these first. Markers are skipped.
    pub(crate) async fn flush<T: Relayed>(&self, incoming: &mut mpsc::Receiver<T>) -> VecDeque<T> {
        self.flush.notify_one();
        let mut queued = VecDeque::new();
        while let Some(message) = incoming.recv().await {
            match message.marker() {
                Some(FLUSHED) => break,
                Some(_) => {}
                None => queued.push_back(message),
            }
        }
        queued
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
                // Adopting Codex or ACP: what the agent wrote after a request
                // waits until the replayed handshake has sent that request,
                // so a reply finds it and the driver reads the rest in the
                // state the earlier engine read it in (an ACP update that
                // arrived while the driver was still setting up would be
                // dropped as replayed history).
                loop {
                    let written = kept.written.notified();
                    {
                        let state = kept.lock();
                        if state.repeat_ids_through.is_none()
                            || state.written_id >= state.journal_id
                        {
                            break;
                        }
                    }
                    written.await;
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
            FrameBody::Stdin { line } => {
                let mut state = kept.lock();
                if state.repeat_ids_through.is_some()
                    && let Some(id) = serde_json::from_str::<Value>(&line)
                        .ok()
                        .and_then(|v| kept.wire.request_id(&v))
                {
                    state.journal_id = state.journal_id.max(id);
                }
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

/// A message a relay passes to a session loop.
pub(crate) trait Relayed: Send + 'static {
    /// A marker with this method.
    fn mark(method: &'static str) -> Self;
    /// The method, when this is a marker.
    fn marker(&self) -> Option<&str>;
    /// The last message: the agent's output ended.
    fn ends(&self) -> bool;
}

impl Relayed for Incoming {
    fn mark(method: &'static str) -> Self {
        Incoming::Notification {
            method: method.into(),
            params: Value::Null,
        }
    }

    fn marker(&self) -> Option<&str> {
        match self {
            Incoming::Notification { method, .. } if is_marker(method) => Some(method),
            _ => None,
        }
    }

    fn ends(&self) -> bool {
        matches!(self, Incoming::Eof)
    }
}

/// Pi's frames: a read error is the last one.
impl Relayed for Result<Value, HarnessError> {
    fn mark(method: &'static str) -> Self {
        Ok(serde_json::json!({ "type": method }))
    }

    fn marker(&self) -> Option<&str> {
        self.as_ref()
            .ok()
            .and_then(|frame| frame["type"].as_str())
            .filter(|kind| is_marker(kind))
    }

    fn ends(&self) -> bool {
        self.is_err()
    }
}

/// The session loop's messages, each followed by a marker. The loop takes
/// the marker only after it has handled the message before it.
async fn relay<T: Relayed>(kept: Arc<Kept>, mut from: mpsc::Receiver<T>, to: mpsc::Sender<T>) {
    let mut passed = 0u64;
    loop {
        let message = tokio::select! {
            message = from.recv() => message,
            _ = kept.poke.notified() => {
                if !mark(&to, MARK).await {
                    return;
                }
                kept.marker_taken(None);
                continue;
            }
            _ = kept.flush.notified() => {
                for _ in 0..from.len() {
                    let Ok(message) = from.try_recv() else { break };
                    if !pass(&kept, &to, message, &mut passed).await {
                        return;
                    }
                }
                if !mark(&to, FLUSHED).await {
                    return;
                }
                continue;
            }
        };
        let Some(message) = message else { return };
        if !pass(&kept, &to, message, &mut passed).await {
            return;
        }
    }
}

/// Pass one message and its marker; false once the relay is done.
async fn pass<T: Relayed>(kept: &Kept, to: &mpsc::Sender<T>, message: T, passed: &mut u64) -> bool {
    let end = message.ends();
    if to.send(message).await.is_err() || end {
        return false;
    }
    *passed += 1;
    if !mark(to, MARK).await {
        return false;
    }
    kept.marker_taken(Some(*passed));
    true
}

/// Send a marker and wait until the loop has taken it.
async fn mark<T: Relayed>(to: &mpsc::Sender<T>, method: &'static str) -> bool {
    to.send(T::mark(method)).await.is_ok() && to.reserve().await.is_ok()
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

        let ask = json!({"jsonrpc":"2.0","id":3,"method":"session/request_permission"});
        let answer = json!({"jsonrpc":"2.0","id":3,"result":{"outcome":{}}});
        let prompt = json!({"jsonrpc":"2.0","id":5,"method":"session/prompt","params":{}});
        let settled = json!({"jsonrpc":"2.0","id":5,"result":{"stopReason":"end_turn"}});
        assert_eq!(Wire::Acp.request_key(&ask), Wire::Acp.answer_key(&answer));
        assert_eq!(Wire::Acp.request_id(&prompt), Some(5));
        assert_eq!(Wire::Acp.response_id(&settled), Some(5));
        assert!(Wire::Acp.queued(&ask));
        assert!(!Wire::Acp.queued(&settled));

        let ask = json!({"type":"extension_ui_request","id":"u1","method":"select"});
        let note = json!({"type":"extension_ui_request","id":"u2","method":"notify"});
        let answer = json!({"type":"extension_ui_response","id":"u1","value":"A"});
        let prompt = json!({"type":"prompt","id":"openagents-4","message":"hi"});
        let accepted = json!({"type":"response","id":"openagents-4","success":true});
        assert_eq!(Wire::Pi.request_key(&ask), Wire::Pi.answer_key(&answer));
        assert_eq!(Wire::Pi.request_key(&note), None);
        assert_eq!(Wire::Pi.request_id(&prompt), Some(4));
        assert_eq!(Wire::Pi.request_id(&answer), None);
        assert_eq!(Wire::Pi.response_id(&accepted), Some(4));
        assert!(Wire::Pi.queued(&accepted));
    }
}
