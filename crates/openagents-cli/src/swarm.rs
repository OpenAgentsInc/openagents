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
//! Delivery itself stays cheap and dumb. Budgets, the reply-depth cap, the
//! per-sender mute list, and the per-turn drain cap live in [`SwarmPolicy`]
//! and [`plan_drain`]: the turn loop and the `swarm_*` tools ask before they
//! send or inject, and a refusal names the cap it hit.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const REGISTRATION_SCHEMA: &str = "openagents.swarm.registration.v1";
pub const MESSAGE_SCHEMA: &str = "openagents.swarm.message.v2";
pub const LISTING_SCHEMA: &str = "openagents.swarm.listing.v1";
pub const MUTE_SCHEMA: &str = "openagents.swarm.mute.v1";
pub const INBOX_TOOL: &str = "swarm.inbox";

const INBOX_FILE: &str = "inbox.jsonl";
const OUTBOX_FILE: &str = "outbox.jsonl";
const MUTE_FILE: &str = "swarm-mute.json";
const SWARM_DIR: &str = "swarm";

/// How many unread messages one turn will inject. The rest stay unread for
/// the next turn; they are never dropped.
pub const DEFAULT_DRAIN_CAP: usize = 8;

/// How many messages one session may send in any rolling hour.
pub const DEFAULT_HOURLY_BUDGET: usize = 60;

/// How deep a `reply_expected` chain may go. Default 2: ask, answer, stop.
pub const DEFAULT_REPLY_DEPTH_CAP: u32 = 2;

/// One hour, in milliseconds, for the send-budget window.
pub const HOUR_MS: u128 = 60 * 60 * 1000;

/// The largest message body one delivery accepts, in bytes. The ingest routes
/// cap trace documents; this is the same discipline for mail, sized for
/// meaningful answers rather than transcripts. A body that will not fit is
/// refused at send time, not truncated in flight — a truncated answer is a
/// wrong answer wearing a smaller envelope.
/// The most bytes one message — body plus any structured payload — accepts.
/// The envelope's total, so a payload cannot smuggle an oversized message in
/// under a body-only check (#286).
pub const MAXIMUM_BODY_BYTES: usize = 256 * 1024;

/// How fresh a registration's heartbeat must be, in milliseconds, before the
/// session is reported `Live` rather than `Stale`. Generous on purpose: a
/// session mid-long-turn is alive even when it has nothing to say for
/// minutes, and a heartbeat is refreshed on turn boundaries by the frame
/// loop, not on a timer thread.
pub const DEFAULT_ALIVE_AFTER_MS: u128 = 30 * 60 * 1000;

/// A structured payload beside the prose `body`: one content-type tag plus
/// the raw payload (#286). Handoffs carry diffs, file lists, and parameters
/// as data the recipient reads verbatim, instead of prose it must re-parse.
/// Transport and trajectory handling are identical to the body: one JSON
/// line, the same cap, the same receipts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuredPayload {
    /// An IANA-style content type, e.g. `application/json`, `text/x-diff`.
    pub content_type: String,
    /// The payload itself, verbatim. Interpreted by the recipient, never by
    /// the transport.
    pub payload: String,
}

impl StructuredPayload {
    /// Size in bytes on the wire: both fields, counted as sent.
    pub fn byte_size(&self) -> usize {
        self.content_type.len() + self.payload.len()
    }
}

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
    /// One line on what this session is doing, for discovery at a glance.
    /// Written by the session itself from its own checkpoint notes — the
    /// first sentence, truncated — never by a neighbor (#281).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
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
            // and belongs to someone this process may signal. Pid 0 is
            // excluded first: POSIX reads it as "every process in the
            // caller's group", so a zero pid would otherwise answer alive
            // forever by asking about unrelated processes.
            self.pid > 0 && unsafe { libc::kill(self.pid as i32, 0) == 0 }
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
    ///
    /// Two signals, and the module doc has always promised both: the pid
    /// must exist, and the heartbeat must be inside the alive window. Either
    /// alone lies. A pid alone lies twice — the process may have exited and
    /// the pid been recycled to something that is not this session (a macOS
    /// launchd daemon was found standing in for a dead Coder session), and a
    /// session wedged before its first turn never proves itself by existing.
    /// A heartbeat alone lies the other way: an open session sitting idle
    /// between turns stops beating but is not gone. Requiring both means a
    /// recycled pid reads stale once its heartbeat ages out, which is the
    /// drift this check exists to catch (#339).
    ///
    /// Historical note for readers of old registration files: sessions that
    /// predate the per-turn heartbeat in `drain_swarm_inbox` hold
    /// `heartbeat_at_ms` from their registration moment only. They read
    /// stale — correctly, because nothing has been able to confirm them
    /// alive since.
    pub fn state(&self) -> SwarmState {
        if self.process_alive() && self.heartbeat_fresh() {
            SwarmState::Live
        } else {
            SwarmState::Stale
        }
    }

    /// Deliverable: the process is alive, or the session is stale but still
    /// registered — mail to a stale session queues in its inbox file, which
    /// survives the process (#283). A session that is slow is not a session
    /// that is gone, and async work left for an offline agent is the reason
    /// swarms exist. Only a session with no registration at all is
    /// undeliverable.
    pub fn deliverable(&self) -> bool {
        true
    }

    /// Whether this session was stale at the given moment, so the recipient
    /// can tell queued-from-offline mail from live conversation and the
    /// sender's report can carry the same flag.
    pub fn stale_at(&self) -> bool {
        self.state() == SwarmState::Stale
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
    /// How many `reply_expected` hops this message sits from the start of
    /// its chain. Stamped at send time so a recipient can refuse the next
    /// hop without walking the whole thread. Absent on a message that does
    /// not ask for a reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_depth: Option<u32>,
    /// A structured payload beside the prose body, when the sender attached
    /// one (#286). Absent on plain messages; v1 readers that do not know the
    /// field ignore it and keep the body intact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<StructuredPayload>,
    pub body: String,
    pub created_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at_ms: Option<u128>,
    /// True when this message was delivered while its recipient was stale:
    /// queued mail (#283). The recipient sees the flag beside the arrival
    /// timestamp, so waking mail is never mistaken for a live exchange.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_when_queued: Option<bool>,
}

/// Where a delivered message lands and what it cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryReport {
    pub to: String,
    /// `live` for a same-moment delivery, `stale` for queued mail the
    /// recipient will see on its next live drain (#283).
    pub state: String,
    pub message_id: String,
    pub sequence: u64,
    /// True when the recipient's process was already gone at send time:
    /// the message is queued in the recipient's inbox file, not refused.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale_at_send: bool,
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

/// The swarm session id a `delegate` child registers under. The parent
/// addresses it as this id, or as the short handle `child-{n}`.
pub fn child_session_id(parent: &str, child_id: usize) -> String {
    format!("{parent}-child-{child_id}")
}

/// Where a session that is not a Coder store keeps its inbox and outbox:
/// `~/.openagents/swarm/mail/<session-id>/`. Delegate children live here so
/// their mail outlives the worktree and is not mixed with the code they
/// write.
pub fn mail_directory(home: &Path, session_id: &str) -> PathBuf {
    swarm_root(home).join("mail").join(session_id)
}

/// Rewrite a registration's pid. A delegate child is registered with the
/// parent's pid so it appears in the tree the moment it is spawned, then
/// this updates it to the child's own pid once that process exists, so a
/// killed child is stale rather than still looking like the parent.
pub fn set_pid(home: &Path, session_id: &str, pid: u32) -> Result<(), String> {
    let mut registration = match load_registration(home, session_id)? {
        Some(registration) => registration,
        None => return Err(format!("no session `{session_id}` is registered")),
    };
    registration.pid = pid;
    register(home, &registration)
}

/// Expand a parent-relative child handle (`child-1`, or `1`) into the
/// child's registered session id. Anything else is returned unchanged, so
/// `all` and `role:children-of:` keep their existing meaning.
pub fn expand_destination(home: &Path, from: &str, to: &str) -> String {
    let number = to
        .strip_prefix("child-")
        .or_else(|| {
            if !to.is_empty() && to.bytes().all(|byte| byte.is_ascii_digit()) {
                Some(to)
            } else {
                None
            }
        })
        .and_then(|digits| digits.parse::<usize>().ok());
    if let Some(number) = number {
        let candidate = child_session_id(from, number);
        if load_registration(home, &candidate).ok().flatten().is_some() {
            return candidate;
        }
    }
    to.to_string()
}

/// The message id for one send: `msg_` + 16 hex of SHA-256 over the fields
/// that make the message what it is — including any payload (#286), so an
/// id is still content-derived and a re-send with different data still
/// dedups apart.
fn message_id(
    from: &str,
    to: &str,
    thread: Option<&str>,
    body: &str,
    data: Option<&StructuredPayload>,
    at_ms: u128,
) -> String {
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
    if let Some(data) = data {
        hasher.update(data.content_type.as_bytes());
        hasher.update([0]);
        hasher.update(data.payload.as_bytes());
    }
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
/// Publish a status line onto an existing registration. A missing
/// registration is not created here — same law as [`heartbeat`]: only the
/// session's own startup registers.
pub fn set_status(home: &Path, session_id: &str, status: &str) -> Result<(), String> {
    let Some(mut registration) = load_registration(home, session_id)? else {
        return Ok(());
    };
    registration.status = Some(status.to_string());
    register(home, &registration)
}

/// Unregister a session: the file goes, so discovery stops offering it.
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
        let sequences: Vec<u64> = Self::read(&self.inbox)?
            .iter()
            .filter_map(|message| message.sequence)
            .filter(|sequence| *sequence <= through)
            .collect();
        self.mark_read_sequences(&sequences)
    }

    /// Stamp `read_at_ms` only on the named sequences, leaving everything
    /// else unread. The drain uses this so a muted sender between two
    /// injected messages stays unread.
    pub fn mark_read_sequences(&self, sequences: &[u64]) -> Result<usize, String> {
        if sequences.is_empty() {
            return Ok(0);
        }
        let wanted: BTreeSet<u64> = sequences.iter().copied().collect();
        let messages = Self::read(&self.inbox)?;
        let mut marked = 0;
        let mut updated = Vec::with_capacity(messages.len());
        for mut message in messages {
            if message
                .sequence
                .is_some_and(|sequence| wanted.contains(&sequence))
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

    /// The `reply_expected` depth of the message `thread` names, looking in
    /// this mailbox's inbox and outbox. Zero when there is no thread, or the
    /// named message carries no depth: the next send with `reply_expected`
    /// then starts (or continues) a chain at depth 1.
    pub fn thread_depth(&self, thread: Option<&str>) -> u32 {
        let Some(thread_id) = thread else {
            return 0;
        };
        let mut found = Vec::new();
        if let Ok(messages) = self.messages() {
            found.extend(messages);
        }
        if let Ok(messages) = Self::read(&self.outbox) {
            found.extend(messages);
        }
        found
            .iter()
            .find(|message| message.id == thread_id)
            .and_then(|message| message.reply_depth)
            .unwrap_or(0)
    }

    /// Messages that have arrived and have not been stamped read.
    pub fn unread(&self) -> Result<Vec<SwarmMessage>, String> {
        Ok(self
            .messages()?
            .into_iter()
            .filter(|message| message.read_at_ms.is_none())
            .collect())
    }

    /// What this session sent, in file order. Used by the fan-out report so
    /// an exchange is not only the mail the child received.
    pub fn sent(&self) -> Result<Vec<SwarmMessage>, String> {
        Self::read(&self.outbox)
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

/// What one send did: one `DeliveryReport` per resolved recipient, plus the
/// honest accounting the sender needs to pace itself — what the send cost
/// against the hourly budget, how deep a reply chain could still go, and
/// where every recipient actually landed (#282).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendReport {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub thread: Option<String>,
    pub message_id: String,
    /// What this send leaves of the rolling hourly budget: sends still
    /// available and when the window rolls open again.
    pub budget_remaining: BudgetRemaining,
    /// How many `reply_expected` hops remain on this thread before the cap
    /// refuses the next one. `None` when the send did not ask for a reply.
    pub reply_depth_remaining: Option<u32>,
    pub deliveries: Vec<DeliveryReport>,
    pub undeliverable: Vec<Undeliverable>,
}

/// The hourly send budget as it stands right after one send.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetRemaining {
    pub sends_left: usize,
    /// When the rolling hour opens a fresh window, in epoch milliseconds.
    pub resets_at_ms: u128,
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
    data: Option<&StructuredPayload>,
    sender_policy: Option<&mut SwarmPolicy>,
) -> Result<SendReport, String> {
    let trimmed = body.trim();
    let data_bytes = data.map(|payload| payload.byte_size()).unwrap_or(0);
    if trimmed.is_empty() && data.is_none() {
        return Err("the message body is empty, and an empty message is not a message".to_string());
    }
    if trimmed.len() + data_bytes > MAXIMUM_BODY_BYTES {
        return Err(format!(
            "the message is {} bytes (body plus any payload) and the most one delivery accepts is {MAXIMUM_BODY_BYTES}. \
             Say less, or write the long form to a file and send the path.",
            trimmed.len() + data_bytes
        ));
    }
    let known_kinds = ["question", "answer", "status", "handoff", "broadcast"];
    if !known_kinds.contains(&kind) {
        return Err(format!(
            "`{kind}` is not a message kind this swarm carries. Use one of {}.",
            known_kinds.join(", ")
        ));
    }
    let to = expand_destination(home, from, to);
    if from == to {
        return Err("a session cannot send a message to itself".to_string());
    }

    let at_ms = now_ms();
    let id = message_id(from, &to, thread, trimmed, data, at_ms);
    let sender_mailbox = Mailbox::at(from_directory);
    let reply_depth = if reply_expected {
        Some(sender_mailbox.thread_depth(thread).saturating_add(1))
    } else {
        None
    };

    // `all` and `role:children-of:` resolve against the live registrations.
    // Fan-out means sessions that can answer: a broadcast that queues into
    // forty stale inboxes wakes nobody and buries the next live drain under
    // stale-queue flags (#339).
    let registrations: Vec<_> = list(home)?
        .into_iter()
        .filter(|registration| registration.state() == SwarmState::Live)
        .collect();
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
        return Err(match to.as_str() {
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
        // A stale but registered session receives queued mail (#283): the
        // inbox file survives the process, so delivery is an append that
        // waits for the recipient's next live drain. The report and the
        // message both flag it, so nobody mistakes queued mail for a live
        // exchange. Only a missing registration is undeliverable. With the
        // live-only fan-out above (#339) this flag is now only reachable on
        // direct sends to a session that went stale between listing and
        // delivery — exactly the race the flag was built for.
        let stale_at_send = registration.stale_at();
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
            reply_depth,
            data: data.cloned(),
            body: trimmed.to_string(),
            created_at_ms: at_ms,
            delivered_at_ms: None,
            read_at_ms: None,
            stale_when_queued: stale_at_send.then_some(true),
        };
        match mailbox.deliver(message) {
            Ok(sequence) => deliveries.push(DeliveryReport {
                to: target.clone(),
                state: if stale_at_send {
                    SwarmState::Stale.as_str().to_string()
                } else {
                    SwarmState::Live.as_str().to_string()
                },
                message_id: id.clone(),
                sequence,
                stale_at_send,
            }),
            Err(why) => undeliverable.push(Undeliverable {
                to: target.clone(),
                why,
            }),
        }
    }

    // The honest accounting rides the report when the caller can share its
    // policy; library callers without one get a zeroed budget rather than a
    // wrong number. `admit_send` already swept and pushed (or was never
    // called), so this view is the post-send state.
    let (budget_remaining, reply_depth_remaining) = {
        let mut policy = sender_policy;
        let budget = policy
            .as_deref_mut()
            .map(|policy| policy.budget_remaining(at_ms))
            .unwrap_or(BudgetRemaining {
                sends_left: 0,
                resets_at_ms: at_ms,
            });
        let depth = match policy.as_deref() {
            Some(policy) if reply_expected => {
                Some(policy.reply_depth_remaining(reply_depth.unwrap_or_default()))
            }
            _ => None,
        };
        (budget, depth)
    };
    let report = SendReport {
        from: from.to_string(),
        to: to.to_string(),
        kind: kind.to_string(),
        thread: thread.map(str::to_string),
        message_id: id,
        budget_remaining,
        reply_depth_remaining,
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
        reply_depth,
        data: data.cloned(),
        body: trimmed.to_string(),
        created_at_ms: at_ms,
        delivered_at_ms: None,
        read_at_ms: None,
        stale_when_queued: None,
    });
    Ok(report)
}

/// How a gap check came out. An intact inbox carries no quarantine; a gapped
/// one carries the readable prefix, the first message after the gap, and the
/// sequence that went missing (#280).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantineSplit {
    /// The messages before the gap, contiguous from sequence 1.
    pub readable: Vec<SwarmMessage>,
    /// The first message after the gap — lost territory, not delivered mail.
    /// Present whenever a gap was found.
    pub first_after_gap: Option<SwarmMessage>,
    /// The last contiguous sequence before the gap: the last good sequence.
    pub last_good_sequence: u64,
}

impl QuarantineSplit {
    pub fn gap(&self) -> bool {
        self.first_after_gap.is_some()
    }
}

/// Split an inbox at its first sequence gap: everything from sequence 1
/// contiguous is readable, the message sitting after the missing sequence is
/// quarantined. Nothing here rewrites the file — the split is what the
/// quarantine notice names and what `repair` acts on.
fn quarantine_split(messages: &[SwarmMessage]) -> QuarantineSplit {
    let mut expected = 0u64;
    for (index, message) in messages.iter().enumerate() {
        if let Some(sequence) = message.sequence {
            if sequence != expected + 1 {
                return QuarantineSplit {
                    readable: messages[..index].to_vec(),
                    first_after_gap: Some(message.clone()),
                    last_good_sequence: expected,
                };
            }
            expected = sequence;
        }
    }
    QuarantineSplit {
        readable: messages.to_vec(),
        first_after_gap: None,
        last_good_sequence: expected,
    }
}

/// Read one session's inbox, refusing a gap the way the contract always has.
/// The refusal names the missing sequence and the repair path;
/// [`inbox_quarantine`] is the form that keeps reading instead.
pub fn read_inbox(session_directory: &Path) -> Result<Vec<SwarmMessage>, String> {
    let messages = Mailbox::at(session_directory).messages()?;
    let split = quarantine_split(&messages);
    if split.gap() {
        return Err(format!(
            "the inbox at {} gaps at sequence {}: a message is missing, and \
             reading past the gap would pretend it never existed — \
             `openagents swarm inbox repair` truncates to the last good sequence",
            session_directory.join(INBOX_FILE).display(),
            split.last_good_sequence
        ));
    }
    Ok(messages)
}

/// What a reader is told when an inbox has a gap (#280).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuarantineNotice {
    /// The last contiguous sequence; the next number is the one missing.
    pub missing_after_sequence: u64,
    /// The id of the first message stranded past the gap.
    pub first_quarantined_id: String,
    /// Who sent it, so the human can decide whether to ask for a re-send.
    pub first_quarantined_from: String,
    /// How many messages total sit past the gap.
    pub quarantined_count: usize,
    /// The exact repair path, so the refusal is never the end of the story.
    pub repair_hint: String,
}

/// The tool-facing read: everything readable plus a quarantine notice when a
/// gap exists (#280). A lost line is reported, never papered over — and it
/// no longer bricks every read: the readable prefix keeps flowing, the
/// notice names the gap, and nothing after it is silently dropped.
pub fn inbox_quarantine(
    session_directory: &Path,
) -> Result<(Vec<SwarmMessage>, Option<QuarantineNotice>), String> {
    let messages = Mailbox::at(session_directory).messages()?;
    let split = quarantine_split(&messages);
    if !split.gap() {
        return Ok((messages, None));
    }
    let after = split.first_after_gap.as_ref().unwrap();
    let notice = QuarantineNotice {
        missing_after_sequence: split.last_good_sequence,
        first_quarantined_id: after.id.clone(),
        first_quarantined_from: after.from.clone(),
        quarantined_count: messages.len() - split.readable.len(),
        repair_hint: format!(
            "`openagents swarm inbox repair` truncates this inbox to sequence {} \
             (the tail is preserved at inbox-quarantined.jsonl)",
            split.last_good_sequence
        ),
    };
    Ok((split.readable, Some(notice)))
}

/// Repair a gapped inbox by truncating it to the last contiguous sequence:
/// everything before the gap is kept, the orphaned tail is preserved at
/// `inbox-quarantined.jsonl` beside the mailbox (#280). Refuses an intact
/// inbox: a repair that "succeeded" on healthy mail is a bug with a receipt.
pub fn repair_inbox(session_directory: &Path) -> Result<RepairReport, String> {
    let mailbox = Mailbox::at(session_directory);
    let messages = Mailbox::read(&mailbox.inbox)?;
    let split = quarantine_split(&messages);
    if !split.gap() {
        return Err(
            "this inbox has no gap, so there is nothing to repair: refusing to rewrite healthy mail"
                .to_string(),
        );
    }
    let tail: Vec<SwarmMessage> = messages[split.readable.len()..].to_vec();
    let quarantine_path = session_directory.join("inbox-quarantined.jsonl");
    let mut preserved = String::new();
    for message in &tail {
        let mut line = serde_json::to_string(message)
            .map_err(|error| format!("the message could not be rendered: {error}"))?;
        line.push('\n');
        preserved.push_str(&line);
    }
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&quarantine_path)
        .map_err(|error| {
            format!(
                "the quarantine file at {} could not be opened: {error}",
                quarantine_path.display()
            )
        })?;
    file.write_all(preserved.as_bytes()).map_err(|error| {
        format!(
            "the quarantined tail could not be preserved at {}: {error}",
            quarantine_path.display()
        )
    })?;
    file.flush().map_err(|error| {
        format!(
            "the quarantine file at {} could not be flushed: {error}",
            quarantine_path.display()
        )
    })?;
    mailbox.rewrite(&split.readable)?;
    Ok(RepairReport {
        truncated_after_sequence: split.last_good_sequence,
        quarantined_count: tail.len(),
        preserved_at: quarantine_path.display().to_string(),
    })
}

/// What one repair did, for the receipt the confirmation gate prints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepairReport {
    pub truncated_after_sequence: u64,
    pub quarantined_count: usize,
    pub preserved_at: String,
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

// ---------------------------------------------------------------------------
// Drain, budgets, mute — slice 3
// ---------------------------------------------------------------------------

/// The home directory the swarm lives under. Registrations land at
/// `<home>/.openagents/swarm`. Tests pass a temporary home instead.
pub fn default_home() -> PathBuf {
    crate::auth::home_directory()
}

/// The session-store directory that holds a registration's inbox.
pub fn inbox_directory(registration: &Registration) -> PathBuf {
    Path::new(&registration.inbox)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

/// Whether `name` is a swarm tool or the synthetic drain entry. Used by the
/// TUI and the ATIF exporter so swarm traffic is never drawn or exported as
/// user speech.
pub fn is_swarm_tool(name: &str) -> bool {
    matches!(
        name,
        "swarm_list" | "swarm_send" | "swarm_inbox" | "swarm.inbox"
    )
}

/// What one drain pass will inject, skip, and leave for later.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainPlan {
    /// Unread, unmuted messages, in file order, up to the cap.
    pub inject: Vec<SwarmMessage>,
    /// Unread messages from muted senders. They stay unread.
    pub muted: Vec<SwarmMessage>,
    /// Unread unmuted messages past the cap. They stay unread for the next
    /// turn; they are never dropped.
    pub deferred: usize,
    /// Set when the inbox has a sequence gap (#280): the readable prefix
    /// flowed, the gap is reported, nothing past it is silently dropped.
    pub quarantine: Option<QuarantineNotice>,
}

impl DrainPlan {
    /// Ids this drain stamped read, in inject order. Autopilot's primary
    /// mail receipt (#310): the set is exactly what the boundary injected.
    pub fn consumed_ids(&self) -> Vec<String> {
        self.inject
            .iter()
            .map(|message| message.id.clone())
            .collect()
    }
}

/// Decide what this turn injects. `messages` is the inbox in file order;
/// already-read lines are ignored. Muted senders do not consume the cap.
pub fn plan_drain(messages: &[SwarmMessage], muted: &BTreeSet<String>, cap: usize) -> DrainPlan {
    let mut plan = DrainPlan::default();
    for message in messages {
        if message.read_at_ms.is_some() {
            continue;
        }
        if muted.contains(&message.from) {
            plan.muted.push(message.clone());
            continue;
        }
        if plan.inject.len() >= cap {
            plan.deferred += 1;
            continue;
        }
        plan.inject.push(message.clone());
    }
    plan
}

/// The live anti-livelock knobs for one session: hourly send budget, reply
/// chain cap, drain cap. Mute is a file beside the inbox so a slash command
/// and the drain path share it without sharing a lock.
#[derive(Debug, Clone)]
pub struct SwarmPolicy {
    pub hourly_budget: usize,
    pub reply_depth_cap: u32,
    pub drain_cap: usize,
    send_times: Vec<u128>,
}

impl Default for SwarmPolicy {
    fn default() -> Self {
        Self {
            hourly_budget: DEFAULT_HOURLY_BUDGET,
            reply_depth_cap: DEFAULT_REPLY_DEPTH_CAP,
            drain_cap: DEFAULT_DRAIN_CAP,
            send_times: Vec::new(),
        }
    }
}

impl SwarmPolicy {
    /// Admit one send, or refuse naming the cap that stopped it.
    ///
    /// `parent_depth` is the `reply_depth` of the message this one answers,
    /// or 0 when this send starts a chain. Returns the depth this send will
    /// carry when `reply_expected` is set.
    pub fn admit_send(
        &mut self,
        reply_expected: bool,
        parent_depth: u32,
        now: u128,
    ) -> Result<u32, String> {
        self.send_times
            .retain(|sent| now.saturating_sub(*sent) < HOUR_MS);
        if self.send_times.len() >= self.hourly_budget {
            return Err(format!(
                "this session has already sent {} messages in the last hour, which is the \
                 per-session budget. Wait, or mute the neighbor that is filling the hour.",
                self.hourly_budget
            ));
        }
        let depth = if reply_expected {
            parent_depth.saturating_add(1)
        } else {
            0
        };
        if reply_expected && depth > self.reply_depth_cap {
            return Err(format!(
                "the reply-expected chain would reach depth {depth}, and the cap is {}. \
                 The cap exists so two agents cannot livelock each other.",
                self.reply_depth_cap
            ));
        }
        self.send_times.push(now);
        Ok(depth)
    }

    /// The budget as it stands right after `admit_send` recorded this send:
    /// the sends still open in the rolling window and when the oldest falls
    /// out of it. Observational only, but it does share the window sweep —
    /// call it after `admit_send`, never instead of it.
    pub fn budget_remaining(&mut self, now: u128) -> BudgetRemaining {
        self.send_times
            .retain(|sent| now.saturating_sub(*sent) < HOUR_MS);
        BudgetRemaining {
            sends_left: self.hourly_budget.saturating_sub(self.send_times.len()),
            resets_at_ms: self
                .send_times
                .first()
                .map(|oldest| oldest.saturating_add(HOUR_MS))
                .unwrap_or(now),
        }
    }

    /// The depth a `reply_expected` reply on this thread could still reach
    /// before the cap refuses it.
    pub fn reply_depth_remaining(&self, thread_depth: u32) -> u32 {
        self.reply_depth_cap.saturating_sub(thread_depth)
    }
}

/// The identity a running session uses to send, drain, and mute. Shared
/// between the tool registry and the turn loop so a `swarm_send` and a
/// turn-boundary drain see the same budget.
#[derive(Debug, Clone)]
pub struct SwarmBinding {
    pub home: PathBuf,
    pub session_id: String,
    pub session_directory: PathBuf,
    pub policy: Arc<Mutex<SwarmPolicy>>,
}

impl SwarmBinding {
    pub fn new(home: PathBuf, session_id: impl Into<String>, session_directory: PathBuf) -> Self {
        Self {
            home,
            session_id: session_id.into(),
            session_directory,
            policy: Arc::new(Mutex::new(SwarmPolicy::default())),
        }
    }

    fn policy(&self) -> std::sync::MutexGuard<'_, SwarmPolicy> {
        self.policy
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Senders this session will not inject, file plus any in-memory extras.
    pub fn muted(&self) -> BTreeSet<String> {
        load_mute_list(&self.session_directory)
    }

    pub fn mute(&self, session: &str) -> Result<(), String> {
        let mut muted = load_mute_list(&self.session_directory);
        muted.insert(session.to_string());
        save_mute_list(&self.session_directory, &muted)
    }

    pub fn unmute(&self, session: &str) -> Result<(), String> {
        let mut muted = load_mute_list(&self.session_directory);
        muted.remove(session);
        save_mute_list(&self.session_directory, &muted)
    }
}

/// The mute list stored beside this session's inbox.
pub fn load_mute_list(session_directory: &Path) -> BTreeSet<String> {
    let path = session_directory.join(MUTE_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return BTreeSet::new(),
    };
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    value
        .get("muted")
        .and_then(|muted| muted.as_array())
        .map(|muted| {
            muted
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Replace the mute list. Atomic: staged beside, renamed in.
pub fn save_mute_list(session_directory: &Path, muted: &BTreeSet<String>) -> Result<(), String> {
    std::fs::create_dir_all(session_directory).map_err(|error| {
        format!(
            "the session directory at {} could not be created: {error}",
            session_directory.display()
        )
    })?;
    let path = session_directory.join(MUTE_FILE);
    let staged = session_directory.join(format!(".{MUTE_FILE}.tmp"));
    let document = serde_json::json!({
        "schema": MUTE_SCHEMA,
        "muted": muted.iter().collect::<Vec<_>>(),
    });
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("the mute list could not be rendered: {error}"))?;
    std::fs::write(&staged, text).map_err(|error| {
        format!(
            "the staged mute list at {} could not be written: {error}",
            staged.display()
        )
    })?;
    std::fs::rename(&staged, &path).map_err(|error| {
        format!(
            "the mute list at {} could not be moved into place: {error}",
            path.display()
        )
    })?;
    Ok(())
}

/// Registrations, optionally kept to one cwd or one parent tree.
pub fn list_filtered(
    home: &Path,
    cwd: Option<&str>,
    tree: Option<&str>,
) -> Result<Vec<Registration>, String> {
    let mut found = list(home)?;
    if let Some(cwd) = cwd.map(str::trim).filter(|value| !value.is_empty()) {
        found.retain(|registration| {
            registration.cwd == cwd
                || registration
                    .cwd
                    .starts_with(&format!("{cwd}{}", std::path::MAIN_SEPARATOR))
        });
    }
    if let Some(root) = tree.map(str::trim).filter(|value| !value.is_empty()) {
        let ids: BTreeSet<String> = found.iter().map(|r| r.session_id.clone()).collect();
        if !ids.contains(root) {
            return Err(format!("no session `{root}` is registered"));
        }
        let mut keep: BTreeSet<String> = BTreeSet::new();
        keep.insert(root.to_string());
        let mut grew = true;
        while grew {
            grew = false;
            for registration in &found {
                if let Some(parent) = &registration.parent
                    && keep.contains(parent)
                    && keep.insert(registration.session_id.clone())
                {
                    grew = true;
                }
            }
        }
        found.retain(|registration| keep.contains(&registration.session_id));
    }
    Ok(found)
}

/// A public-safe JSON object for one message, used as a tool result.
pub fn message_document(message: &SwarmMessage) -> serde_json::Value {
    let mut document = serde_json::json!({
        "id": message.id,
        "sequence": message.sequence,
        "from": message.from,
        "to": message.to,
        "kind": message.kind,
        "thread": message.thread,
        "reply_expected": message.reply_expected,
        "reply_depth": message.reply_depth,
        "body": message.body,
        "delivered_at_ms": message.delivered_at_ms,
        "read_at_ms": message.read_at_ms,
    });
    if let Some(data) = &message.data {
        document["data"] = serde_json::json!({
            "content_type": data.content_type,
            "payload": data.payload,
        });
    }
    if let Some(stale) = message.stale_when_queued {
        document["stale_when_queued"] = serde_json::json!(stale);
    }
    document
}

/// The filters one inbox read may apply before a drain or a peek. `sender`,
/// `kind`, and `thread` narrow independently; `unread_only` widens a peek to
/// the whole inbox when false, so thread reconstruction can read history
/// without walking the session record. A drain never re-injects read mail,
/// so a drain refuses `unread_only: false` instead of pretending to honor it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxFilter {
    pub sender: Option<String>,
    pub kind: Option<String>,
    pub thread: Option<String>,
    pub unread_only: bool,
}

impl InboxFilter {
    /// The unfiltered read: everything unread, which is what every inbox
    /// read meant before filters existed.
    pub fn unread() -> Self {
        Self {
            sender: None,
            kind: None,
            thread: None,
            unread_only: true,
        }
    }

    /// Whether one message passes the sender and kind filters. Thread
    /// selection needs the whole message list (a reply names its immediate
    /// parent, not the root, so a chain is a closure over `thread` links);
    /// it lives in [`Self::select`]. `unread_only` is also not applied here:
    /// the drain path reads only unread mail to begin with, and the peek
    /// path applies it when widening or not.
    fn matches(&self, message: &SwarmMessage) -> bool {
        if let Some(sender) = &self.sender
            && &message.from != sender
        {
            return false;
        }
        if let Some(kind) = &self.kind
            && &message.kind != kind
        {
            return false;
        }
        true
    }

    /// The messages passing every filter, in file order. When a `thread`
    /// filter is set, the selection is the named message and every message
    /// whose `thread` links close on it — the whole chain, whatever its
    /// depth — because thread reconstruction is the reason the filter
    /// exists. The named message itself is included: a thread filter that
    /// omits the opening message answers a question nobody asked.
    pub fn select<'a>(&self, messages: &'a [SwarmMessage]) -> Vec<&'a SwarmMessage> {
        let thread_set: Option<BTreeSet<String>> = self.thread.as_ref().map(|root| {
            let mut set: BTreeSet<String> = BTreeSet::new();
            set.insert(root.clone());
            let mut grew = true;
            while grew {
                grew = false;
                for message in messages {
                    if let Some(thread) = &message.thread
                        && set.contains(thread)
                        && set.insert(message.id.clone())
                    {
                        grew = true;
                    }
                }
            }
            set
        });
        messages
            .iter()
            .filter(|message| {
                if let Some(set) = &thread_set
                    && !set.contains(&message.id)
                {
                    return false;
                }
                self.matches(message)
            })
            .collect()
    }
}

/// Drain this session's inbox for one turn: inject up to the cap, skip muted
/// senders (leaving them unread), defer the rest. The caller is responsible
/// for putting `plan.inject` on the tool stream — never as user speech.
pub fn drain_turn(binding: &SwarmBinding) -> Result<DrainPlan, String> {
    drain_turn_filtered(binding, &InboxFilter::unread())
}

/// The drain over a filtered inbox: the filter narrows the unread candidates
/// before the cap, so a filtered drain stamps only what it returns and the
/// deferred count is the honest size of the rest of the match. A drain never
/// re-injects read mail, so `unread_only: false` is refused here — a peek
/// with the same filter is the way to read history.
pub fn drain_turn_filtered(
    binding: &SwarmBinding,
    filter: &InboxFilter,
) -> Result<DrainPlan, String> {
    if !filter.unread_only {
        return Err(
            "a drain only ever injects unread mail, so `unread_only: false` has no effect \
             there: use a peek with the same filter to read history"
                .to_string(),
        );
    }
    let mailbox = Mailbox::at(&binding.session_directory);
    let muted = binding.muted();
    let cap = binding.policy().drain_cap;
    // Quarantine-aware read (#280): a gapped inbox drains what is readable
    // and reports the gap, instead of refusing every read forever.
    let (readable, quarantine) = inbox_quarantine(&binding.session_directory)?;
    let unread: Vec<SwarmMessage> = readable
        .into_iter()
        .filter(|message| message.read_at_ms.is_none())
        .collect();
    let selected: Vec<SwarmMessage> = filter.select(&unread).into_iter().cloned().collect();
    let mut plan = plan_drain(&selected, &muted, cap);
    plan.quarantine = quarantine;
    let sequences: Vec<u64> = plan
        .inject
        .iter()
        .filter_map(|message| message.sequence)
        .collect();
    mailbox.mark_read_sequences(&sequences)?;
    Ok(plan)
}

/// Drain exactly the messages named by id: stamp them read, return them in
/// file order, leave everything else untouched. The precise instrument next
/// to the capped drain — a batch holding a `reply_expected` message can take
/// that one message and leave the rest for turns that can answer them.
///
/// All or nothing: one unknown id refuses the whole call before anything is
/// stamped, because a drain that silently skipped a name would return a
/// success carrying an unread message the caller believes was read. An
/// already-read id is an idempotent no-op — the message is returned
/// unchanged, its first read stamp intact. An id from a muted sender still
/// drains: the mute quiets a sender's stream, not a message the caller
/// named by id.
pub fn drain_by_ids(binding: &SwarmBinding, ids: &[String]) -> Result<Vec<SwarmMessage>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mailbox = Mailbox::at(&binding.session_directory);
    let messages = mailbox.messages()?;
    let mut sequences = Vec::with_capacity(ids.len());
    for id in ids {
        let message = messages
            .iter()
            .find(|message| &message.id == id)
            .ok_or_else(|| {
                format!(
                    "no message `{id}` exists in this inbox, so nothing was drained: stamping \
                 the rest would pretend a message was read"
                )
            })?;
        sequences.push(
            message
                .sequence
                .expect("a delivered message carries its inbox sequence"),
        );
    }
    // `mark_read_sequences` stamps only unread lines, so an already-read id
    // keeps its first stamp — the idempotent no-op the contract promises.
    // The messages are re-read after the stamp so the returned copies carry
    // the read stamp this drain itself just wrote; the pre-stamp snapshots
    // would answer "unread" about a drain that happened.
    mailbox.mark_read_sequences(&sequences)?;
    let stamped = mailbox.messages()?;
    Ok(sequences
        .iter()
        .filter_map(|sequence| {
            stamped
                .iter()
                .find(|message| message.sequence == Some(*sequence))
        })
        .cloned()
        .collect())
}

/// How long one [`wait_filtered`] may park before returning empty. The
/// issue's cap: a wait is a bounded pause at the turn boundary, not a
/// subscription — anything longer belongs to the next turn's drain.
pub const MAXIMUM_WAIT_MS: u128 = 60 * 1000;

/// The wait poll cadence. The mailbox is an append-only file, not a channel,
/// so waiting is a bounded poll; 250 ms is fast against a 60 s ceiling and
/// the poll itself is a directory read, cheaper than the model turn the wait
/// exists to save.
pub const WAIT_POLL_MS: u64 = 250;

/// Park until a message matching `filter` arrives or `timeout_ms` expires,
/// then report what is visible — the same shape as a peek, and **never** a
/// stamp: a `reply_expected` message surfaced by a wait is not committed to
/// an answer until a real drain stamps it (#287). A wait that returned a
/// read-stamped reply would quietly spend the recipient's turn on an
/// obligation it never saw the human approve.
pub fn wait_filtered(
    binding: &SwarmBinding,
    filter: &InboxFilter,
    timeout_ms: u64,
) -> Result<WaitOutcome, String> {
    if timeout_ms == 0 {
        return Err(
            "a wait of zero time is a peek, not a wait: give a timeout or use the inbox"
                .to_string(),
        );
    }
    if timeout_ms as u128 > MAXIMUM_WAIT_MS {
        return Err(format!(
            "`timeout_seconds` may park at most {MAXIMUM_WAIT_MS} ms; {timeout_ms} was asked for"
        ));
    }
    let started = std::time::Instant::now();
    let deadline = started + std::time::Duration::from_millis(timeout_ms);
    let mailbox = Mailbox::at(&binding.session_directory);
    loop {
        let messages = mailbox.messages()?;
        let muted = binding.muted();
        let visible: Vec<SwarmMessage> = filter
            .select(&messages)
            .into_iter()
            .filter(|message| !muted.contains(&message.from))
            .filter(|message| !filter.unread_only || message.read_at_ms.is_none())
            .cloned()
            .collect();
        if !visible.is_empty() {
            return Ok(WaitOutcome {
                matched: true,
                elapsed_ms: started.elapsed().as_millis(),
                messages: visible,
            });
        }
        if std::time::Instant::now() >= deadline {
            return Ok(WaitOutcome {
                matched: false,
                elapsed_ms: started.elapsed().as_millis(),
                messages: Vec::new(),
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(
            WAIT_POLL_MS.min(
                deadline
                    .saturating_duration_since(std::time::Instant::now())
                    .as_millis() as u64,
            ),
        ));
    }
}

/// What one wait returned: whether a match ended it early, how long it
/// actually parked, and what was visible when it did. `messages` is empty
/// when the timeout expired.
#[derive(Debug)]
pub struct WaitOutcome {
    pub matched: bool,
    pub elapsed_ms: u128,
    pub messages: Vec<SwarmMessage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registration whose pid answers and whose heartbeat is current —
    /// the every-signal-good shape.
    fn live_registration() -> Registration {
        Registration {
            schema: REGISTRATION_SCHEMA.to_string(),
            session_id: "1astatetest0000000000".to_string(),
            pid: std::process::id(),
            cwd: "/tmp".to_string(),
            lane: "flash".to_string(),
            model: None,
            role: "root".to_string(),
            parent: None,
            worktree: None,
            status: None,
            inbox: "/tmp/inbox.jsonl".to_string(),
            alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
            started_at_ms: now_ms(),
            heartbeat_at_ms: now_ms(),
        }
    }

    /// Liveness needs both signals (#339). A live pid with a fresh heartbeat
    /// is the only Live shape; each signal failing alone must read stale.
    #[test]
    fn state_requires_both_pid_and_heartbeat() {
        let mut registration = live_registration();
        assert_eq!(registration.state(), SwarmState::Live);

        // Heartbeat aged out while the pid still answers: the pid was
        // recycled or the registration was copied, and neither is alive.
        registration.heartbeat_at_ms = registration.heartbeat_at_ms.saturating_sub(
            registration.alive_after_ms + 1,
        );
        assert_eq!(registration.state(), SwarmState::Stale);

        // Fresh heartbeat from a dead pid: nothing is home.
        let mut dead = live_registration();
        dead.pid = 0;
        assert!(!dead.process_alive(), "pid 0 must not answer signal 0");
        assert_eq!(dead.state(), SwarmState::Stale);
    }

    fn unread(from: &str, sequence: u64, body: &str) -> SwarmMessage {
        SwarmMessage {
            schema: MESSAGE_SCHEMA.to_string(),
            id: format!("msg_{sequence}"),
            sequence: Some(sequence),
            from: from.to_string(),
            to: "here".to_string(),
            thread: None,
            kind: "status".to_string(),
            reply_expected: None,
            reply_depth: None,
            data: None,
            body: body.to_string(),
            created_at_ms: sequence as u128,
            delivered_at_ms: Some(sequence as u128),
            read_at_ms: None,
            stale_when_queued: None,
        }
    }

    #[test]
    fn swarm_drain_skips_muted_senders_without_consuming_the_cap() {
        let messages = vec![
            unread("noisy", 1, "ignore me"),
            unread("friend", 2, "hello"),
            unread("friend", 3, "again"),
        ];
        let muted = BTreeSet::from(["noisy".to_string()]);
        let plan = plan_drain(&messages, &muted, 1);
        assert_eq!(plan.inject.len(), 1);
        assert_eq!(plan.inject[0].from, "friend");
        assert_eq!(plan.muted.len(), 1);
        assert_eq!(plan.deferred, 1, "the second friend message waits");
    }

    #[test]
    fn swarm_drain_defers_overflow_and_never_drops_it() {
        let messages = (1..=5).map(|n| unread("a", n, "x")).collect::<Vec<_>>();
        let plan = plan_drain(&messages, &BTreeSet::new(), 2);
        assert_eq!(plan.inject.len(), 2);
        assert_eq!(plan.deferred, 3);
        assert!(plan.muted.is_empty());
    }

    #[test]
    fn swarm_already_read_messages_are_not_injected_again() {
        let mut message = unread("a", 1, "old");
        message.read_at_ms = Some(1);
        let plan = plan_drain(&[message, unread("a", 2, "new")], &BTreeSet::new(), 8);
        assert_eq!(plan.inject.len(), 1);
        assert_eq!(plan.inject[0].sequence, Some(2));
    }

    #[test]
    fn swarm_a_payload_becomes_part_of_the_content_derived_id() {
        let base = ("session-a", "session-b", None, "report", now_ms());
        let plain = message_id(base.0, base.1, base.2, base.3, None, base.4);
        let with_data = message_id(
            base.0,
            base.1,
            base.2,
            base.3,
            Some(&StructuredPayload {
                content_type: "application/json".to_string(),
                payload: "{\"a\":1}".to_string(),
            }),
            base.4,
        );
        assert_ne!(plain, with_data, "payload changes the content id");
        let again = message_id(
            base.0,
            base.1,
            base.2,
            base.3,
            Some(&StructuredPayload {
                content_type: "application/json".to_string(),
                payload: "{\"a\":1}".to_string(),
            }),
            base.4,
        );
        assert_eq!(with_data, again, "same content, same id");
    }

    #[test]
    fn swarm_payload_byte_size_counts_both_fields() {
        let payload = StructuredPayload {
            content_type: "text/x-diff".to_string(),
            payload: "+-one line-".to_string(),
        };
        assert_eq!(
            payload.byte_size(),
            "text/x-diff".len() + "+-one line-".len()
        );
    }

    #[test]
    fn swarm_reply_depth_cap_names_the_cap() {
        let mut policy = SwarmPolicy {
            reply_depth_cap: 2,
            hourly_budget: 10,
            ..SwarmPolicy::default()
        };
        assert_eq!(policy.admit_send(true, 0, 1).unwrap(), 1);
        assert_eq!(policy.admit_send(true, 1, 2).unwrap(), 2);
        let why = policy.admit_send(true, 2, 3).unwrap_err();
        assert!(why.contains("cap is 2"), "{why}");
        assert!(why.contains("depth 3"), "{why}");
    }

    #[test]
    fn swarm_hourly_budget_refuses_the_next_send() {
        let mut policy = SwarmPolicy {
            hourly_budget: 1,
            ..SwarmPolicy::default()
        };
        policy.admit_send(false, 0, 1_000).unwrap();
        let why = policy.admit_send(false, 0, 2_000).unwrap_err();
        assert!(why.contains("per-session budget"), "{why}");
        // A send after the window rolls is admitted.
        policy.admit_send(false, 0, 1_000 + HOUR_MS).unwrap();
    }

    #[test]
    fn a_parent_addresses_a_child_by_short_handle() {
        assert_eq!(child_session_id("sess-a", 2), "sess-a-child-2");
        let home = tempfile::tempdir().unwrap();
        let path = home.path().to_path_buf();
        let dir = tempfile::tempdir().unwrap();
        let child = Registration {
            schema: REGISTRATION_SCHEMA.to_string(),
            session_id: child_session_id("sess-a", 1),
            pid: std::process::id(),
            cwd: "/work".to_string(),
            lane: "flash".to_string(),
            model: None,
            role: "child".to_string(),
            parent: Some("sess-a".to_string()),
            worktree: Some("/work/child".to_string()),
            status: None,
            inbox: dir.path().join("inbox.jsonl").display().to_string(),
            alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
            started_at_ms: now_ms(),
            heartbeat_at_ms: now_ms(),
        };
        register(&path, &child).unwrap();
        assert_eq!(
            expand_destination(&path, "sess-a", "child-1"),
            child.session_id
        );
        assert_eq!(expand_destination(&path, "sess-a", "1"), child.session_id);
        assert_eq!(
            expand_destination(&path, "sess-a", "role:children-of:sess-a"),
            "role:children-of:sess-a"
        );
        // An unknown handle stays itself so send can refuse by name.
        assert_eq!(expand_destination(&path, "sess-a", "child-9"), "child-9");
    }
    #[test]
    fn swarm_wait_returns_a_match_without_stamping_it() {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let dir = tempfile::tempdir().unwrap();
        register(
            &home_path,
            &Registration {
                schema: REGISTRATION_SCHEMA.to_string(),
                session_id: "waiter".to_string(),
                pid: std::process::id(),
                cwd: "/work".to_string(),
                lane: "flash".to_string(),
                model: None,
                role: "root".to_string(),
                parent: None,
                worktree: None,
                status: None,
                inbox: dir.path().join("inbox.jsonl").display().to_string(),
                alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
                started_at_ms: now_ms(),
                heartbeat_at_ms: now_ms(),
            },
        )
        .unwrap();
        let binding = SwarmBinding::new(home_path.clone(), "waiter", dir.path().to_path_buf());

        // Nothing in the box: a short wait expires honestly.
        let outcome = wait_filtered(&binding, &InboxFilter::unread(), 600).unwrap();
        assert!(!outcome.matched);
        assert!(outcome.messages.is_empty());
        assert!(
            outcome.elapsed_ms >= 500,
            "waited the timeout: {}",
            outcome.elapsed_ms
        );

        // A sender delivers mid-wait: the wait returns early with the
        // message visible, still unread.
        let sender_dir = tempfile::tempdir().unwrap();
        let sender_home = home_path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = register(
                &sender_home,
                &Registration {
                    schema: REGISTRATION_SCHEMA.to_string(),
                    session_id: "sender".to_string(),
                    pid: std::process::id(),
                    cwd: "/work".to_string(),
                    lane: "flash".to_string(),
                    model: None,
                    role: "root".to_string(),
                    parent: None,
                    worktree: None,
                    status: None,
                    inbox: sender_dir.path().join("inbox.jsonl").display().to_string(),
                    alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
                    started_at_ms: now_ms(),
                    heartbeat_at_ms: now_ms(),
                },
            );
            send(
                &sender_home,
                "sender",
                sender_dir.path(),
                "waiter",
                "question",
                None,
                true,
                "are you there?",
                None,
                None,
            )
        });
        let outcome = wait_filtered(&binding, &InboxFilter::unread(), 10_000).unwrap();
        writer.join().unwrap().unwrap();
        assert!(outcome.matched, "the mid-wait delivery ended the wait");
        assert!(
            outcome.elapsed_ms < 5_000,
            "returned early: {}",
            outcome.elapsed_ms
        );
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].from, "sender");
        assert!(
            outcome.messages[0].read_at_ms.is_none(),
            "a wait never stamps"
        );
        assert_eq!(outcome.messages[0].reply_expected, Some(true));

        // Still unread after the wait: the next drain owns it.
        let unread = Mailbox::at(dir.path()).unread().unwrap();
        assert_eq!(unread.len(), 1, "the wait left the message unread");
    }

    #[test]
    fn swarm_wait_honors_filters_and_never_blocks_past_the_ceiling() {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().to_path_buf();
        let dir = tempfile::tempdir().unwrap();
        register(
            &home_path,
            &Registration {
                schema: REGISTRATION_SCHEMA.to_string(),
                session_id: "waiter".to_string(),
                pid: std::process::id(),
                cwd: "/work".to_string(),
                lane: "flash".to_string(),
                model: None,
                role: "root".to_string(),
                parent: None,
                worktree: None,
                status: None,
                inbox: dir.path().join("inbox.jsonl").display().to_string(),
                alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
                started_at_ms: now_ms(),
                heartbeat_at_ms: now_ms(),
            },
        )
        .unwrap();
        let binding = SwarmBinding::new(home_path.clone(), "waiter", dir.path().to_path_buf());
        let sender_dir = tempfile::tempdir().unwrap();
        register(
            &home_path,
            &Registration {
                schema: REGISTRATION_SCHEMA.to_string(),
                session_id: "sender".to_string(),
                pid: std::process::id(),
                cwd: "/work".to_string(),
                lane: "flash".to_string(),
                model: None,
                role: "root".to_string(),
                parent: None,
                worktree: None,
                status: None,
                inbox: sender_dir.path().join("inbox.jsonl").display().to_string(),
                alive_after_ms: DEFAULT_ALIVE_AFTER_MS,
                started_at_ms: now_ms(),
                heartbeat_at_ms: now_ms(),
            },
        )
        .unwrap();

        // A message that does not match the filter leaves the wait waiting.
        send(
            &home_path,
            "sender",
            sender_dir.path(),
            "waiter",
            "status",
            None,
            false,
            "not the kind you asked for",
            None,
            None,
        )
        .unwrap();
        let outcome = wait_filtered(
            &binding,
            &InboxFilter {
                sender: None,
                kind: Some("question".to_string()),
                thread: None,
                unread_only: true,
            },
            600,
        )
        .unwrap();
        assert!(
            !outcome.matched,
            "a status does not satisfy a question wait"
        );

        // Zero and over-ceiling are refused by name.
        let why = wait_filtered(&binding, &InboxFilter::unread(), 0).unwrap_err();
        assert!(why.contains("zero"), "{why}");
        let why = wait_filtered(&binding, &InboxFilter::unread(), 61_000).unwrap_err();
        assert!(why.contains("timeout_seconds"), "{why}");
    }
}
