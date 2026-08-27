//! Local swarm: session discovery and agent-to-agent messaging.
//!
//! Eight tabs running Coder cannot see each other, and a `delegate` child
//! cannot reach its parent except through the one prompt it was spawned with
//! and the one report it leaves behind. This module is the fix, built the way
//! the session store is built: local files, no daemon, no network.
//!
//! Three pieces:
//!
//! - **Registration** ([`registration_path`], [`register`], [`unregister`],
//!   [`list`]) — a `~/.openagents/swarm/<session-id>.json` file per live
//!   session carrying what discovery needs: pid, cwd, lane, model, role,
//!   parent, worktree, and the inbox path. Liveness is pid plus heartbeat
//!   freshness, and a registration past its heartbeat is `Stale` — shown as
//!   stale, never silently disappeared, because a session that is slow is not
//!   a session that is gone.
//! - **Mailboxes** ([`Mailbox`]) — an append-only `inbox.jsonl` and
//!   `outbox.jsonl` in the session's own store directory. Delivery is one
//!   appended line per resolved recipient, so a crash mid-send leaves a whole
//!   line or none, and the file is both the transport and the record.
//! - **Message envelopes** ([`SwarmMessage`]) —
//!   `openagents.swarm.message.v1`, with per-inbox sequence numbers (a gap is
//!   reported corruption, never papered over), delivery/read receipt stamps,
//!   thread references for reply chains, and fan-out addressing
//!   (`role:children-of:<id>`, `all`) resolved at send time, so there is no
//!   routing state anywhere.
//!
//! Every operation is local-only under `~/.openagents/`. There is no network
//! path in this contract; the remote widening is separate work with its own
//! consent and redaction review.
//!
//! Budgets and anti-livelock caps live with the consumers of this module (the
//! turn-boundary drain, `#182` slice 3); delivery itself stays cheap and
//! dumb, which is what makes it safe to call from anywhere.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const REGISTRATION_SCHEMA: &str = "openagents.swarm.registration.v1";
pub const MESSAGE_SCHEMA: &str = "openagents.swarm.message.v1";
pub const LISTING_SCHEMA: &str = "openagents.swarm.listing.v1";

const INBOX_FILE: &str = "inbox.jsonl";
const OUTBOX_FILE: &str = "outbox.jsonl";
const SWARM_DIR: &str = "swarm";

/// The largest message body one delivery accepts, in bytes. The ingest routes
/// cap trace documents; this is the same discipline for mail, sized for
/// meaningful answers rather than transcripts. A body that will not fit is
/// refused at send time, not truncated in flight — a truncated answer is a
/// wrong answer wearing a smaller envelope.
pub const MAXIMUM_BODY_BYTES: usize = 256 * 1024;

/// How fresh a registration's heartbeat must be, in milliseconds, before the
/// session is reported `Live` rather than `Stale`. Generous on purpose: a
/// session mid-long-turn is alive even when it has nothing to say for
/// minutes, and a heartbeat is refreshed on turn boundaries by the frame
/// loop, not on a timer thread.
pub const DEFAULT_ALIVE_AFTER_MS: u128 = 30 * 60 * 1000;

/// One live (or recently live) session, as discovery reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registration {
    pub schema: String,
    pub session_id: String,
    pub pid: u32,
    pub cwd: String,
    pub lane: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `root` for a tab the human opened; `child` for a delegate fan-out
    /// member, which also carries `parent`.
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    /// Where this session's `inbox.jsonl` lives, so a sender never has to
    /// guess the store layout.
    pub inbox: String,
    /// The heartbeat freshness this session asks to be judged by.
    pub alive_after_ms: u128,
    pub started_at_ms: u128,
    /// Refreshed on turn boundaries. Liveness is this plus the pid.
    pub heartbeat_at_ms: u128,
}

impl Registration {
    /// Live: the process exists. Freshness is reported alongside, never
    /// folded in — a slow session and a dead one are different findings.
    pub fn process_alive(&self) -> bool {
        #[cfg(unix)]
        {
            // Signal 0 delivers nowhere; it answers whether the pid exists
            // and belongs to someone this process may signal.
            unsafe { libc::kill(self.pid as i32, 0) == 0 }
        }
        #[cfg(not(unix))]
        {
            // Windows has no signal-0 probe in std. The heartbeat carries
            // the verdict alone there.
            self.heartbeat_fresh()
        }
    }

    pub fn heartbeat_fresh(&self) -> bool {
        now_ms().saturating_sub(self.heartbeat_at_ms) <= self.alive_after_ms
    }

    /// What discovery says about this session right now.
    pub fn state(&self) -> SwarmState {
        if self.process_alive() {
            SwarmState::Live
        } else {
            SwarmState::Stale
        }
    }

    /// Deliverable: the process is alive. A message to a stale session is
    /// refused rather than delivered into a file nobody will read —
    /// fire-and-forget into the void is not a delivery.
    pub fn deliverable(&self) -> bool {
        matches!(self.state(), SwarmState::Live)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmState {
    Live,
    Stale,
}

impl SwarmState {
    pub fn as_str(self) -> &'static str {
        match self {
            SwarmState::Live => "live",
            SwarmState::Stale => "stale",
        }
    }
}

/// One message, as written into an inbox or an outbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmMessage {
    pub schema: String,
    /// `msg_` + 16 hex of the SHA-256 over from+to+body+created_at, so the id
    /// is content-derived and dedup survives a re-send.
    pub id: String,
    /// Sequence within the receiving inbox, 1-based, gapless. Stamped by the
    /// delivering append, not by the sender, so only the inbox owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    pub from: String,
    /// A session id, `role:children-of:<session-id>`, or `all`.
    pub to: String,
    /// The id of the message this one answers or continues, when there is
    /// one — the thread a reply chain hangs from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<String>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_expected: Option<bool>,
    pub body: String,
    pub created_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at_ms: Option<u128>,
}

/// Where a delivered message lands and what it cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReport {
    pub to: String,
    pub state: String,
    pub message_id: String,
    pub sequence: u64,
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default()
}

/// `~/.openagents/swarm`, the registration directory.
pub fn swarm_root(home: &Path) -> PathBuf {
    home.join(".openagents").join(SWARM_DIR)
}

/// Where one session's registration lives.
pub fn registration_path(home: &Path, session_id: &str) -> PathBuf {
    swarm_root(home).join(format!("{session_id}.json"))
}

/// The message id for one send: `msg_` + 16 hex of SHA-256 over the fields
/// that make the message what it is.
fn message_id(from: &str, to: &str, thread: Option<&str>, body: &str, at_ms: u128) -> String {
    let mut hasher = Sha256::new();
    hasher.update(from.as_bytes());
    hasher.update([0]);
    hasher.update(to.as_bytes());
    hasher.update([0]);
    if let Some(thread) = thread {
        hasher.update(thread.as_bytes());
    }
    hasher.update([0]);
    hasher.update(body.as_bytes());
    hasher.update([0]);
    hasher.update(at_ms.to_string().as_bytes());
    let digest = hasher.finalize();
    format!("msg_{}", hex_prefix(&digest))
}

fn hex_prefix(digest: &[u8]) -> String {
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Write this session's registration, creating the swarm directory when it
/// does not exist. Atomic per the house rule: staged beside, renamed in.
pub fn register(home: &Path, registration: &Registration) -> Result<(), String> {
    let root = swarm_root(home);
    std::fs::create_dir_all(&root).map_err(|error| {
        format!(
            "the swarm directory at {} could not be created: {error}",
            root.display()
        )
    })?;
    let path = registration_path(home, &registration.session_id);
    let staged = root.join(format!(".{}.tmp", registration.session_id));
    let text = serde_json::to_string_pretty(registration)
        .map_err(|error| format!("the registration could not be rendered: {error}"))?;
    std::fs::write(&staged, text).map_err(|error| {
        format!(
            "the staged registration at {} could not be written: {error}",
            staged.display()
        )
    })?;
    std::fs::rename(&staged, &path).map_err(|error| {
        format!(
            "the registration at {} could not be moved into place: {error}",
            path.display()
        )
    })?;
    Ok(())
}

/// Refresh the heartbeat on an existing registration. A missing registration
/// is not created here: only the session's own startup registers.
pub fn heartbeat(home: &Path, session_id: &str) -> Result<(), String> {
    let path = registration_path(home, session_id);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        format!(
            "the registration at {} could not be read: {error}",
            path.display()
        )
    })?;
    let mut registration: Registration = serde_json::from_str(&text).map_err(|error| {
        format!(
            "the registration at {} could not be parsed: {error}",
            path.display()
        )
    })?;
    registration.heartbeat_at_ms = now_ms();
    register(home, &registration)
}

/// Remove this session's registration. A missing file is already the goal,
/// so removing a registration that is not there succeeds.
pub fn unregister(home: &Path, session_id: &str) -> Result<(), String> {
    let path = registration_path(home, session_id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "the registration at {} could not be removed: {error}",
            path.display()
        )),
    }
}

/// Read one registration, if it is there and parseable.
pub fn load_registration(home: &Path, session_id: &str) -> Result<Option<Registration>, String> {
    let path = registration_path(home, session_id);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "the registration at {} could not be read: {error}",
                path.display()
            ));
        }
    };
    serde_json::from_str(&text).map(Some).map_err(|error| {
        format!(
            "the registration at {} could not be parsed: {error}",
            path.display()
        )
    })
}

/// Every registration on this machine, sorted by start time. A registration
/// file that will not parse is reported as an error, never skipped: discovery
/// that quietly drops a session is a swarm that lies about who is here.
pub fn list(home: &Path) -> Result<Vec<Registration>, String> {
    let root = swarm_root(home);
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(format!(
                "the swarm directory at {} could not be read: {error}",
                root.display()
            ));
        }
    };
    let mut found: Vec<Registration> = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("the swarm directory could not be walked: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|error| {
            format!(
                "the registration at {} could not be read: {error}",
                path.display()
            )
        })?;
        let registration: Registration = serde_json::from_str(&text).map_err(|error| {
            format!(
                "the registration at {} could not be parsed: {error}",
                path.display()
            )
        })?;
        found.push(registration);
    }
    found.sort_by_key(|registration| registration.started_at_ms);
    Ok(found)
}

// ---------------------------------------------------------------------------
// Mailboxes
// ---------------------------------------------------------------------------

/// One session's mailbox: the inbox it receives at and the outbox it records
/// its sends in. Both are append-only JSON Lines in the session's own store
/// directory, which is what makes a message transport also be the record.
pub struct Mailbox {
    inbox: PathBuf,
    outbox: PathBuf,
}

impl Mailbox {
    /// The mailbox living in a session's store directory.
    pub fn at(session_directory: &Path) -> Self {
        Self {
            inbox: session_directory.join(INBOX_FILE),
            outbox: session_directory.join(OUTBOX_FILE),
        }
    }

    /// Append one message as the next line. `O_APPEND` plus one
    /// `write_all` of a whole line is the atomicity story: a crash leaves
    /// the previous file intact and either the whole line or nothing.
    fn append(path: &Path, message: &SwarmMessage) -> Result<u64, String> {
        let mut line = serde_json::to_string(message)
            .map_err(|error| format!("the message could not be rendered: {error}"))?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| {
                format!(
                    "the mailbox at {} could not be opened for append: {error}",
                    path.display()
                )
            })?;
        file.write_all(line.as_bytes())
            .and_then(|()| file.flush())
            .map_err(|error| {
                format!(
                    "the message could not be appended to {}: {error}",
                    path.display()
                )
            })?;
        Ok(line.len() as u64)
    }

    /// Read every message in an inbox, in file order. A truncated final line
    /// (a crash mid-append on a filesystem without atomic small appends) is
    /// reported as the corruption it is, with the byte offset, rather than
    /// dropped.
    pub fn read(path: &Path) -> Result<Vec<SwarmMessage>, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "the mailbox at {} could not be read: {error}",
                    path.display()
                ));
            }
        };
        let mut messages = Vec::new();
        let reader = BufReader::new(file);
        for (index, line) in reader.lines().enumerate() {
            let line = line.map_err(|error| {
                format!(
                    "the mailbox at {} could not be read at line {}: {error}",
                    path.display(),
                    index + 1
                )
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let message: SwarmMessage = serde_json::from_str(&line).map_err(|error| {
                format!(
                    "the mailbox at {} holds something that is not a message at line {}: {error}",
                    path.display(),
                    index + 1
                )
            })?;
            messages.push(message);
        }
        Ok(messages)
    }

    /// Deliver one message into this mailbox as its next sequence number.
    /// The sequence is computed by reading first, so a delivered batch is
    /// gapless in the absence of concurrent senders — and a gap that does
    /// appear (two senders racing, a lost line) is visible in the numbers
    /// instead of invisible in the file.
    pub fn deliver(&self, mut message: SwarmMessage) -> Result<u64, String> {
        let next = Self::read(&self.inbox)?
            .iter()
            .filter_map(|message| message.sequence)
            .max()
            .unwrap_or_default()
            + 1;
        message.sequence = Some(next);
        if message.delivered_at_ms.is_none() {
            message.delivered_at_ms = Some(now_ms());
        }
        Self::append(&self.inbox, &message)?;
        Ok(next)
    }

    /// Record what this session sent, in its own outbox.
    pub fn record_sent(&self, message: &SwarmMessage) -> Result<(), String> {
        Self::append(&self.outbox, message)?;
        Ok(())
    }

    /// New messages since `after_sequence`, the drain cursor. The cursor is
    /// the caller's (kept in memory by the turn loop; a session that
    /// restarts replays unread mail, which is the honest behaviour).
    pub fn unread_since(&self, after_sequence: u64) -> Result<Vec<SwarmMessage>, String> {
        Ok(Self::read(&self.inbox)?
            .into_iter()
            .filter(|message| message.sequence.unwrap_or_default() > after_sequence)
            .collect())
    }

    /// Stamp `read_at_ms` on every message up to and including `through`.
    /// Rewrites the inbox with atomic staging; the read stamp is the only
    /// field that changes.
    pub fn mark_read_through(&self, through: u64) -> Result<usize, String> {
        let messages = Self::read(&self.inbox)?;
        let mut marked = 0;
        let mut updated = Vec::with_capacity(messages.len());
        for mut message in messages {
            if message.sequence.is_some_and(|sequence| sequence <= through)
                && message.read_at_ms.is_none()
            {
                message.read_at_ms = Some(now_ms());
                marked += 1;
            }
            updated.push(message);
        }
        if marked == 0 {
            return Ok(0);
        }
        self.rewrite(&updated)?;
        Ok(marked)
    }

    /// Replace the inbox's contents, staged-then-renamed. Used only by the
    /// read-stamp path; delivery itself never rewrites.
    fn rewrite(&self, messages: &[SwarmMessage]) -> Result<(), String> {
        let staged = self.inbox.with_extension("jsonl.tmp");
        let mut file = File::create(&staged).map_err(|error| {
            format!(
                "the staged mailbox at {} could not be written: {error}",
                staged.display()
            )
        })?;
        for message in messages {
            let mut line = serde_json::to_string(message)
                .map_err(|error| format!("the message could not be rendered: {error}"))?;
            line.push('\n');
            file.write_all(line.as_bytes()).map_err(|error| {
                format!(
                    "the staged mailbox at {} could not be appended: {error}",
                    staged.display()
                )
            })?;
        }
        file.flush().map_err(|error| {
            format!(
                "the staged mailbox at {} could not be flushed: {error}",
                staged.display()
            )
        })?;
        std::fs::rename(&staged, &self.inbox).map_err(|error| {
            format!(
                "the mailbox at {} could not be moved into place: {error}",
                self.inbox.display()
            )
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sending
// ---------------------------------------------------------------------------

/// What one send did: one `DeliveryReport` per resolved recipient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendReport {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub thread: Option<String>,
    pub message_id: String,
    pub deliveries: Vec<DeliveryReport>,
    pub undeliverable: Vec<Undeliverable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Undeliverable {
    pub to: String,
    pub why: String,
}

/// Compose one message and deliver it everywhere `to` resolves to.
///
/// Fan-out targets resolve from the registration list at send time, so a
/// child that exited between the fan-out composing and its turn delivering
/// is reported `undeliverable` — honestly, by name — rather than silently
/// skipped.
pub fn send(
    home: &Path,
    from: &str,
    from_directory: &Path,
    to: &str,
    kind: &str,
    thread: Option<&str>,
    reply_expected: bool,
    body: &str,
) -> Result<SendReport, String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err("the message body is empty, and an empty message is not a message".to_string());
    }
    if trimmed.len() > MAXIMUM_BODY_BYTES {
        return Err(format!(
            "the message body is {} bytes and the most one delivery accepts is {MAXIMUM_BODY_BYTES}. \
             Say less, or write the long form to a file and send the path.",
            trimmed.len()
        ));
    }
    let known_kinds = ["question", "answer", "status", "handoff", "broadcast"];
    if !known_kinds.contains(&kind) {
        return Err(format!(
            "`{kind}` is not a message kind this swarm carries. Use one of {}.",
            known_kinds.join(", ")
        ));
    }
    if from == to {
        return Err("a session cannot send a message to itself".to_string());
    }

    let at_ms = now_ms();
    let id = message_id(from, to, thread, trimmed, at_ms);
    let sender_mailbox = Mailbox::at(from_directory);

    // `all` and `role:children-of:` resolve against the live registrations.
    let registrations = list(home)?;
    let targets: Vec<String> = if to == "all" {
        registrations
            .iter()
            .filter(|registration| registration.session_id != from)
            .map(|registration| registration.session_id.clone())
            .collect()
    } else if let Some(parent) = to.strip_prefix("role:children-of:") {
        registrations
            .iter()
            .filter(|registration| {
                registration.role == "child"
                    && registration.parent.as_deref() == Some(parent)
                    && registration.session_id != from
            })
            .map(|registration| registration.session_id.clone())
            .collect()
    } else {
        vec![to.to_string()]
    };
    if targets.is_empty() {
        return Err(match to {
            "all" => {
                "no other session is registered, so there is nobody to broadcast to".to_string()
            }
            other => {
                if other.starts_with("role:children-of:") {
                    format!("no live child of {other} is registered")
                } else {
                    format!("no session `{other}` is registered")
                }
            }
        });
    }
    // A direct send names one recipient, and the sender named it because it
    // meant that session. Silence where a session was expected is a refusal,
    // not an empty success.
    if targets.len() == 1 {
        let only = load_registration(home, &targets[0])?;
        if only.is_none() {
            return Err(format!("no session `{}` is registered", targets[0]));
        }
    }

    let mut deliveries = Vec::new();
    let mut undeliverable = Vec::new();
    for target in &targets {
        let registration = match load_registration(home, target)? {
            Some(registration) => registration,
            None => {
                undeliverable.push(Undeliverable {
                    to: target.clone(),
                    why: "no registration".to_string(),
                });
                continue;
            }
        };
        if !registration.deliverable() {
            undeliverable.push(Undeliverable {
                to: target.clone(),
                why: format!("session is {}", registration.state().as_str()),
            });
            continue;
        }
        let mailbox = Mailbox::at(
            Path::new(&registration.inbox)
                .parent()
                .unwrap_or(Path::new(".")),
        );
        let message = SwarmMessage {
            schema: MESSAGE_SCHEMA.to_string(),
            id: id.clone(),
            sequence: None,
            from: from.to_string(),
            to: target.clone(),
            thread: thread.map(str::to_string),
            kind: kind.to_string(),
            reply_expected: reply_expected.then_some(true),
            body: trimmed.to_string(),
            created_at_ms: at_ms,
            delivered_at_ms: None,
            read_at_ms: None,
        };
        match mailbox.deliver(message) {
            Ok(sequence) => deliveries.push(DeliveryReport {
                to: target.clone(),
                state: SwarmState::Live.as_str().to_string(),
                message_id: id.clone(),
                sequence,
            }),
            Err(why) => undeliverable.push(Undeliverable {
                to: target.clone(),
                why,
            }),
        }
    }

    let report = SendReport {
        from: from.to_string(),
        to: to.to_string(),
        kind: kind.to_string(),
        thread: thread.map(str::to_string),
        message_id: id,
        deliveries,
        undeliverable,
    };
    // The outbox record is best-effort relative to the deliveries: the
    // deliveries are the contract, and a read-only outbox must not fail a
    // send that delivered.
    let _ = sender_mailbox.record_sent(&SwarmMessage {
        schema: MESSAGE_SCHEMA.to_string(),
        id: report.message_id.clone(),
        sequence: None,
        from: from.to_string(),
        to: to.to_string(),
        thread: thread.map(str::to_string),
        kind: kind.to_string(),
        reply_expected: reply_expected.then_some(true),
        body: trimmed.to_string(),
        created_at_ms: at_ms,
        delivered_at_ms: None,
        read_at_ms: None,
    });
    Ok(report)
}

/// Read one session's inbox. Refuses an inbox whose sequence numbers gap:
/// the file is the transport of record, and a silent gap is a lost message
/// pretending to be a quiet day.
pub fn read_inbox(session_directory: &Path) -> Result<Vec<SwarmMessage>, String> {
    let messages = Mailbox::at(session_directory).deliver_count_guard()?;
    let mut expected = 0u64;
    for message in &messages {
        if let Some(sequence) = message.sequence {
            if sequence != expected + 1 {
                return Err(format!(
                    "the inbox at {} gaps at sequence {expected}: a message is missing, and \
                     reading past the gap would pretend it never existed",
                    session_directory.join(INBOX_FILE).display()
                ));
            }
            expected = sequence;
        }
    }
    Ok(messages)
}

impl Mailbox {
    fn deliver_count_guard(&self) -> Result<Vec<SwarmMessage>, String> {
        Self::read(&self.inbox)
    }

    /// Every inbox message, without the gap guard. For reporting a finished
    /// exchange, where the reader wants what is there, not a refusal about a
    /// line a crashed sender owed.
    pub fn messages(&self) -> Result<Vec<SwarmMessage>, String> {
        Self::read(&self.inbox)
    }
}
