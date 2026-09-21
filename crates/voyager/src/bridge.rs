//! The seam to the Minecraft bot.
//!
//! `mc-bridge` is a child process that speaks one JSON object per line on
//! stdin and stdout, the same shape the lev helper speaks — a shape this
//! workspace settled on after auditing a hand-parsed socket. Two kinds of
//! line come back: a **response**, `{"id": N, "ok": ..., ...}`, which
//! answers the request that carried `id`, and an **event**,
//! `{"event": "chat", ...}`, which the bot reports on its own — chat it
//! sees, a spawn, a death, feedback while it works. One channel keeps
//! them ordered, so an episode reads them the way the world produced
//! them.
//!
//! A child process can hang, flood, answer with the wrong id, or exit.
//! [`Bridge`] therefore owns a reader thread for stdout, a drainer for
//! stderr that keeps a tail for diagnostics, caps one line at
//! [`MAX_RESPONSE_BYTES`], matches response ids, gives every exchange an
//! absolute deadline, and retires — kills and reaps — a helper that
//! violates any of it.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::error::{Error, Result};

/// The most bytes one line may carry. A state report is a few hundred
/// names at most; a line that reaches a megabyte is a helper that has
/// stopped speaking the protocol.
pub const MAX_RESPONSE_BYTES: usize = 1 << 20;
/// The bytes of stderr kept for diagnostics.
const STDERR_TAIL_BYTES: usize = 4 << 10;
/// How long a shutdown waits for the stderr drainer.
const STDERR_SETTLE: Duration = Duration::from_millis(250);
/// The deadline an exchange gets when the caller names none.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(30);
/// Slack added to a request's own `seconds` bound: the helper's clock
/// starts when it reads the request, and chat or feedback events do not
/// count against either.
const OP_SLACK: Duration = Duration::from_secs(15);

/// One response the helper sent.
#[derive(Debug, Deserialize)]
struct Wire {
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// One event the bot reported on its own.
#[derive(Clone, Debug)]
pub struct Event {
    /// `spawn`, `chat`, `death`, `disconnect`, `feedback`, `server_exit`.
    pub event: String,
    /// Everything else the line carried.
    pub fields: Map<String, Value>,
}

impl Event {
    /// One field as a string, when it is one.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<String> {
        self.fields
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}

/// What the reader thread hands back for one line of stdout.
enum Frame {
    /// A response object: `id` plus `ok`.
    Response(Wire),
    /// An event object: `event` plus fields.
    Event(Event),
    /// The line passed [`MAX_RESPONSE_BYTES`] without ending.
    Oversized,
    /// Stdout closed or failed.
    Closed(Option<std::io::Error>),
}

/// The last [`STDERR_TAIL_BYTES`] the helper wrote to stderr.
#[derive(Default)]
struct Tail {
    bytes: VecDeque<u8>,
}

impl Tail {
    fn push(&mut self, chunk: &[u8]) {
        for byte in chunk {
            if self.bytes.len() == STDERR_TAIL_BYTES {
                self.bytes.pop_front();
            }
            self.bytes.push_back(*byte);
        }
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.iter().copied().collect::<Vec<_>>())
            .trim()
            .to_string()
    }
}

/// Reads stdout one capped, classified line at a time.
fn read_frames(mut stdout: impl BufRead, frames: &mpsc::Sender<Frame>) {
    loop {
        let mut line = Vec::new();
        let read = stdout
            .by_ref()
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line);
        let frame = match read {
            Ok(0) => Frame::Closed(None),
            Ok(_) if line.last() != Some(&b'\n') && line.len() > MAX_RESPONSE_BYTES => {
                Frame::Oversized
            }
            Ok(_) if line.last() != Some(&b'\n') => Frame::Closed(None),
            Ok(_) => classify(&line),
            Err(error) => Frame::Closed(Some(error)),
        };
        let last = !matches!(frame, Frame::Response(_) | Frame::Event(_));
        if frames.send(frame).is_err() || last {
            return;
        }
    }
}

/// One line is a response when it carries `id` and `ok`, an event when
/// it carries `event`, and a protocol fault otherwise.
fn classify(line: &[u8]) -> Frame {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return Frame::Closed(Some(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "the helper printed a line that is not JSON: {}",
                String::from_utf8_lossy(line)
            ),
        )));
    };
    if value.get("id").is_some() && value.get("ok").is_some() {
        match serde_json::from_value::<Wire>(value) {
            Ok(wire) => return Frame::Response(wire),
            Err(error) => {
                return Frame::Closed(Some(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("the helper's response did not decode: {error}"),
                )));
            }
        }
    }
    if let Some(event) = value.get("event").and_then(Value::as_str) {
        let mut fields = value.as_object().cloned().unwrap_or_default();
        fields.remove("event");
        return Frame::Event(Event {
            event: event.to_string(),
            fields,
        });
    }
    Frame::Closed(Some(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "the helper printed a line that is neither response nor event: {}",
            String::from_utf8_lossy(line)
        ),
    )))
}

/// Drains stderr so the helper never blocks on a full pipe, keeping a
/// tail, and reports through `done` when the pipe closes.
fn drain_stderr(mut stderr: impl Read, tail: &Mutex<Tail>, done: &mpsc::Sender<()>) {
    let mut buffer = [0_u8; 4096];
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) | Err(_) => {
                let _ = done.send(());
                return;
            }
            Ok(read) => tail
                .lock()
                .expect("the stderr tail lock is not poisoned")
                .push(&buffer[..read]),
        }
    }
}

/// A supervised `mc-bridge` process.
pub struct Bridge {
    child: Child,
    stdin: ChildStdin,
    frames: Receiver<Frame>,
    stderr: Arc<Mutex<Tail>>,
    stderr_done: Receiver<()>,
    pending: VecDeque<Event>,
    retired: bool,
}

impl Bridge {
    /// Starts the helper at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Bridge`] when the process or its threads will not
    /// spawn.
    pub fn start(path: &Path) -> Result<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                Error::bridge(format!(
                    "the helper at {} did not start: {error}",
                    path.display()
                ))
            })?;
        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped"));
        let stderr_pipe = child.stderr.take().expect("stderr was piped");
        let (sender, frames) = mpsc::channel();
        std::thread::Builder::new()
            .name("mc-bridge-stdout".to_string())
            .spawn(move || read_frames(stdout, &sender))
            .map_err(|error| {
                Error::bridge(format!("the helper's reader thread did not start: {error}"))
            })?;
        let stderr = Arc::new(Mutex::new(Tail::default()));
        let tail = Arc::clone(&stderr);
        let (done, stderr_done) = mpsc::channel();
        std::thread::Builder::new()
            .name("mc-bridge-stderr".to_string())
            .spawn(move || drain_stderr(stderr_pipe, &tail, &done))
            .map_err(|error| {
                Error::bridge(format!(
                    "the helper's stderr drainer did not start: {error}"
                ))
            })?;
        Ok(Self {
            child,
            stdin,
            frames,
            stderr,
            stderr_done,
            pending: VecDeque::new(),
            retired: false,
        })
    }

    /// Whether a fault retired this helper.
    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.retired
    }

    /// The events that arrived since the last drain, oldest first.
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.pending.drain(..).collect()
    }

    /// The most recent diagnostics the helper wrote to stderr.
    #[must_use]
    pub fn stderr_tail(&self) -> String {
        self.stderr
            .lock()
            .expect("the stderr tail lock is not poisoned")
            .text()
    }

    /// Kills and reaps the helper and marks it unusable.
    fn retire(&mut self, why: String) -> Error {
        self.retired = true;
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.stderr_done.recv_timeout(STDERR_SETTLE);
        let tail = self.stderr_tail();
        let message = if tail.is_empty() {
            why
        } else {
            format!("{why}; the helper's last diagnostics: {tail}")
        };
        Error::bridge(message)
    }

    /// One exchange: a request object and the response that names it back.
    /// Events the bot emits mid-exchange are queued for
    /// [`Bridge::drain_events`], not dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Refused`] for the helper's typed failure and
    /// [`Error::Bridge`] for a protocol fault that retires the helper.
    pub fn call(&mut self, op: &str, args: Value, deadline: Duration) -> Result<Value> {
        if self.retired {
            return Err(Error::bridge(
                "the helper was retired after an earlier fault",
            ));
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let request = json!({"id": id, "op": op, "args": args});
        let line = serde_json::to_string(&request).map_err(Error::Json)?;
        if let Err(error) = writeln!(self.stdin, "{line}").and_then(|()| self.stdin.flush()) {
            return Err(self.retire(format!("the helper closed its input: {error}")));
        }
        let end = Instant::now() + deadline;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(self.retire(format!(
                    "the helper did not answer within {} ms",
                    deadline.as_millis()
                )));
            }
            match self.frames.recv_timeout(left) {
                Ok(Frame::Response(wire)) if wire.id == id => {
                    if wire.ok {
                        return Ok(wire.result);
                    }
                    return Err(Error::Refused {
                        code: wire.code.unwrap_or_else(|| "unknown".to_string()),
                        message: wire
                            .error
                            .unwrap_or_else(|| "the helper refused without a reason".to_string()),
                    });
                }
                Ok(Frame::Response(wire)) => {
                    return Err(self.retire(format!(
                        "the helper answered call {} when {id} was asked",
                        wire.id
                    )));
                }
                Ok(Frame::Event(event)) => self.pending.push_back(event),
                Ok(Frame::Oversized) => {
                    return Err(self.retire(format!(
                        "the helper answered with a line over {MAX_RESPONSE_BYTES} bytes"
                    )));
                }
                Ok(Frame::Closed(Some(error))) => {
                    return Err(self.retire(format!("the helper closed its output: {error}")));
                }
                Ok(Frame::Closed(None)) | Err(RecvTimeoutError::Disconnected) => {
                    return Err(self.retire("the helper exited".to_string()));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(self.retire(format!(
                        "the helper did not answer within {} ms",
                        deadline.as_millis()
                    )));
                }
            }
        }
    }

    /// `join`: connect the bot to `address` as `username`.
    pub fn join(&mut self, address: &str, username: &str) -> Result<Value> {
        self.call(
            "join",
            json!({"address": address, "username": username}),
            DEFAULT_DEADLINE + Duration::from_secs(45),
        )
    }

    /// `state`: a bounded scan around the bot.
    pub fn state(&mut self, radius: u32) -> Result<Value> {
        self.call("state", json!({"radius": radius}), DEFAULT_DEADLINE)
    }

    /// `say`: chat text other players — and the person watching — can see.
    pub fn say(&mut self, text: &str) -> Result<Value> {
        self.call("say", json!({"text": text}), DEFAULT_DEADLINE)
    }

    /// `goto` on the horizontal plane.
    pub fn goto(&mut self, x: i64, z: i64, seconds: u64) -> Result<Value> {
        self.call(
            "goto",
            json!({"x": x, "z": z, "seconds": seconds}),
            Duration::from_secs(seconds) + OP_SLACK,
        )
    }

    /// `explore`: walk a compass direction for at most `seconds`.
    pub fn explore(&mut self, direction: &str, distance: u32, seconds: u64) -> Result<Value> {
        self.call(
            "explore",
            json!({"direction": direction, "distance": distance, "seconds": seconds}),
            Duration::from_secs(seconds) + OP_SLACK,
        )
    }

    /// `mine`: dig `count` blocks matching `names` within `radius`.
    pub fn mine(&mut self, names: &[&str], count: u32, radius: u32, seconds: u64) -> Result<Value> {
        self.call(
            "mine",
            json!({"names": names, "count": count, "radius": radius, "seconds": seconds}),
            Duration::from_secs(seconds) + OP_SLACK,
        )
    }

    /// `wait`: let the world run for `seconds`.
    pub fn wait(&mut self, seconds: u64) -> Result<Value> {
        self.call(
            "wait",
            json!({"seconds": seconds}),
            Duration::from_secs(seconds) + OP_SLACK,
        )
    }

    /// `disconnect`: the bot leaves the world.
    pub fn disconnect(&mut self) -> Result<Value> {
        self.call("disconnect", json!({}), DEFAULT_DEADLINE)
    }

    /// `shutdown`: the helper exits. The bridge is dropped afterwards.
    pub fn shutdown(&mut self) -> Result<()> {
        if self.retired {
            return Ok(());
        }
        let _ = self.call("shutdown", json!({}), DEFAULT_DEADLINE);
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        // Best effort: ask first so the helper can disconnect cleanly.
        let _ = self.shutdown();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Where the built helper lives: `VOYAGER_MC_BRIDGE`, then the script's
/// output path relative to this crate's worktree.
///
/// # Errors
///
/// Returns [`Error::Bridge`] when no candidate exists.
pub fn helper_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("VOYAGER_MC_BRIDGE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(Error::bridge(format!(
            "VOYAGER_MC_BRIDGE names {}, which is not a file",
            path.display()
        )));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [
        root.join("target-mc/release/mc-bridge"),
        root.join("target-mc/debug/mc-bridge"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(Error::bridge(
        "no mc-bridge binary; run ./scripts/build-mc-bridge.sh first".to_string(),
    ))
}
