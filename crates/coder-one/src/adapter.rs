//! The Claude Code and Codex adapters as controllable sessions.
//!
//! [`CliSession`] runs one executor session under [`supervise::Live`]: the
//! process is the leader of its own group, the host writes its input,
//! reads its stream as it arrives, and can stop it with a cleanup
//! acknowledgement. What each adapter can do, and the tests that show it:
//!
//! | Capability | Claude Code 2.1.280 | Codex 0.155.1 |
//! | --- | --- | --- |
//! | Start | `claude -p --session-id <uuid>`; the host picks the ID | `codex exec --json -`; the ID is the stream's `thread_id` |
//! | Observe | stream-json read as it arrives | `--json` read as it arrives |
//! | Stop | `SIGTERM`, then `SIGKILL`, to the process group; acknowledged once the group is empty | The same |
//! | Resume | `claude -p --resume <id>` | `codex exec resume <id> -` |
//! | Steer | `--input-format stream-json`: a user message written into the running process | Refused: `codex exec` reads one prompt and closes its input |
//!
//! Steering needs Claude Code's stream-json input, which keeps the process
//! reading after its first message. The adapter uses it only when a policy
//! steers. In that mode a turn ends with a `result` event and the process
//! waits for more input, so the adapter closes the input one host tick
//! after a `result` unless the host steered in that tick; that is what
//! ends the process. Without steering, the briefing is written as text and
//! the input is closed at once, as the file redirect did before.
//!
//! The stream is read incrementally through [`crate::tail::Normalizer`]
//! with bounded records and explicit gaps, summarized line by line through
//! [`SummaryReader`], and retained through [`Retainer`]: the first
//! [`STREAM_KEEP`] bytes are written to the stream file as they arrive, so
//! a reader can follow it, and past that the file keeps its first and last
//! halves, as [`crate::delegate::keep_ends`] cuts them. Memory for the
//! retained tail is half of [`STREAM_KEEP`].
//!
//! Both CLIs report session totals, not process totals, after a resume:
//! a resumed Claude Code session's `total_cost_usd` and a resumed Codex
//! thread's usage include the earlier processes, as captured against the
//! local model server on 2026-09-22. The session's summary is therefore
//! the last process's.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::delegate::{
    self, Agent, Briefing, Cli, Launch, Report, SessionArg, Status, Summary, SummaryReader, Wrap,
};
use crate::session::{Capabilities, Session, StopAck};
use crate::stream::{Event, Format, Kind};
use crate::tail::Normalizer;

/// The bytes one stream file keeps: its first and last halves past this.
pub const STREAM_KEEP: usize = 8 * 1024 * 1024;

/// The longest the adapter waits between looks at a running process.
const POLL: Duration = Duration::from_millis(20);

/// What each CLI adapter has demonstrated, and the tests that show it.
#[must_use]
pub fn capabilities(agent: Agent) -> (Capabilities, &'static str) {
    match agent {
        Agent::ClaudeCode => (
            Capabilities::all(),
            "Demonstrated by coder_one::adapter's tests against a stand-in CLI and, with `coder-one capabilities --demonstrate`, against Claude Code 2.1.280 and a local model server: start with --session-id, observe the stream as it arrives, stop with an empty process group, resume with --resume, and steer through --input-format stream-json.",
        ),
        Agent::Microluna => (
            Capabilities::start_only(),
            "Microluna runs in this process: coder_one::micro starts each short session and records its events as they happen, and the host doesn't stop, resume, or steer it; its own bounds end it.",
        ),
        Agent::Codex => (
            Capabilities {
                steer: false,
                ..Capabilities::all()
            },
            "Demonstrated by coder_one::adapter's tests against a stand-in CLI and, with `coder-one capabilities --demonstrate`, against Codex 0.155.1 and a local model server: start, observe, stop with an empty process group, and resume with `codex exec resume`. Steering is refused: `codex exec` reads one prompt and closes its input.",
        ),
    }
}

/// A new session ID in UUID version 4 form, which Claude Code's
/// `--session-id` requires.
#[must_use]
pub fn new_session_id() -> String {
    let mut bytes = [0u8; 16];
    let random = std::fs::File::open("/dev/urandom")
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut bytes));
    if random.is_err() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seed = format!(
            "{}-{}-{}",
            std::process::id(),
            atif::now_ms(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        bytes.copy_from_slice(&Sha256::digest(seed.as_bytes())[..16]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// One Claude Code stream-json user message.
#[must_use]
pub fn user_message(text: &str) -> String {
    let mut line = json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
    })
    .to_string();
    line.push('\n');
    line
}

/// A native stream kept on disk while it arrives, bounded.
pub struct Retainer {
    path: PathBuf,
    recorded: String,
    file: Option<std::fs::File>,
    written: usize,
    keep: usize,
    tail: VecDeque<u8>,
    total: u64,
    gaps: Vec<supervise::Gap>,
}

impl Retainer {
    /// Starts `path`, which the record names `recorded`, keeping `keep`
    /// bytes.
    #[must_use]
    pub fn create(path: &Path, recorded: &str, keep: usize) -> Self {
        Retainer {
            path: path.to_path_buf(),
            recorded: recorded.to_string(),
            file: std::fs::File::create(path).ok(),
            written: 0,
            keep: keep.max(2),
            tail: VecDeque::new(),
            total: 0,
            gaps: Vec::new(),
        }
    }

    /// Keeps a chunk.
    pub fn push(&mut self, bytes: &[u8]) {
        self.total += bytes.len() as u64;
        let room = self.keep.saturating_sub(self.written).min(bytes.len());
        if room > 0
            && let Some(file) = &mut self.file
            && file.write_all(&bytes[..room]).is_ok()
        {
            self.written += room;
        }
        let half = self.keep / 2;
        self.tail.extend(&bytes[bytes.len().saturating_sub(half)..]);
        while self.tail.len() > half {
            self.tail.pop_front();
        }
    }

    /// Notes a span the supervisor dropped.
    pub fn gap(&mut self, gap: supervise::Gap) {
        self.total += gap.bytes;
        self.gaps.push(gap);
        // The tail can't straddle a hole: what it held came before it.
        self.tail.clear();
    }

    /// Finishes the file and returns its record.
    pub fn finish(mut self) -> Option<Value> {
        let file = self.file.take()?;
        drop(file);
        let truncated = self.total > self.keep as u64;
        if truncated {
            let head = std::fs::read(&self.path).ok()?;
            let half = self.keep / 2;
            let head = &head[..half.min(head.len())];
            let head_end = head
                .iter()
                .rposition(|byte| *byte == b'\n')
                .map_or(head.len(), |i| i + 1);
            let tail: Vec<u8> = self.tail.iter().copied().collect();
            let tail_start = tail
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(0, |i| i + 1);
            let omitted = self.total - head_end as u64 - (tail.len() - tail_start) as u64;
            let marker = json!({
                "type": "openagents_truncated",
                "omitted_bytes": omitted,
                "total_bytes": self.total,
            });
            let mut kept = head[..head_end].to_vec();
            kept.extend_from_slice(marker.to_string().as_bytes());
            kept.push(b'\n');
            kept.extend_from_slice(&tail[tail_start..]);
            crate::record::write_atomic(&self.path, &kept).ok()?;
        }
        let kept = std::fs::read(&self.path).ok()?;
        Some(json!({
            "path": self.recorded,
            "bytes": self.total,
            "kept_bytes": kept.len(),
            "truncated": truncated,
            "sha256": Sha256::digest(&kept).iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "gaps": self.gaps.iter().map(|gap| json!({ "offset": gap.offset, "bytes": gap.bytes })).collect::<Vec<_>>(),
            "retention": format!("first {} bytes written as they arrive; past that, the first and last halves", self.keep),
        }))
    }
}

/// One process of a session: the start, or a resume.
struct Process {
    live: Option<supervise::Live>,
    normalizer: Normalizer,
    summary: SummaryReader,
    retainer: Option<Retainer>,
    ended: Option<supervise::Stopped>,
    /// Whether the stream said the session ended.
    said_ended: bool,
    /// When a steerable process finished a turn: the host tick it was seen.
    turn_over: Option<u64>,
    last_line: usize,
    /// Whether the stream says only that the provider is unreachable.
    transport: crate::transport::Watch,
}

/// A Claude Code or Codex session the host controls.
pub struct CliSession<'a> {
    pub cli: &'a Cli,
    wrap: &'a Wrap<'a>,
    binary: PathBuf,
    /// Each process's wall deadline.
    pub deadline: Duration,
    /// Whether to launch Claude Code with stream-json input.
    pub steerable: bool,
    /// A session to continue instead of starting a new one: the first
    /// process resumes this ID with the briefing as its message.
    pub resume: Option<String>,
    /// The stream files' base name, such as `delegate-1`.
    name: String,
    session_id: Option<String>,
    processes: Vec<Process>,
    pending: Vec<Event>,
    clock: Instant,
    stopped_by_deadline: bool,
    stop_reason: Option<String>,
    stderr: String,
    /// A failure before any process ran.
    harness: Option<String>,
    /// How long a Codex process may report only connection errors before
    /// the adapter ends it; [`crate::transport::BOUND`] by default.
    pub transport_bound: Duration,
    /// Why the adapter ended a process that couldn't reach its provider,
    /// and whether it made progress first.
    transport: Option<(String, bool)>,
}

impl<'a> CliSession<'a> {
    /// A session of `cli`'s agent, run through `wrap`, writing its streams
    /// under `name`.
    #[must_use]
    pub fn new(cli: &'a Cli, binary: PathBuf, wrap: &'a Wrap<'a>, name: &str) -> Self {
        CliSession {
            cli,
            wrap,
            binary,
            deadline: cli.deadline,
            steerable: false,
            resume: None,
            name: name.to_string(),
            session_id: None,
            processes: Vec::new(),
            pending: Vec::new(),
            clock: Instant::now(),
            stopped_by_deadline: false,
            stop_reason: None,
            stderr: String::new(),
            harness: None,
            transport_bound: crate::transport::BOUND,
            transport: None,
        }
    }

    fn format(&self) -> Format {
        match self.cli.agent {
            Agent::ClaudeCode => Format::Claude,
            Agent::Codex | Agent::Microluna => Format::Codex,
        }
    }

    fn stream_name(&self, generation: usize) -> String {
        if generation <= 1 {
            format!("{}.stream.jsonl", self.name)
        } else {
            format!("{}.resume-{}.stream.jsonl", self.name, generation - 1)
        }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.clock.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Spawns one process and writes its first message.
    async fn launch(&mut self, session: SessionArg, message: &str) -> Result<(), String> {
        let launch = Launch {
            session,
            steerable: self.steerable && self.cli.agent == Agent::ClaudeCode,
        };
        let command = (self.wrap)(self.cli.live_command(&self.binary, &launch))?;
        let generation = self.processes.len() + 1;
        let stream_name = self.stream_name(generation);
        let retainer = Retainer::create(
            &self.cli.artifacts.join(&stream_name),
            &format!("{}/{stream_name}", self.cli.artifacts_label),
            STREAM_KEEP,
        );
        let mut live = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(self.deadline).keeping(STREAM_KEEP))
            .start(supervise::Input::Piped)?;
        let written = if launch.steerable {
            live.send(user_message(message).as_bytes()).await
        } else {
            let sent = live.send(message.as_bytes()).await;
            live.close_input();
            sent
        };
        self.processes.push(Process {
            live: Some(live),
            normalizer: Normalizer::of(self.format()),
            summary: SummaryReader::of(self.cli.agent, &self.cli.model),
            retainer: Some(retainer),
            ended: None,
            said_ended: false,
            turn_over: None,
            last_line: 0,
            transport: crate::transport::Watch::new(self.transport_bound),
        });
        // A process that exits before reading its input is reported by its
        // ending, not by the write.
        if let Err(error) = written {
            self.stderr
                .push_str(&format!("writing the first message failed: {error}\n"));
        }
        Ok(())
    }

    /// Reads what the current process delivered since the last look.
    fn pump(&mut self, delivery: supervise::Delivery, at_ms: u64) {
        let Some(process) = self.processes.last_mut() else {
            return;
        };
        for gap in &delivery.gaps {
            process.normalizer.reader.dropped(gap.offset, gap.bytes);
            if let Some(retainer) = &mut process.retainer {
                retainer.gap(*gap);
            }
        }
        if delivery.bytes.is_empty() {
            return;
        }
        if let Some(retainer) = &mut process.retainer {
            retainer.push(&delivery.bytes);
        }
        let summary = &mut process.summary;
        let transport = &mut process.transport;
        let events = process
            .normalizer
            .feed(delivery.offset, &delivery.bytes, &mut |line| {
                summary.line(line);
                transport.line(line, at_ms);
            });
        for event in &events {
            if let Kind::SessionStarted {
                session_id: Some(id),
            } = &event.kind
            {
                self.session_id.get_or_insert_with(|| id.clone());
            }
            if matches!(event.kind, Kind::SessionEnded { .. }) {
                process.said_ended = true;
                process.turn_over = Some(at_ms);
            }
            process.last_line = event.line;
        }
        self.pending.extend(events);
    }

    /// Ends the current process's reading once it has exited.
    fn close(&mut self, stopped: supervise::Stopped, at_ms: u64) {
        let rest = stopped.rest.clone();
        self.pump(rest, at_ms);
        let format = self.format();
        let Some(process) = self.processes.last_mut() else {
            return;
        };
        let summary = &mut process.summary;
        let mut events = process.normalizer.finish(&mut |line| summary.line(line));
        if !process.said_ended && format == Format::Codex {
            // Codex ends a successful turn with `turn.completed` and no
            // end event; the process ending is the session ending.
            let seq = events
                .last()
                .map_or(process.normalizer.seq(), |event| event.seq)
                + 1;
            events.push(Event {
                seq,
                line: process.last_line,
                offset: None,
                kind: Kind::SessionEnded {
                    error: !stopped.ending.success(),
                    result: None,
                },
            });
        }
        self.pending.extend(events);
        let stderr = stopped.stderr.marked();
        if !stderr.trim().is_empty() {
            self.stderr.push_str(&stderr);
        }
        process.ended = Some(stopped);
    }

    /// The stream records of every process, first to last.
    fn stream_records(&mut self) -> Vec<Value> {
        self.processes
            .iter_mut()
            .filter_map(|process| process.retainer.take().and_then(Retainer::finish))
            .collect()
    }

    /// Ends every process still running, as a dropped session must.
    async fn abandon(&mut self) {
        for process in &mut self.processes {
            if let Some(live) = process.live.take() {
                process.ended = Some(live.stop().await);
            }
        }
    }
}

impl Session for CliSession<'_> {
    fn adapter(&self) -> &str {
        self.cli.agent.word()
    }

    fn capabilities(&self) -> Capabilities {
        capabilities(self.cli.agent).0
    }

    fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    async fn start(&mut self, briefing: &Briefing) -> Result<(), String> {
        if !self.processes.is_empty() {
            return Err("the session already started".to_string());
        }
        self.clock = Instant::now();
        let session = match (self.resume.clone(), self.cli.agent) {
            (Some(id), _) => {
                self.session_id = Some(id.clone());
                SessionArg::Resume(id)
            }
            (None, Agent::ClaudeCode) => {
                let id = new_session_id();
                self.session_id = Some(id.clone());
                SessionArg::New(Some(id))
            }
            (None, Agent::Codex | Agent::Microluna) => SessionArg::New(None),
        };
        let started = self.launch(session, &briefing.text).await;
        if let Err(error) = &started {
            self.harness = Some(error.clone());
        }
        started
    }

    async fn advance(&mut self, now_ms: u64) -> bool {
        loop {
            let finished = self
                .processes
                .last()
                .and_then(|process| process.live.as_ref())
                .is_none_or(supervise::Live::finished);
            let elapsed = self.elapsed_ms();
            if finished || elapsed >= now_ms {
                break;
            }
            tokio::time::sleep(POLL.min(Duration::from_millis(now_ms - elapsed))).await;
        }
        let delivery = self
            .processes
            .last()
            .and_then(|process| process.live.as_ref())
            .map(supervise::Live::take);
        let Some(delivery) = delivery else {
            return false;
        };
        self.pump(delivery, now_ms);
        let Some(process) = self.processes.last_mut() else {
            return false;
        };
        // A steerable process waits for more input after its turn; one
        // tick after the turn ends with no steer, the input closes.
        if let (Some(at), Some(live)) = (process.turn_over, process.live.as_mut())
            && at < now_ms
            && live.input_open()
        {
            live.close_input();
        }
        // A Codex process that has reported only connection errors for its
        // bound won't recover on its own: it retries until its deadline.
        let unreachable = (self.cli.agent == Agent::Codex)
            .then(|| process.transport.expired(now_ms))
            .flatten();
        if let Some(why) = unreachable
            && let Some(live) = process.live.take()
        {
            let reached = process.transport.reached();
            crate::say::say!("  delegate ▸ stopped the session: {why}");
            let stopped = live.stop().await;
            self.close(stopped, now_ms);
            self.transport = Some((why, reached));
            return false;
        }
        let finished = process.live.as_ref().is_some_and(supervise::Live::finished);
        if finished && let Some(live) = process.live.take() {
            let stopped = live.wait().await;
            self.close(stopped, now_ms);
            return false;
        }
        true
    }

    fn observe(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending)
    }

    async fn stop(&mut self, now_ms: u64, reason: &str) -> Result<StopAck, String> {
        let live = self
            .processes
            .last_mut()
            .and_then(|process| process.live.take())
            .ok_or_else(|| "no process is running".to_string())?;
        let pending = live.take();
        self.pump(pending, now_ms);
        let stopped = live.stop().await;
        let group = stopped.group.map_or_else(
            || "its process group".to_string(),
            |g| format!("process group {g}"),
        );
        let cleanup = format!(
            "SIGTERM to {group}, {}; the group was {} after cleanup",
            if stopped.graceful {
                "which exited within the grace period"
            } else {
                "then SIGKILL after the grace period"
            },
            if stopped.group_clear {
                "empty"
            } else {
                "NOT empty"
            },
        );
        let clear = stopped.group_clear;
        self.close(stopped, now_ms);
        self.stopped_by_deadline = reason.contains("deadline");
        self.stop_reason = Some(reason.to_string());
        if !clear {
            return Err(cleanup);
        }
        Ok(StopAck {
            at_ms: now_ms,
            reason: reason.to_string(),
            pending: 1,
            cleanup,
        })
    }

    async fn resume(&mut self, _now_ms: u64, message: &str) -> Result<(), String> {
        if self
            .processes
            .last()
            .is_some_and(|process| process.live.is_some())
        {
            return Err("the session is still running".to_string());
        }
        let id = self
            .session_id
            .clone()
            .ok_or_else(|| "the session never reported an ID to resume".to_string())?;
        self.stopped_by_deadline = false;
        self.stop_reason = None;
        self.transport = None;
        self.launch(SessionArg::Resume(id), message).await
    }

    async fn steer(&mut self, _now_ms: u64, message: &str) -> Result<(), String> {
        if self.cli.agent != Agent::ClaudeCode || !self.steerable {
            return Err(
                "this session was not launched to accept messages while it runs".to_string(),
            );
        }
        let process = self
            .processes
            .last_mut()
            .ok_or_else(|| "no process is running".to_string())?;
        let live = process
            .live
            .as_mut()
            .filter(|live| live.input_open())
            .ok_or_else(|| "the running process no longer reads input".to_string())?;
        live.send(user_message(message).as_bytes()).await?;
        process.turn_over = None;
        process.said_ended = false;
        Ok(())
    }

    fn report(&mut self) -> Report {
        let milliseconds = self.elapsed_ms();
        let streams = self.stream_records();
        let summary = self
            .processes
            .last()
            .map(|process| process.summary.clone().finish())
            .unwrap_or_default();
        let stderr = self.stderr.clone();
        let status = if let Some(why) = &self.harness {
            Status::Harness(why.clone())
        } else if let Some((detail, reached)) = &self.transport {
            Status::Transport {
                detail: detail.clone(),
                reached: *reached,
            }
        } else if self.stopped_by_deadline {
            Status::TimedOut
        } else if let Some(reason) = &self.stop_reason {
            Status::Harness(format!("stopped by the host: {reason}"))
        } else {
            match self
                .processes
                .last()
                .and_then(|process| process.ended.as_ref())
            {
                Some(stopped) => delegate::classify(&stopped.ending, &summary, &stderr),
                None => Status::Harness("the session was still running".to_string()),
            }
        };
        let stream = streams.first().cloned().map(|mut first| {
            if streams.len() > 1
                && let Value::Object(map) = &mut first
            {
                map.insert("resumed".to_string(), json!(streams[1..].to_vec()));
            }
            first
        });
        Report {
            status,
            summary,
            milliseconds,
            stderr: crate::judge::clip_tail(&stderr, 4_000),
            stream,
        }
    }
}

impl CliSession<'_> {
    /// Stops anything still running; call before dropping a session the
    /// host abandons mid-run. Dropping also ends each process group, but
    /// without waiting for the acknowledgement.
    pub async fn shutdown(&mut self) {
        self.abandon().await;
    }

    /// Each process's summary, first to last.
    #[must_use]
    pub fn summaries(&self) -> Vec<Summary> {
        self.processes
            .iter()
            .map(|process| process.summary.clone().finish())
            .collect()
    }
}

/// Stand-ins for the two CLIs, for tests: shell scripts that speak each
/// CLI's arguments and stream format closely enough to exercise every
/// capability, and nothing else.
pub mod standin {
    use std::path::{Path, PathBuf};

    /// Claude Code's print mode: stream-json out; text or stream-json in;
    /// `--session-id` and `--resume` name the session. Each user message
    /// is a turn that answers `heard <word>`, where the word is `steer`,
    /// `resume`, or `briefing` by what the message says.
    /// `STANDIN_DELAY` sleeps between a turn's events; `STANDIN_HANG`
    /// names a marker a background child writes after two seconds while
    /// the turn hangs.
    pub const CLAUDE: &str = r#"#!/bin/sh
id=""; json_in=0
while [ $# -gt 0 ]; do
  case "$1" in
    --session-id|--resume) id=$2; shift ;;
    --input-format) [ "$2" = stream-json ] && json_in=1; shift ;;
  esac
  shift
done
n=0
turn() {
  n=$((n+1))
  echo "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"$id\",\"model\":\"stand-in\",\"claude_code_version\":\"stand-in\"}"
  sleep "${STANDIN_DELAY:-0}"
  if [ -n "$STANDIN_HANG" ] && [ "$1" = briefing ]; then
    (sleep 2; printf harmless > "$STANDIN_HANG") &
    wait
  fi
  echo "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_$1_$n\",\"content\":[{\"type\":\"tool_use\",\"id\":\"t$n\",\"name\":\"Write\",\"input\":{\"file_path\":\"out-$1.txt\",\"content\":\"x\"}}],\"usage\":{\"input_tokens\":10}}}"
  echo "{\"type\":\"assistant\",\"message\":{\"id\":\"msg_$1_$n\",\"content\":[{\"type\":\"text\",\"text\":\"heard $1\"}],\"usage\":{\"input_tokens\":10}}}"
  sleep "${STANDIN_DELAY:-0}"
  echo "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":$n,\"result\":\"heard $1\",\"session_id\":\"$id\",\"total_cost_usd\":0.01}"
}
word() {
  case "$1" in *steer*) echo steer ;; *resume*) echo resume ;; *) echo briefing ;; esac
}
if [ "$json_in" = 1 ]; then
  while IFS= read -r line; do turn "$(word "$line")"; done
else
  turn "$(word "$(cat)")"
fi
"#;

    /// `codex exec --json -` and `codex exec resume <id> -`: one turn per
    /// process, a command, and a message that answers `heard <word>`.
    /// `STANDIN_OFFLINE` reports only connection errors until stopped, as
    /// Codex does when it can't reach its provider.
    pub const CODEX: &str = r#"#!/bin/sh
id="0199aaaa-bbbb-7ccc-8ddd-standin00001"
[ "$1" = exec ] && [ "$2" = resume ] && id=$3
input=$(cat)
case "$input" in *resume*) what=resume ;; *) what=briefing ;; esac
echo "{\"type\":\"thread.started\",\"thread_id\":\"$id\"}"
echo "{\"type\":\"turn.started\"}"
if [ -n "$STANDIN_OFFLINE" ]; then
  echo '{"type":"error","message":"Reconnecting... 2/5 (stream disconnected before completion: invalid peer certificate: UnknownIssuer)"}'
  while :; do
    echo '{"type":"error","message":"Reconnecting... waiting for network (Connection failed: error sending request)"}'
    sleep 0.05
  done
fi
sleep "${STANDIN_DELAY:-0}"
if [ -n "$STANDIN_HANG" ] && [ "$what" = briefing ]; then
  (sleep 2; printf harmless > "$STANDIN_HANG") &
  wait
fi
echo "{\"type\":\"item.started\",\"item\":{\"id\":\"i1\",\"type\":\"command_execution\",\"command\":\"false\",\"aggregated_output\":\"\",\"exit_code\":null,\"status\":\"in_progress\"}}"
echo "{\"type\":\"item.completed\",\"item\":{\"id\":\"i1\",\"type\":\"command_execution\",\"command\":\"false\",\"aggregated_output\":\"\",\"exit_code\":1,\"status\":\"completed\"}}"
echo "{\"type\":\"item.completed\",\"item\":{\"id\":\"i2\",\"type\":\"agent_message\",\"text\":\"heard $what\"}}"
sleep "${STANDIN_DELAY:-0}"
echo "{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":10,\"cached_input_tokens\":0,\"output_tokens\":2}}"
"#;

    /// Writes `script` as an executable `name` under `dir`.
    ///
    /// # Panics
    ///
    /// Panics when the file can't be written; it is for tests.
    #[must_use]
    pub fn install(dir: &Path, name: &str, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).expect("stand-in directory");
        let path = dir.join(name);
        std::fs::write(&path, script).expect("stand-in script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("stand-in mode");
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::{BriefingInputs, Credential};
    use crate::record::Recorder;
    use crate::session::{self, Capability, Controls, Driven, Phase, Steer, Trigger};

    fn briefing() -> Briefing {
        Briefing::build(
            &BriefingInputs {
                instruction: "Write the file.".to_string(),
                requirements: vec![],
                files: vec![],
                spans: vec![],
                commands: vec![],
                last_output: None,
                conclusion: String::new(),
                directions: "Go.".to_string(),
            },
            2_000,
        )
    }

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-adapter-{label}-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(dir.join("artifacts")).unwrap();
        dir
    }

    fn cli(agent: Agent, dir: &Path, env: &[(&str, &str)]) -> Cli {
        let (name, script) = match agent {
            Agent::ClaudeCode => ("claude", standin::CLAUDE),
            Agent::Codex | Agent::Microluna => ("codex", standin::CODEX),
        };
        let binary = standin::install(&dir.join("bin"), name, script);
        Cli {
            agent,
            binary: Some(binary),
            model: "stand-in".to_string(),
            deadline: Duration::from_secs(30),
            workdir: dir.to_path_buf(),
            artifacts: dir.join("artifacts"),
            artifacts_label: "artifacts".to_string(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            credential: Credential::OauthToken,
            effort: None,
            tools: None,
            prompt_cache_ttl: None,
            system: None,
            episode: crate::deadline::Deadline::unbounded(),
            gate: None,
            granted: None,
            runs: 1,
            control: crate::delegate::Control::default(),
        }
    }

    async fn run(cli: &Cli, controls: &Controls, steerable: bool) -> (Driven, Recorder) {
        let recorder = Recorder::default();
        let wrap = Ok;
        let mut session = CliSession::new(cli, cli.binary.clone().unwrap(), &wrap, "delegate-1");
        session.steerable = steerable;
        let driven = session::drive(
            &mut session,
            &briefing(),
            controls,
            &recorder,
            &mut session::virtual_time(),
        )
        .await;
        session.shutdown().await;
        (driven, recorder)
    }

    fn controls() -> Controls {
        Controls {
            deadline_ms: 20_000,
            tick_ms: 20,
            ..Controls::default()
        }
    }

    fn claims(driven: &Driven) -> Vec<String> {
        driven
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                Kind::AssistantClaim { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn action(driven: &Driven, capability: Capability) -> Option<&session::Action> {
        driven
            .actions
            .iter()
            .find(|action| action.capability == capability)
    }

    #[test]
    fn every_claude_code_launch_keeps_the_accounts_connectors_out() {
        let dir = scratch("claude-connectors");
        let connectors = |command: &std::process::Command| {
            command
                .get_envs()
                .find(|(name, _)| *name == "ENABLE_CLAUDEAI_MCP_SERVERS")
                .and_then(|(_, value)| value)
                .map(|value| value.to_string_lossy().into_owned())
        };
        let claude = cli(Agent::ClaudeCode, &dir, &[]);
        let binary = claude.binary.clone().unwrap();
        let batch = claude.command(&binary, &dir.join("b"), &dir.join("s"));
        let launch = Launch {
            session: SessionArg::New(None),
            steerable: true,
        };
        let live = claude.live_command(&binary, &launch);
        assert_eq!(connectors(&batch).as_deref(), Some("false"));
        assert_eq!(connectors(&live).as_deref(), Some("false"));
        let codex = cli(Agent::Codex, &dir, &[]);
        let binary = codex.binary.clone().unwrap();
        assert_eq!(connectors(&codex.live_command(&binary, &launch)), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn claude_code_starts_under_the_hosts_session_id_and_is_observed_as_it_runs() {
        let dir = scratch("claude-observe");
        let cli = cli(Agent::ClaudeCode, &dir, &[("STANDIN_DELAY", "0.4")]);
        let (driven, recorder) = run(&cli, &controls(), false).await;
        assert_eq!(
            driven.report.status,
            Status::Answered,
            "{:?}",
            driven.report
        );
        let id = driven.session_id.clone().unwrap();
        assert_eq!(id.len(), 36, "a UUID the host chose: {id}");
        assert!(matches!(
            &driven.events[0].kind,
            Kind::SessionStarted { session_id: Some(started) } if *started == id
        ));
        assert_eq!(claims(&driven), vec!["heard briefing"]);
        // The first event was recorded while the session still ran.
        let steps: Vec<Value> = recorder
            .steps()
            .iter()
            .filter_map(|step| step.extensions.get(session::EVENT_KEY).cloned())
            .collect();
        let first = steps.first().unwrap()["at_ms"].as_u64().unwrap();
        let last = steps.last().unwrap()["at_ms"].as_u64().unwrap();
        assert!(last >= first + 600, "observed at {first} and {last} ms");
        assert!(steps.iter().all(|s| s["session_id"] == json!(id)));
        assert!(steps.iter().all(|s| s["event"]["offset"].is_u64()));
        // The artifact change moved the workspace revision.
        let revisions: Vec<u64> = steps
            .iter()
            .map(|s| s["revision"].as_u64().unwrap())
            .collect();
        assert_eq!(revisions.first(), Some(&0));
        assert_eq!(revisions.last(), Some(&1));
        let stream = driven.report.stream.clone().unwrap();
        assert_eq!(stream["path"], "artifacts/delegate-1.stream.jsonl");
        assert_eq!(stream["truncated"], false);
        let kept = std::fs::read_to_string(dir.join("artifacts/delegate-1.stream.jsonl")).unwrap();
        assert_eq!(stream["bytes"].as_u64(), Some(kept.len() as u64));
        assert_eq!(driven.report.summary.total_cost_usd, Some(0.01));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn claude_code_is_steered_through_its_stream_json_input() {
        let dir = scratch("claude-steer");
        let cli = cli(Agent::ClaudeCode, &dir, &[]);
        let controls = Controls {
            steer: Some(Steer {
                when: Trigger::Claim {
                    contains: "heard briefing".to_string(),
                },
                message: "Please steer toward the tests.".to_string(),
            }),
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, true).await;
        assert_eq!(action(&driven, Capability::Steer).unwrap().outcome, "done");
        assert_eq!(claims(&driven), vec!["heard briefing", "heard steer"]);
        assert_eq!(driven.report.status, Status::Answered);
        assert_eq!(driven.report.summary.num_turns, Some(2));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_stop_ends_the_process_group_and_a_resume_continues_the_same_session() {
        for agent in [Agent::ClaudeCode, Agent::Codex] {
            let dir = scratch(&format!("{}-stop", agent.word()));
            let marker = dir.join("marker");
            let marker_text = marker.to_string_lossy().into_owned();
            let cli = cli(agent, &dir, &[("STANDIN_HANG", &marker_text)]);
            let controls = Controls {
                stop_when: Some(Trigger::After { ms: 400 }),
                resume: Some("Please resume and finish.".to_string()),
                ..controls()
            };
            let (driven, _) = run(&cli, &controls, false).await;
            let stop = action(&driven, Capability::Stop).unwrap();
            assert_eq!(stop.outcome, "done", "{}: {stop:?}", agent.word());
            assert!(
                stop.detail.contains("the group was empty"),
                "{}",
                stop.detail
            );
            assert_eq!(driven.stops.len(), 1);
            let resume = action(&driven, Capability::Resume).unwrap();
            assert_eq!(resume.outcome, "done", "{}: {resume:?}", agent.word());
            // The resumed process answered under the same session ID.
            let id = driven.session_id.clone().unwrap();
            assert!(resume.detail.contains(&id));
            assert_eq!(claims(&driven).last().unwrap(), "heard resume");
            let generations: Vec<(Phase, u32)> = driven
                .transitions
                .iter()
                .map(|t| (t.to, t.generation))
                .collect();
            assert_eq!(
                generations,
                vec![
                    (Phase::Running, 1),
                    (Phase::Stopped, 1),
                    (Phase::Running, 2),
                    (Phase::Ended, 2),
                ],
                "{}",
                agent.word()
            );
            assert_eq!(driven.report.status, Status::Answered, "{}", agent.word());
            let stream = driven.report.stream.clone().unwrap();
            assert_eq!(
                stream["resumed"][0]["path"],
                "artifacts/delegate-1.resume-1.stream.jsonl"
            );
            tokio::time::sleep(Duration::from_millis(2_500)).await;
            assert!(
                !marker.exists(),
                "{}: a descendant outlived an acknowledged stop",
                agent.word()
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[tokio::test]
    async fn codex_refuses_a_steer_it_has_not_demonstrated_and_still_runs() {
        let dir = scratch("codex-steer");
        let cli = cli(Agent::Codex, &dir, &[("STANDIN_DELAY", "0.3")]);
        let controls = Controls {
            steer: Some(Steer {
                when: Trigger::CommandFailed,
                message: "Check the exit code.".to_string(),
            }),
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, true).await;
        let steer = action(&driven, Capability::Steer).unwrap();
        assert_eq!(steer.outcome, "refused");
        assert!(steer.detail.contains("has not demonstrated steer"));
        assert_eq!(driven.report.status, Status::Answered);
        let kinds: Vec<&str> = driven.events.iter().map(|e| e.kind.word()).collect();
        assert_eq!(
            kinds,
            vec![
                "session_started",
                "command_started",
                "command_completed",
                "assistant_claim",
                "usage_update",
                "session_ended"
            ]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_host_deadline_is_a_timeout_with_an_acknowledged_stop() {
        let dir = scratch("claude-deadline");
        let marker = dir.join("marker");
        let marker_text = marker.to_string_lossy().into_owned();
        let cli = cli(Agent::ClaudeCode, &dir, &[("STANDIN_HANG", &marker_text)]);
        let controls = Controls {
            deadline_ms: 300,
            ..controls()
        };
        let (driven, _) = run(&cli, &controls, false).await;
        assert_eq!(driven.report.status, Status::TimedOut);
        assert_eq!(driven.stops.len(), 1);
        assert!(driven.stops[0].cleanup.contains("empty"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_codex_session_that_only_reports_connection_errors_ends_as_a_transport_failure() {
        let dir = scratch("codex-offline");
        let cli = cli(Agent::Codex, &dir, &[("STANDIN_OFFLINE", "1")]);
        let recorder = Recorder::default();
        let wrap = Ok;
        let mut session = CliSession::new(&cli, cli.binary.clone().unwrap(), &wrap, "delegate-1");
        session.transport_bound = Duration::from_millis(400);
        let driven = session::drive(
            &mut session,
            &briefing(),
            &controls(),
            &recorder,
            &mut session::virtual_time(),
        )
        .await;
        session.shutdown().await;
        let Status::Transport { detail, reached } = &driven.report.status else {
            panic!("not a transport failure: {:?}", driven.report.status);
        };
        assert!(detail.contains("only connection errors"), "{detail}");
        assert!(detail.contains("waiting for network"), "{detail}");
        assert!(!reached);
        assert_eq!(driven.report.status.word(), "transport");
        assert!(
            driven.elapsed_ms < 10_000,
            "ended at {} ms",
            driven.elapsed_ms
        );
        assert!(driven.stops.is_empty(), "the host deadline didn't act");
        assert_eq!(
            delegate::charge(&driven.report),
            ("zero", "the executor never reached its provider")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_matrix_is_what_the_tests_demonstrate() {
        let (claude, _) = capabilities(Agent::ClaudeCode);
        assert!(Capability::ALL.iter().all(|c| claude.has(*c)));
        let (codex, note) = capabilities(Agent::Codex);
        assert!(!codex.steer);
        assert!(codex.start && codex.observe && codex.stop && codex.resume);
        assert!(note.contains("Steering is refused"));
    }

    #[test]
    fn a_session_id_is_a_version_4_uuid() {
        let id = new_session_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(
            parts[3].chars().next(),
            Some('8' | '9' | 'a' | 'b')
        ));
        assert_ne!(id, new_session_id());
    }

    #[test]
    fn a_long_stream_keeps_its_first_and_last_halves_as_keep_ends_does() {
        let dir = scratch("retain");
        let path = dir.join("stream.jsonl");
        let raw: String = (0..400).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let mut retainer = Retainer::create(&path, "artifacts/stream.jsonl", 1_000);
        for chunk in raw.as_bytes().chunks(37) {
            retainer.push(chunk);
        }
        let record = retainer.finish().unwrap();
        let kept = std::fs::read(&path).unwrap();
        assert_eq!(kept, delegate::keep_ends(raw.as_bytes(), 1_000));
        assert_eq!(record["truncated"], true);
        assert_eq!(record["bytes"].as_u64(), Some(raw.len() as u64));
        let _ = std::fs::remove_dir_all(dir);
    }
}
