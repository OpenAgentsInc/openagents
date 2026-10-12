//! The agent channel (#11346): every subagent the main Coder delegates to
//! can talk to the others in the same session. `docs/coder/subagent-channel.md`
//! is the spec.
//!
//! One folder per session holds the whole channel:
//!
//! - `log.jsonl`: append-only. Every record is one line: a message, an
//!   intent, an overlap notice, a negotiation outcome, a lease event, or a
//!   member joining or finishing.
//! - `state.json`: members, current intents, leases and agreements, read
//!   and written only under an exclusive lock on `lock`.
//!
//! A file channel is the one shape every engine can reach: in-process
//! members (solvers, the swarm, the live model's `comms` tool) call this
//! module directly, and an external engine gets a short preamble naming the
//! folder and the command.

use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod cli;
pub use cli::command;

/// The plugin's ID in /plugins and in `bundled-plugins.json`.
pub const PLUGIN: &str = "comms";

/// The tool the live model calls to reach the channel.
pub const TOOL: &str = "comms";

/// The channel's append-only record file.
pub const LOG: &str = "log.jsonl";
/// The channel's current state: members, intents, leases, agreements.
pub const STATE: &str = "state.json";
/// The exclusive lock guarding `state.json`.
pub const LOCK: &str = "lock";
/// The folder under Coder's state root that holds one folder per session.
pub const ROOT: &str = "comms";

/// A message to everyone, or to one member, uses this address.
pub const ALL: &str = "all";

/// The scarce shared things a member leases, with a stable rank so several
/// leases are always taken in one global order. A member holding a lease may
/// not take one that ranks before it, so two members can never wait on each
/// other.
pub const LEASE_KINDS: [&str; 5] = [
    "build-folder",
    "build-host",
    "deploy-slot",
    "migration",
    "port",
];

/// How long a lease holds when it is not renewed, in seconds.
pub const LEASE_SECS: u64 = 600;

/// The longest one message may be, in bytes.
pub const MESSAGE_LIMIT: usize = 16 * 1024;

/// Whether `kind` is one of the lease kinds.
#[must_use]
pub fn lease_kind(kind: &str) -> bool {
    LEASE_KINDS.contains(&kind)
}

/// The rank of a lease kind in the global acquisition order; `None` when the
/// kind is unknown.
#[must_use]
pub fn kind_rank(kind: &str) -> Option<usize> {
    LEASE_KINDS.iter().position(|known| *known == kind)
}

/// A member of the channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub joined_at: u64,
    #[serde(default)]
    pub finished_at: Option<u64>,
}

/// What a member means to change: the files, crates and scarce resources it
/// names. Posting an intent is the member saying so out loud.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub member: String,
    #[serde(default)]
    pub files: Vec<String>,
    /// Files outside the checkout, such as a run folder's own artifacts.
    #[serde(default)]
    pub crates: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    pub at: u64,
}

impl Intent {
    /// The intent as the channel records it, with the files' crates derived.
    #[must_use]
    pub fn new(member: &str, files: Vec<String>, resources: Vec<String>) -> Self {
        let crates = crates_of(&files);
        Self {
            member: member.to_owned(),
            files: normalize(files),
            crates,
            resources: normalize(resources),
            at: now_ms(),
        }
    }
}

/// One overlap between two members' intents, and what they share.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlap {
    pub members: [String; 2],
    /// Human-readable shared things: `file PATH`, `folder PATH`, `crate N`,
    /// or `resource KIND:VALUE`.
    pub shared: Vec<String>,
}

/// How an overlap is settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// One member goes first; the other rebases on its branch.
    First,
    /// The shared file's change is divided; each side names its part.
    Split,
    /// Both changes go into one pull request.
    Merge,
}

impl Outcome {
    /// The outcome's word in the log and the state file.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Split => "split",
            Self::Merge => "merge",
        }
    }

    /// Reads an outcome's word.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "first" => Some(Self::First),
            "split" => Some(Self::Split),
            "merge" => Some(Self::Merge),
            _ => None,
        }
    }
}

/// A recorded agreement between two members about an overlap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agreement {
    pub members: [String; 2],
    pub outcome: Outcome,
    /// Who goes first for [`Outcome::First`], or the file split for
    /// [`Outcome::Split`], or the shared branch for [`Outcome::Merge`].
    #[serde(default)]
    pub detail: String,
    /// `who decided`: a member's name for a made agreement, or `swarm` when
    /// the fixed rule settled it.
    pub decided_by: String,
    pub why: String,
    pub at: u64,
}

/// A held lease on a scarce shared thing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    pub kind: String,
    pub value: String,
    pub holder: String,
    pub acquired_at: u64,
    pub expires_at: u64,
}

impl Lease {
    /// The lease's key in `state.json`, and its order in acquisition.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind, self.value)
    }

    /// The lease's rank in the global acquisition order.
    #[must_use]
    pub fn rank(&self) -> (usize, String) {
        (
            kind_rank(&self.kind).unwrap_or(LEASE_KINDS.len()),
            self.value.clone(),
        )
    }
}

/// One line of the channel log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub at: u64,
    pub kind: String,
    #[serde(flatten)]
    pub body: Value,
}

impl Record {
    /// A short line for a child chat's notice or a swarm view's row.
    #[must_use]
    pub fn line(&self) -> String {
        let text = |key: &str| self.body.get(key).and_then(Value::as_str).unwrap_or("");
        match self.kind.as_str() {
            "join" => format!("{} joined", text("member")),
            "finish" => format!("{} finished", text("member")),
            "message" => {
                let to = text("to");
                if to == ALL {
                    format!("{}: {}", text("from"), text("text"))
                } else {
                    format!("{} -> {}: {}", text("from"), to, text("text"))
                }
            }
            "intent" => format!(
                "{} intends {}",
                text("member"),
                summarise(&self.body, "files")
            ),
            "overlap" => format!(
                "overlap: {} and {} share {}",
                text("a"),
                text("b"),
                summarise(&self.body, "shared")
            ),
            "agreement" => format!(
                "{} and {} agreed {} ({})",
                text("a"),
                text("b"),
                text("outcome"),
                text("why")
            ),
            "lease" => format!(
                "{} {} {}:{}",
                text("event"),
                text("holder"),
                text("kind"),
                text("value")
            ),
            other => other.to_owned(),
        }
    }
}

/// A list of names as one bounded line.
fn summarise(body: &Value, key: &str) -> String {
    let items: Vec<String> = body
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if items.is_empty() {
        "(nothing)".into()
    } else {
        items.join(", ")
    }
}

/// The channel's state, as `state.json` holds it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct State {
    members: BTreeMap<String, Member>,
    intents: BTreeMap<String, Intent>,
    leases: BTreeMap<String, Lease>,
    agreements: Vec<Agreement>,
}

/// One session's agent channel, rooted at a folder on disk.
#[derive(Clone, Debug)]
pub struct Channel {
    dir: PathBuf,
}

/// Why a tool could not join a channel: it needs a session folder.
pub const NO_SESSION: &str = "The agent channel has no session folder. Open a chat first.";

/// The session folder name under `--state`, or the process's own when the
/// chat has no session id: two Coder processes then never share a channel.
#[must_use]
pub fn session_id(explicit: Option<&str>) -> String {
    if let Some(id) = explicit.filter(|id| !id.is_empty()) {
        return sanitize(id);
    }
    if let Ok(id) = std::env::var("CODER_SESSION") {
        if !id.is_empty() {
            return sanitize(&id);
        }
    }
    format!("pid-{}", std::process::id())
}

/// A name safe as one path segment.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .take(128)
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "session".into()
    } else {
        cleaned
    }
}

impl Channel {
    /// The channel for `session` under `root`, creating its folder.
    ///
    /// # Errors
    /// The folder cannot be created.
    pub fn open(root: &Path, session: &str) -> Result<Self, String> {
        let dir = root.join(ROOT).join(sanitize(session));
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("Cannot create {}: {error}", dir.display()))?;
        let channel = Self { dir };
        // Touch the lock so a reader never races its creation.
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(channel.lock_path())
            .map_err(|error| format!("Cannot open the channel lock: {error}"))?;
        Ok(channel)
    }

    /// The channel at exactly `dir`, which a `--channel DIR` names.
    ///
    /// # Errors
    /// The folder cannot be created.
    pub fn at(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("Cannot create {}: {error}", dir.display()))?;
        let channel = Self {
            dir: dir.to_path_buf(),
        };
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(channel.lock_path())
            .map_err(|error| format!("Cannot open the channel lock: {error}"))?;
        Ok(channel)
    }

    /// The folder the channel lives in: what a preamble and `--channel` name.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where an in-process member posts its own name, for the log.
    #[must_use]
    pub fn member_name(prefix: &str, id: &str) -> String {
        format!("{prefix}-{}", sanitize(id))
    }

    fn log_path(&self) -> PathBuf {
        self.dir.join(LOG)
    }
    fn state_path(&self) -> PathBuf {
        self.dir.join(STATE)
    }
    fn lock_path(&self) -> PathBuf {
        self.dir.join(LOCK)
    }

    /// Runs `edit` with `state.json` read and written under the exclusive
    /// lock, so two members never lose each other's changes.
    fn with_state<T>(
        &self,
        edit: impl FnOnce(&mut State) -> Result<T, String>,
    ) -> Result<T, String> {
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(self.lock_path())
            .map_err(|error| format!("Cannot open the channel lock: {error}"))?;
        lock.lock()
            .map_err(|error| format!("Cannot lock the channel: {error}"))?;
        let mut bytes = Vec::new();
        let mut state = match File::open(self.state_path()) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)
                    .map_err(|error| format!("Cannot read the channel state: {error}"))?;
                serde_json::from_slice(&bytes).unwrap_or_default()
            }
            Err(_) => State::default(),
        };
        let result = edit(&mut state)?;
        let bytes = serde_json::to_vec_pretty(&state).unwrap_or_default();
        let tmp = self.state_path().with_extension("tmp");
        std::fs::write(&tmp, &bytes)
            .and_then(|()| std::fs::rename(&tmp, self.state_path()))
            .map_err(|error| format!("Cannot write the channel state: {error}"))?;
        Ok(result)
    }

    fn append(&self, record: &Record) -> Result<(), String> {
        let mut line = serde_json::to_string(record).unwrap_or_default();
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())
            .map_err(|error| format!("Cannot open the channel log: {error}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        file.write_all(line.as_bytes())
            .map_err(|error| format!("Cannot write the channel log: {error}"))
    }

    /// Reads every record in the log, in order.
    ///
    /// # Errors
    /// The log cannot be read.
    pub fn log(&self) -> Result<Vec<Record>, String> {
        let mut text = String::new();
        match File::open(self.log_path()) {
            Ok(mut file) => {
                file.read_to_string(&mut text)
                    .map_err(|error| format!("Cannot read the channel log: {error}"))?;
            }
            Err(_) => return Ok(Vec::new()),
        }
        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }

    /// Reads the records a member has not seen: everything after `since`
    /// lines, kept when it is addressed to everyone or to `member`.
    ///
    /// # Errors
    /// The log cannot be read.
    pub fn read_for(&self, member: &str, since: usize) -> Result<(Vec<Record>, usize), String> {
        let records = self.log()?;
        let total = records.len();
        let kept = records
            .into_iter()
            .skip(since)
            .filter(|record| match record.kind.as_str() {
                "message" => {
                    let to = record.body.get("to").and_then(Value::as_str).unwrap_or(ALL);
                    to == ALL || to == member
                }
                // Notices and events are for everyone.
                _ => true,
            })
            .collect();
        Ok((kept, total))
    }

    /// A member joins, once. Joining again is not an error.
    ///
    /// # Errors
    /// The state cannot be written.
    pub fn join(&self, member: &str) -> Result<(), String> {
        let joined = self.with_state(|state| {
            if state.members.contains_key(member) {
                return Ok(false);
            }
            state.members.insert(
                member.to_owned(),
                Member {
                    name: member.to_owned(),
                    joined_at: now_ms(),
                    finished_at: None,
                },
            );
            Ok(true)
        })?;
        if joined {
            self.append(&Record {
                at: now_ms(),
                kind: "join".into(),
                body: json!({ "member": member }),
            })?;
        }
        Ok(())
    }

    /// A new member named `prefix-N`, N the lowest free number from one,
    /// joins and its name is returned: short and stable, never the task
    /// (#11467).
    ///
    /// # Errors
    /// The state cannot be written.
    pub fn join_numbered(&self, prefix: &str) -> Result<String, String> {
        let prefix = sanitize(prefix);
        let member = self.with_state(|state| {
            let member = (1..)
                .map(|n| format!("{prefix}-{n}"))
                .find(|name| !state.members.contains_key(name))
                .unwrap_or_default();
            state.members.insert(
                member.clone(),
                Member {
                    name: member.clone(),
                    joined_at: now_ms(),
                    finished_at: None,
                },
            );
            Ok(member)
        })?;
        self.append(&Record {
            at: now_ms(),
            kind: "join".into(),
            body: json!({ "member": member }),
        })?;
        Ok(member)
    }

    /// A member finishes: every lease it holds is released and it is marked
    /// finished, so no other member waits on it.
    ///
    /// # Errors
    /// The state cannot be written.
    pub fn finish(&self, member: &str) -> Result<Vec<Lease>, String> {
        let released = self.with_state(|state| {
            let held: Vec<Lease> = state
                .leases
                .values()
                .filter(|lease| lease.holder == member)
                .cloned()
                .collect();
            for lease in &held {
                state.leases.remove(&lease.key());
            }
            if let Some(entry) = state.members.get_mut(member) {
                entry.finished_at = Some(now_ms());
            } else {
                state.members.insert(
                    member.to_owned(),
                    Member {
                        name: member.to_owned(),
                        joined_at: now_ms(),
                        finished_at: Some(now_ms()),
                    },
                );
            }
            Ok(held)
        })?;
        for lease in &released {
            self.append(&Record {
                at: now_ms(),
                kind: "lease".into(),
                body: json!({ "event": "release", "holder": member, "kind": lease.kind, "value": lease.value }),
            })?;
        }
        self.append(&Record {
            at: now_ms(),
            kind: "finish".into(),
            body: json!({ "member": member }),
        })?;
        Ok(released)
    }

    /// Every member, joined or finished.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn members(&self) -> Result<Vec<Member>, String> {
        let state = self.read_state()?;
        Ok(state.members.into_values().collect())
    }

    fn read_state(&self) -> Result<State, String> {
        self.with_state(|_| Ok(()))?;
        let bytes = std::fs::read(self.state_path())
            .map_err(|error| format!("Cannot read state: {error}"))?;
        Ok(serde_json::from_slice(&bytes).unwrap_or_default())
    }

    /// Posts a message: to everyone ([`ALL`]) or to one member.
    ///
    /// # Errors
    /// The message is empty or too long, or the log cannot be written.
    pub fn post(&self, from: &str, to: &str, text: &str) -> Result<Record, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("A channel message cannot be empty.".into());
        }
        if text.len() > MESSAGE_LIMIT {
            return Err(format!(
                "A channel message is at most {} bytes.",
                MESSAGE_LIMIT
            ));
        }
        let to = if to.is_empty() { ALL } else { to };
        let record = Record {
            at: now_ms(),
            kind: "message".into(),
            body: json!({ "from": from, "to": to, "text": text }),
        };
        self.append(&record)?;
        Ok(record)
    }

    /// Posts a member's intent and returns the overlap notices it caused.
    /// Each overlap is written once, naming both members.
    ///
    /// # Errors
    /// The intent is empty, or the state or log cannot be written.
    pub fn intent(&self, intent: Intent) -> Result<Vec<Overlap>, String> {
        if intent.files.is_empty() && intent.resources.is_empty() {
            return Err("An intent names at least one file or resource.".into());
        }
        let overlaps = self.with_state(|state| {
            let mut overlaps = Vec::new();
            for other in state.intents.values() {
                if other.member == intent.member {
                    continue;
                }
                let shared = overlap(other, &intent);
                if !shared.is_empty() {
                    overlaps.push(Overlap {
                        members: [other.member.clone(), intent.member.clone()],
                        shared,
                    });
                }
            }
            state.intents.insert(intent.member.clone(), intent.clone());
            Ok(overlaps)
        })?;
        self.append(&Record {
            at: intent.at,
            kind: "intent".into(),
            body: json!({
                "member": intent.member,
                "files": intent.files,
                "crates": intent.crates,
                "resources": intent.resources,
            }),
        })?;
        for overlap in &overlaps {
            self.append(&Record {
                at: now_ms(),
                kind: "overlap".into(),
                body: json!({
                    "a": overlap.members[0],
                    "b": overlap.members[1],
                    "shared": overlap.shared,
                }),
            })?;
        }
        Ok(overlaps)
    }

    /// Records an agreement between two members and returns it.
    ///
    /// # Errors
    /// The log or state cannot be written.
    pub fn agree(&self, agreement: Agreement) -> Result<Agreement, String> {
        let mut agreement = agreement;
        agreement.at = now_ms();
        self.with_state(|state| {
            state.agreements.push(agreement.clone());
            Ok(())
        })?;
        self.append(&Record {
            at: agreement.at,
            kind: "agreement".into(),
            body: json!({
                "a": agreement.members[0],
                "b": agreement.members[1],
                "outcome": agreement.outcome.as_str(),
                "detail": agreement.detail,
                "decided_by": agreement.decided_by,
                "why": agreement.why,
            }),
        })?;
        Ok(agreement)
    }

    /// Settles an overlap by the fixed swarm rule: the member whose intent
    /// came first goes first, the other rebases. Returns the agreement.
    ///
    /// # Errors
    /// The state or log cannot be written.
    pub fn settle_first(&self, a: &str, b: &str) -> Result<Agreement, String> {
        let (first, second) = {
            let state = self.read_state()?;
            let at_a = state.intents.get(a).map(|intent| intent.at).unwrap_or(0);
            let at_b = state.intents.get(b).map(|intent| intent.at).unwrap_or(0);
            if at_b < at_a { (b, a) } else { (a, b) }
        };
        self.agree(Agreement {
            members: [a.to_owned(), b.to_owned()],
            outcome: Outcome::First,
            detail: first.to_owned(),
            decided_by: "swarm".into(),
            why: format!("{first}'s intent came first, so {second} rebases"),
            at: 0,
        })
    }

    /// Every overlap notice in the log.
    ///
    /// # Errors
    /// The log cannot be read.
    pub fn overlaps(&self) -> Result<Vec<Record>, String> {
        Ok(self
            .log()?
            .into_iter()
            .filter(|record| record.kind == "overlap")
            .collect())
    }

    /// Every agreement in the state.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn agreements(&self) -> Result<Vec<Agreement>, String> {
        Ok(self.read_state()?.agreements)
    }

    /// Expires leases whose time has passed, releasing them, and returns
    /// what expired.
    ///
    /// # Errors
    /// The state or log cannot be written.
    pub fn sweep(&self) -> Result<Vec<Lease>, String> {
        let expired = self.with_state(|state| {
            let now = now_ms();
            let dead: Vec<Lease> = state
                .leases
                .values()
                .filter(|lease| lease.expires_at <= now)
                .cloned()
                .collect();
            for lease in &dead {
                state.leases.remove(&lease.key());
            }
            Ok(dead)
        })?;
        for lease in &expired {
            self.append(&Record {
                at: now_ms(),
                kind: "lease".into(),
                body: json!({ "event": "expire", "holder": lease.holder, "kind": lease.kind, "value": lease.value }),
            })?;
        }
        Ok(expired)
    }

    /// Takes several leases in one call, in the fixed global order. A member
    /// holding a lease may not take one that ranks before it, so two members
    /// can never wait on each other; the offending lease is refused with who
    /// holds what and until when.
    ///
    /// # Errors
    /// A lease is held by another member, the order would deadlock, or the
    /// state or log cannot be written.
    pub fn acquire(&self, member: &str, wants: &[(String, String)]) -> Result<Vec<Lease>, String> {
        for (kind, _) in wants {
            if !lease_kind(kind) {
                return Err(format!(
                    "{kind} is not a lease kind. Use one of {}.",
                    LEASE_KINDS.join(", ")
                ));
            }
        }
        let mut sorted: Vec<(String, String)> = wants.to_vec();
        sorted.sort_by_key(|(kind, value)| {
            (kind_rank(kind).unwrap_or(LEASE_KINDS.len()), value.clone())
        });
        sorted.dedup();
        self.sweep()?;
        let taken = self.with_state(|state| {
            // The member's own lowest held rank bounds what it may take now.
            let lowest = state
                .leases
                .values()
                .filter(|lease| lease.holder == member)
                .map(Lease::rank)
                .min();
            for (kind, value) in &sorted {
                let next = (
                    kind_rank(kind).unwrap_or(LEASE_KINDS.len()),
                    value.clone(),
                );
                if let Some(lowest) = &lowest {
                    if next < *lowest {
                        return Err(format!(
                            "{member} already holds {}:{}; taking {kind}:{value} first would let two members wait on each other. Take them in the fixed order.",
                            state
                                .leases
                                .values()
                                .filter(|lease| lease.holder == member)
                                .min_by_key(|lease| lease.rank())
                                .map(|lease| format!("{}", lease.kind))
                                .unwrap_or_default(),
                            state
                                .leases
                                .values()
                                .filter(|lease| lease.holder == member)
                                .min_by_key(|lease| lease.rank())
                                .map(|lease| lease.value.clone())
                                .unwrap_or_default(),
                        ));
                    }
                }
                let key = format!("{kind}:{value}");
                if let Some(held) = state.leases.get(&key) {
                    if held.holder != member {
                        return Err(format!(
                            "{kind}:{value} is held by {} until {}.",
                            held.holder,
                            stamp(held.expires_at)
                        ));
                    }
                }
            }
            let now = now_ms();
            let leases: Vec<Lease> = sorted
                .iter()
                .map(|(kind, value)| Lease {
                    kind: kind.clone(),
                    value: value.clone(),
                    holder: member.to_owned(),
                    acquired_at: now,
                    expires_at: now + LEASE_SECS * 1000,
                })
                .collect();
            for lease in &leases {
                state.leases.insert(lease.key(), lease.clone());
            }
            Ok(leases)
        })?;
        for lease in &taken {
            self.log_lease("acquire", lease)?;
        }
        Ok(taken)
    }

    /// Renews a held lease so it does not expire.
    ///
    /// # Errors
    /// The member does not hold it, or the state or log cannot be written.
    pub fn renew(&self, member: &str, kind: &str, value: &str) -> Result<Lease, String> {
        let lease = self.with_state(|state| {
            let key = format!("{kind}:{value}");
            let Some(held) = state.leases.get_mut(&key) else {
                return Err(format!("{kind}:{value} is not held."));
            };
            if held.holder != member {
                return Err(format!("{kind}:{value} is held by {}.", held.holder));
            }
            held.expires_at = now_ms() + LEASE_SECS * 1000;
            Ok(held.clone())
        })?;
        self.log_lease("renew", &lease)?;
        Ok(lease)
    }

    /// Releases a lease the member holds.
    ///
    /// # Errors
    /// The member does not hold it, or the state or log cannot be written.
    pub fn release(&self, member: &str, kind: &str, value: &str) -> Result<(), String> {
        let lease = self.with_state(|state| {
            let key = format!("{kind}:{value}");
            match state.leases.get(&key) {
                Some(held) if held.holder == member => {
                    let lease = held.clone();
                    state.leases.remove(&key);
                    Ok(lease)
                }
                Some(held) => Err(format!("{kind}:{value} is held by {}.", held.holder)),
                None => Err(format!("{kind}:{value} is not held.")),
            }
        })?;
        self.log_lease("release", &lease)
    }

    fn log_lease(&self, event: &str, lease: &Lease) -> Result<(), String> {
        self.append(&Record {
            at: now_ms(),
            kind: "lease".into(),
            body: json!({
                "event": event,
                "holder": lease.holder,
                "kind": lease.kind,
                "value": lease.value,
                "expires_at": lease.expires_at,
            }),
        })
    }

    /// Every held lease.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn leases(&self) -> Result<Vec<Lease>, String> {
        let state = self.read_state()?;
        Ok(state.leases.into_values().collect())
    }

    /// The current intent of each member.
    ///
    /// # Errors
    /// The state cannot be read.
    pub fn intents(&self) -> Result<Vec<Intent>, String> {
        let state = self.read_state()?;
        Ok(state.intents.into_values().collect())
    }

    /// The whole channel as one JSON document, for the swarm view and the
    /// CLI's `read`.
    ///
    /// # Errors
    /// The state or log cannot be read.
    pub fn snapshot(&self) -> Result<Value, String> {
        let state = self.read_state()?;
        Ok(json!({
            "folder": self.dir.display().to_string(),
            "members": state.members.into_values().collect::<Vec<_>>(),
            "intents": state.intents.into_values().collect::<Vec<_>>(),
            "leases": state.leases.into_values().collect::<Vec<_>>(),
            "agreements": state.agreements,
        }))
    }
}

/// The two intents' shared files, folders, crates and resources.
#[must_use]
pub fn overlap(a: &Intent, b: &Intent) -> Vec<String> {
    let mut shared = Vec::new();
    for file in &a.files {
        for other in &b.files {
            if file == other {
                shared.push(format!("file {file}"));
            } else if inside(file, other) || inside(other, file) {
                shared.push(format!("folder {}", common_dir(file, other)));
            }
        }
    }
    for krate in &a.crates {
        if b.crates.contains(krate) {
            shared.push(format!("crate {krate}"));
        }
    }
    for resource in &a.resources {
        if b.resources.contains(resource) {
            shared.push(format!("resource {resource}"));
        }
    }
    shared.sort();
    shared.dedup();
    shared
}

/// Whether `path` is inside folder `dir`.
fn inside(path: &str, dir: &str) -> bool {
    let dir = dir.trim_end_matches('/');
    !dir.is_empty()
        && path.len() > dir.len() + 1
        && path.starts_with(dir)
        && path.as_bytes()[dir.len()] == b'/'
}

/// The shared leading folder of two paths.
fn common_dir(a: &str, b: &str) -> String {
    let mut dir = String::new();
    for (left, right) in a.split('/').zip(b.split('/')) {
        if left != right {
            break;
        }
        if !dir.is_empty() {
            dir.push('/');
        }
        dir.push_str(left);
    }
    if dir.is_empty() { "/".into() } else { dir }
}

/// The crates a list of files lives in: `crates/<name>/...` becomes `<name>`.
#[must_use]
pub fn crates_of(files: &[String]) -> Vec<String> {
    let mut crates: Vec<String> = files.iter().filter_map(|file| crate_of(file)).collect();
    crates.sort();
    crates.dedup();
    crates
}

/// The crate a path lives in, or `None`.
#[must_use]
pub fn crate_of(path: &str) -> Option<String> {
    let rest = path.strip_prefix("crates/")?;
    let name = rest.split('/').next()?;
    (!name.is_empty()).then(|| name.to_owned())
}

/// Sorted, de-duplicated, trimmed paths.
fn normalize(paths: Vec<String>) -> Vec<String> {
    let mut paths: Vec<String> = paths
        .into_iter()
        .map(|path| path.trim().trim_start_matches("./").to_owned())
        .filter(|path| !path.is_empty())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A lease's expiry as `HH:MM:SS` on this machine's clock, for a message
/// that tells a waiting member how long to wait.
#[must_use]
pub fn stamp(expires_at_ms: u64) -> String {
    let seconds = expires_at_ms / 1000;
    let day = seconds % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

/// The preamble an external engine gets on its task, naming the channel
/// folder, its member name and the command. Empty when the channel is off.
#[must_use]
pub fn preamble(channel: &Channel, member: &str, binary: &str) -> String {
    format!(
        "You are a member of the agent channel for this session, named {member}. Other subagents \
working at the same time can see what you post. Post what you are about to change and read what \
the others posted before you edit a shared file.\n\
\n\
- Post a message: `{binary} post --channel {dir} --as {member} --to all --text \"...\"`\n\
- Read what is new: `{binary} read --channel {dir} --as {member} --since N`\n\
- Say what you intend to change: `{binary} intent --channel {dir} --as {member} --file PATH [--file PATH] [--resource KIND:VALUE]`\n\
- Take a lease on a scarce thing: `{binary} lease acquire --channel {dir} --as {member} --kind KIND --value VALUE` (renew and release take the same flags)\n\
- Agree how to settle an overlap: `{binary} agree --channel {dir} --as {member} --with NAME --outcome first|split|merge --why \"...\"`\n\
\n\
Use exactly this command; it is the program that started you. If it fails, say so in your \
reply and carry on without the channel. Never write to the channel folder yourself.\n",
        dir = channel.dir().display(),
    )
}

/// The command a delegated agent runs to reach the channel: this very
/// program by its full path, as `PATH comms` for `coder` or `PATH coder
/// comms` for `openagents`, so the agent never meets another `coder` on its
/// `PATH` that has no `comms` (#11468). Without a known path, `coder comms`.
#[must_use]
pub fn invocation() -> String {
    let Ok(exe) = std::env::current_exe() else {
        return "coder comms".into();
    };
    invocation_for(&exe)
}

fn invocation_for(exe: &Path) -> String {
    let path = exe.display().to_string();
    let path = if path.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"') {
        format!("'{}'", path.replace('\'', "'\\''"))
    } else {
        path
    };
    let stem = exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem == "openagents" {
        format!("{path} coder comms")
    } else if stem == "coder" {
        format!("{path} comms")
    } else {
        // A test binary or another host: the installed `coder`.
        "coder comms".into()
    }
}

/// What the model is told about the `comms` tool.
#[must_use]
pub const fn instructions() -> &'static str {
    "The agent channel lets the subagents this chat delegates to talk to each other. Post what you intend to change before you edit a shared file; read what the others posted; take a lease on a scarce shared thing before you use it, renew it while you hold it, and release it when you finish. Use it whenever two or more delegations run at once.\n"
}

/// The tool's input schema.
#[must_use]
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["post", "read", "intent", "lease", "agree", "status"],
                "description": "What to do on the channel."
            },
            "to": {"type": "string", "description": "post: the member, or `all`."},
            "text": {"type": "string", "description": "post: the message."},
            "member": {"type": "string", "description": "The name to act as; defaults to this chat's own."},
            "since": {"type": "integer", "description": "read: the record count already seen."},
            "files": {"type": "array", "items": {"type": "string"}, "description": "intent: the files to change."},
            "resources": {"type": "array", "items": {"type": "string"}, "description": "intent/lease: `KIND:VALUE` scarce things."},
            "lease": {
                "type": "object",
                "description": "lease: acquire, renew or release.",
                "properties": {
                    "op": {"type": "string", "enum": ["acquire", "renew", "release"]},
                    "kind": {"type": "string", "enum": ["build-folder", "build-host", "deploy-slot", "migration", "port"]},
                    "value": {"type": "string"}
                }
            },
            "with": {"type": "string", "description": "agree: the other member."},
            "outcome": {"type": "string", "enum": ["first", "split", "merge"], "description": "agree: how the overlap is settled."},
            "why": {"type": "string", "description": "agree: the reason."}
        },
        "required": ["action"],
        "additionalProperties": false
    })
}

/// The tool's definition, in the chat-completions function shape.
#[must_use]
pub fn tool_definition() -> Value {
    json!({"type":"function","function":{
        "name": TOOL,
        "description": "Talk to the other subagents in this session: post a message, read what is new, say what you intend to change, take and release leases on scarce shared things, and agree how to settle an overlap.",
        "parameters": schema(),
    }})
}

/// Runs one `comms` tool call.
///
/// # Errors
/// The arguments are invalid, or the channel refuses the call.
pub fn execute(
    arguments: &Value,
    channel: &Channel,
    emit: &mut dyn FnMut(crate::bundled_runtime::RuntimeEvent),
) -> Result<Value, String> {
    use crate::bundled_runtime::RuntimeEvent;
    const USAGE: &str =
        "comms takes action and the fields that action needs, with no other fields.";
    let object = arguments.as_object().ok_or(USAGE)?;
    let member = |object: &serde_json::Map<String, Value>| {
        object
            .get("member")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| "coder".to_owned())
    };
    let text = |key: &str| object.get(key).and_then(Value::as_str).map(str::to_owned);
    let list = |key: &str| -> Vec<String> {
        object
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    };
    let action = text("action").ok_or(USAGE)?;
    match action.as_str() {
        "post" => {
            let from = member(object);
            let to = text("to").unwrap_or_else(|| ALL.into());
            let message = text("text").ok_or("comms post needs text.")?;
            let record = channel.post(&from, &to, &message)?;
            emit(RuntimeEvent::Text(format!("{}\n", record.line())));
            Ok(json!({"posted": record.body, "at": record.at}))
        }
        "read" => {
            let since = object.get("since").and_then(Value::as_u64).unwrap_or(0) as usize;
            let (records, cursor) = channel.read_for(&member(object), since)?;
            let lines: Vec<String> = records.iter().map(Record::line).collect();
            if !lines.is_empty() {
                emit(RuntimeEvent::Text(format!("{}\n", lines.join("\n"))));
            }
            Ok(json!({"since": cursor, "count": records.len(), "records": records}))
        }
        "intent" => {
            let from = member(object);
            let intent = Intent::new(&from, list("files"), list("resources"));
            let overlaps = channel.intent(intent)?;
            for overlap in &overlaps {
                emit(RuntimeEvent::Text(format!(
                    "overlap: {} and {} share {}\n",
                    overlap.members[0],
                    overlap.members[1],
                    overlap.shared.join(", ")
                )));
            }
            Ok(json!({"overlaps": overlaps}))
        }
        "lease" => {
            let lease = object
                .get("lease")
                .and_then(Value::as_object)
                .ok_or(USAGE)?;
            let kind = lease
                .get("kind")
                .and_then(Value::as_str)
                .ok_or("lease needs kind.")?;
            let value = lease
                .get("value")
                .and_then(Value::as_str)
                .ok_or("lease needs value.")?;
            let from = member(object);
            match lease.get("op").and_then(Value::as_str).unwrap_or("acquire") {
                "acquire" => {
                    let taken = channel.acquire(&from, &[(kind.to_owned(), value.to_owned())])?;
                    Ok(json!({"held": taken}))
                }
                "renew" => Ok(json!({"held": channel.renew(&from, kind, value)?})),
                "release" => {
                    channel.release(&from, kind, value)?;
                    Ok(json!({"released": format!("{kind}:{value}")}))
                }
                other => Err(format!(
                    "lease op takes acquire, renew or release, not {other}."
                )),
            }
        }
        "agree" => {
            let outcome = Outcome::parse(text("outcome").as_deref().unwrap_or_default())
                .ok_or("agree needs outcome first, split or merge.")?;
            let from = member(object);
            let other = text("with").ok_or("agree needs with.")?;
            let agreement = channel.agree(Agreement {
                members: [from.clone(), other],
                outcome,
                detail: text("detail").unwrap_or_default(),
                decided_by: from,
                why: text("why").unwrap_or_default(),
                at: 0,
            })?;
            emit(RuntimeEvent::Text(format!(
                "agreed {} between {} and {} ({})\n",
                agreement.outcome.as_str(),
                agreement.members[0],
                agreement.members[1],
                agreement.why
            )));
            Ok(json!({"agreement": agreement}))
        }
        "status" => Ok(channel.snapshot()?),
        other => Err(format!(
            "comms action takes post, read, intent, lease, agree or status, not {other}."
        )),
    }
}

#[cfg(test)]
mod tests;
