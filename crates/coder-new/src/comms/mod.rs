//! The agent channel (#11346): every subagent the main Coder delegates to
//! can talk to the others in the same session.
//!
//! A session's channel is a folder (`docs/coder/subagent-channel.md`):
//! `log.jsonl`, the append-only channel every member reads and writes, and
//! `state.json`, the members, intents, leases and agreements, changed only
//! under an exclusive lock on `lock`. A file channel is the one shape every
//! engine can reach: in-process members (solvers, the swarm, the live
//! model's `channel` tool, microclaude's tools) call [`Member`] directly,
//! and external engines (Codex, OpenCode, Cursor, Devin, Claude in print
//! mode, Microcoder) run `coder comms` ([`cli`]) as their task's preamble
//! tells them ([`Member::preamble`]), or append a JSON line to the log.
//!
//! - **Intents** ([`Member::intent`]): the files, crates and resources a
//!   member means to change. Each new intent is compared with every other
//!   member's ([`overlap`]); each overlap posts one notice both sides see.
//! - **Agreements** ([`Member::agree`]): how an overlap is settled — one
//!   goes first and the other rebases, a split, or a merge — recorded with
//!   who decided and why.
//! - **Leases** ([`Member::acquire`]): scarce shared things (the build
//!   folder, a deploy slot, a migration number, a port, a build host),
//!   taken with an expiry, renewed while held, released on finish, and
//!   dropped when they expire or their Coder process is gone. Several are
//!   taken in one call in a fixed order, and a holder can't take one that
//!   sorts before what it holds, so no two members wait on each other.
//!
//! The plugin ([`PLUGIN`]) is on by default; off, there is no channel and
//! delegations run isolated as before.

pub mod cli;
pub mod lease;
pub mod overlap;
pub mod swarm;
pub mod tools;
pub mod view;
pub mod watch;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use lease::{Lease, LeaseKind, LeaseRequest};
pub use overlap::{Intent, Overlap};

/// The plugin's ID in /plugins and in `bundled-plugins.json`.
pub const PLUGIN: &str = "comms";

/// Set on a nested Coder so it joins its parent's channel.
pub const CHANNEL_ENV: &str = "CODER_COMMS_CHANNEL";

/// The member a `coder comms` command speaks as, when `--as` is not given.
pub const MEMBER_ENV: &str = "CODER_COMMS_MEMBER";

/// The name the channel itself signs its notices with.
pub const SYSTEM: &str = "channel";

/// Everyone, as a message's addressee.
pub const ALL: &str = "all";

/// The most log lines one read returns.
const READ_MAX: usize = 2000;

/// Now, in Unix milliseconds.
#[must_use]
pub fn now_ms() -> u64 {
    coder_lease::now_ms()
}

/// What one log line records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Words from one member to everyone or to one member.
    #[default]
    Message,
    /// A member joined the session.
    Join,
    /// A member said what it means to change.
    Intent,
    /// Two members' intents share something.
    Overlap,
    /// An overlap was settled.
    Agreement,
    /// A lease was taken, renewed, released or expired.
    Lease,
    /// A member finished.
    Finish,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Join => "join",
            Self::Intent => "intent",
            Self::Overlap => "overlap",
            Self::Agreement => "agreement",
            Self::Lease => "lease",
            Self::Finish => "finish",
        }
    }
}

fn all() -> String {
    ALL.into()
}

/// One line of the channel. Every field has a default, so a line an
/// engine appends by hand (`{"from":..,"text":..}`) reads as a message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub at_ms: u64,
    #[serde(default)]
    pub from: String,
    #[serde(default = "all")]
    pub to: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub text: String,
    /// The members a notice is about, beside its addressee.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub concerns: Vec<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

impl Record {
    fn new(from: &str, kind: Kind, text: impl Into<String>) -> Self {
        Self {
            at_ms: now_ms(),
            from: from.to_owned(),
            to: ALL.into(),
            kind,
            text: text.into(),
            concerns: Vec::new(),
            data: Value::Null,
        }
    }

    /// Whether `member` should see it: a notice about members only when it
    /// is one of them, else addressed to everyone or to it; never its own words echoed back.
    #[must_use]
    pub fn concerns(&self, member: &str) -> bool {
        if self.kind == Kind::Join {
            return false;
        }
        if self.from == member && self.kind == Kind::Message {
            return false;
        }
        // A notice about some members is for them alone.
        if self.kind != Kind::Message && !self.concerns.is_empty() {
            return self.concerns.iter().any(|name| name == member);
        }
        self.to == ALL || self.to == member || self.concerns.iter().any(|name| name == member)
    }
}

/// A member of the session.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemberState {
    /// What kind of agent it is: `solver`, `codex`, `microcoder`, ...
    pub kind: String,
    pub joined_ms: u64,
    /// The Coder process it runs under; its leases go when that ends.
    #[serde(default)]
    pub pid: Option<u32>,
    /// How it ended, once it has.
    #[serde(default)]
    pub finished: Option<String>,
}

/// How an overlap was settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// One goes first; the other rebases on its branch after it lands.
    First,
    /// The shared change is divided.
    Split,
    /// Both changes go into one pull request.
    Merge,
}

impl Outcome {
    /// Reads `first`, `split` or `merge`.
    ///
    /// # Errors
    /// Any other word.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word {
            "first" => Ok(Self::First),
            "split" => Ok(Self::Split),
            "merge" => Ok(Self::Merge),
            _ => Err("An outcome is first, split or merge.".into()),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Split => "split",
            Self::Merge => "merge",
        }
    }
}

/// A settled overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Agreement {
    /// The two members, in the order agreed: for `first`, who goes first.
    pub order: Vec<String>,
    pub outcome: Outcome,
    /// The files, crates or resources it is about.
    pub on: Vec<String>,
    /// Who settled it: a member, or the swarm's rule.
    pub decided_by: String,
    pub note: String,
    pub at_ms: u64,
}

impl Agreement {
    /// The agreement as one plain sentence.
    #[must_use]
    pub fn sentence(&self) -> String {
        let on = if self.on.is_empty() {
            String::new()
        } else {
            format!(" on {}", self.on.join(", "))
        };
        let pair = self.order.join(" and ");
        let mut text = match (self.outcome, self.order.as_slice()) {
            (Outcome::First, [first, then, ..]) => {
                format!("{first} goes first{on}; {then} rebases on its branch after it lands.")
            }
            (Outcome::Split, _) => format!("{pair} split the change{on}."),
            (Outcome::Merge, [into, ..]) => {
                format!("{pair} merge their changes{on} into one pull request, {into}'s.")
            }
            _ => format!("{pair} agreed{on}."),
        };
        if !self.note.is_empty() {
            text.push(' ');
            text.push_str(&self.note);
        }
        text
    }

    fn pair(&self) -> BTreeSet<&str> {
        self.order.iter().map(String::as_str).collect()
    }
}

/// Everything `state.json` holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub members: BTreeMap<String, MemberState>,
    #[serde(default)]
    pub intents: BTreeMap<String, Intent>,
    /// Held leases by resource key.
    #[serde(default)]
    pub leases: BTreeMap<String, Lease>,
    #[serde(default)]
    pub agreements: Vec<Agreement>,
    /// The overlap last told to each pair, so a notice is posted once.
    #[serde(default)]
    pub told: BTreeMap<String, String>,
    /// The highest migration number handed out in this session.
    #[serde(default)]
    pub migration: u64,
}

/// A session's channel folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    dir: PathBuf,
}

impl Channel {
    /// Opens (making it when missing) the channel in `dir`.
    ///
    /// # Errors
    /// The folder can't be made.
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|error| {
            format!(
                "The channel folder {} can't be made: {error}",
                dir.display()
            )
        })?;
        Ok(Self { dir })
    }

    /// The channel for a session named `session` under Coder's store `root`.
    ///
    /// The folder is made when something is first written.
    ///
    /// # Errors
    /// The name is not a plain word.
    pub fn for_session(root: &Path, session: &str) -> Result<Self, String> {
        if session.is_empty()
            || !session
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            || session.starts_with('.')
        {
            return Err("A session name is letters, digits, dashes and dots.".into());
        }
        Ok(Self::at(root.join("comms").join(session)))
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn log_path(&self) -> PathBuf {
        self.dir.join("log.jsonl")
    }

    /// Runs `change` on the state under the lock, writes it back, and
    /// appends the records it returns to the log.
    fn update<T>(
        &self,
        change: impl FnOnce(&mut State, &mut Vec<Record>) -> T,
    ) -> Result<T, String> {
        fs::create_dir_all(&self.dir).map_err(|error| {
            format!(
                "The channel folder {} can't be made: {error}",
                self.dir.display()
            )
        })?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join("lock"))
            .map_err(|error| format!("The channel can't be locked: {error}"))?;
        lock.lock()
            .map_err(|error| format!("The channel can't be locked: {error}"))?;
        let mut state = self.read_state()?;
        let before = state.clone();
        let mut records = Vec::new();
        let value = change(&mut state, &mut records);
        if state != before {
            let path = self.dir.join("state.json");
            let temp = self.dir.join(format!("state.json.{}", std::process::id()));
            let text = serde_json::to_vec_pretty(&state).map_err(|error| error.to_string())?;
            fs::write(&temp, text)
                .and_then(|()| fs::rename(&temp, &path))
                .map_err(|error| format!("The channel state can't be saved: {error}"))?;
        }
        if !records.is_empty() {
            let mut log = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.log_path())
                .map_err(|error| format!("The channel log can't be written: {error}"))?;
            let mut bytes = Vec::new();
            for record in &records {
                serde_json::to_writer(&mut bytes, record).map_err(|error| error.to_string())?;
                bytes.push(b'\n');
            }
            log.write_all(&bytes)
                .map_err(|error| format!("The channel log can't be written: {error}"))?;
        }
        drop(lock);
        Ok(value)
    }

    /// The state as last saved (empty when there is none yet).
    ///
    /// # Errors
    /// The file is there but unreadable.
    pub fn read_state(&self) -> Result<State, String> {
        match fs::read(self.dir.join("state.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("The channel state is unreadable: {error}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(error) => Err(format!("The channel state can't be read: {error}")),
        }
    }

    /// The log's records from line `from` on, with the line after the last.
    /// A line that isn't a record is skipped.
    #[must_use]
    pub fn records(&self, from: usize) -> (Vec<Record>, usize) {
        let Ok(file) = File::open(self.log_path()) else {
            return (Vec::new(), from);
        };
        let mut records = Vec::new();
        let mut next = 0;
        for line in BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            next += 1;
            if next <= from || line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<Record>(&line) {
                records.push(record);
            }
        }
        if records.len() > READ_MAX {
            records.drain(..records.len() - READ_MAX);
        }
        (records, next)
    }

    /// Adds a member named after `label`, made unique (`Codex`, `Codex 2`).
    ///
    /// # Errors
    /// The channel can't be written.
    pub fn join(&self, kind: &str, label: &str) -> Result<Member, String> {
        let label = label.trim();
        let label = if label.is_empty() { "Agent" } else { label };
        let name = self.update(|state, records| {
            let mut name = label.to_owned();
            let mut n = 2;
            while state.members.contains_key(&name) || name == SYSTEM || name == ALL {
                name = format!("{label} {n}");
                n += 1;
            }
            state.members.insert(
                name.clone(),
                MemberState {
                    kind: kind.to_owned(),
                    joined_ms: now_ms(),
                    pid: Some(std::process::id()),
                    finished: None,
                },
            );
            records.push(Record::new(&name, Kind::Join, format!("{name} joined.")));
            name
        })?;
        Ok(Member {
            channel: self.clone(),
            name,
        })
    }

    /// Speaks as `name`, adding it as a member when it isn't one (a
    /// `coder comms` command from an agent the Coder didn't name).
    ///
    /// # Errors
    /// The name is empty or reserved, or the channel can't be written.
    pub fn member(&self, name: &str) -> Result<Member, String> {
        let name = name.trim();
        if name.is_empty() || name == SYSTEM || name == ALL {
            return Err("Name the member with --as NAME.".into());
        }
        self.update(|state, records| {
            if !state.members.contains_key(name) {
                state.members.insert(
                    name.to_owned(),
                    MemberState {
                        kind: "agent".into(),
                        joined_ms: now_ms(),
                        pid: None,
                        finished: None,
                    },
                );
                records.push(Record::new(name, Kind::Join, format!("{name} joined.")));
            }
        })?;
        Ok(Member {
            channel: self.clone(),
            name: name.to_owned(),
        })
    }

    /// Records the swarm's rule for an overlap no one has settled: the
    /// member whose intent came first goes first, the other rebases. Both
    /// are told. Returns the agreement, or `None` when one already exists.
    ///
    /// # Errors
    /// The channel can't be written.
    pub fn settle_by_order(
        &self,
        overlap: &Overlap,
        decided_by: &str,
    ) -> Result<Option<Agreement>, String> {
        self.update(|state, records| {
            let pair: BTreeSet<&str> = [overlap.a.as_str(), overlap.b.as_str()].into();
            if state
                .agreements
                .iter()
                .any(|agreement| agreement.pair() == pair)
            {
                return None;
            }
            let at = |name: &str| {
                state
                    .intents
                    .get(name)
                    .map_or(u64::MAX, |intent| intent.first_ms)
            };
            let (first, then) =
                if (at(&overlap.a), overlap.a.as_str()) <= (at(&overlap.b), overlap.b.as_str()) {
                    (overlap.a.clone(), overlap.b.clone())
                } else {
                    (overlap.b.clone(), overlap.a.clone())
                };
            let agreement = Agreement {
                order: vec![first, then],
                outcome: Outcome::First,
                on: overlap.shared(),
                decided_by: decided_by.to_owned(),
                note: "The earlier intent goes first.".into(),
                at_ms: now_ms(),
            };
            records.push(agreement_record(decided_by, &agreement));
            state.agreements.push(agreement.clone());
            Some(agreement)
        })
    }

    /// Drops leases that expired or whose Coder process is gone, recording
    /// each, at `now`.
    fn expire(state: &mut State, records: &mut Vec<Record>, now: u64) {
        let gone: Vec<String> = state
            .leases
            .iter()
            .filter(|(_, lease)| {
                lease.expires_ms <= now
                    || state
                        .members
                        .get(&lease.holder)
                        .and_then(|member| member.pid)
                        .is_some_and(|pid| {
                            pid != std::process::id() && !coder_lease::process_running(pid)
                        })
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in gone {
            if let Some(lease) = state.leases.remove(&key) {
                let mut record = Record::new(
                    SYSTEM,
                    Kind::Lease,
                    format!(
                        "{}'s lease on {} ended: it was not renewed.",
                        lease.holder, lease.resource
                    ),
                );
                record.at_ms = now;
                record.concerns = vec![lease.holder.clone()];
                record.data = json!({"event":"expired","lease":lease});
                records.push(record);
            }
        }
    }
}

fn agreement_record(from: &str, agreement: &Agreement) -> Record {
    let mut record = Record::new(from, Kind::Agreement, agreement.sentence());
    record.concerns = agreement.order.clone();
    record.data = json!(agreement);
    record
}

/// One agent in the session, as the channel knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    channel: Channel,
    name: String,
}

impl Member {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    /// Posts `text` to everyone, or to one member.
    ///
    /// # Errors
    /// The text is empty, the addressee isn't a member, or the channel
    /// can't be written.
    pub fn post(&self, to: Option<&str>, text: &str) -> Result<Record, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("A message needs some words.".into());
        }
        if text.len() > 16 * 1024 {
            return Err("A message is at most 16 KiB.".into());
        }
        self.channel.update(|state, records| {
            let mut record = Record::new(&self.name, Kind::Message, text);
            if let Some(to) = to.filter(|to| *to != ALL) {
                if !state.members.contains_key(to) {
                    let names: Vec<&str> = state.members.keys().map(String::as_str).collect();
                    return Err(format!(
                        "{to} isn't in this session. Members: {}.",
                        names.join(", ")
                    ));
                }
                to.clone_into(&mut record.to);
            }
            records.push(record.clone());
            Ok(record)
        })?
    }

    /// Says what this member means to change, replacing its last intent,
    /// and returns every overlap it now has. Each new or changed overlap is
    /// posted once, to both members.
    ///
    /// # Errors
    /// The intent is empty, or the channel can't be written.
    pub fn intent(
        &self,
        files: &[String],
        crates: &[String],
        resources: &[String],
        note: &str,
    ) -> Result<Vec<Overlap>, String> {
        let mut intent = Intent::new(&self.name, files, crates, resources, note);
        if intent.is_empty() {
            return Err("An intent names at least one file, crate or resource.".into());
        }
        self.channel.update(|state, records| {
            intent.first_ms = state
                .intents
                .get(&self.name)
                .map_or(intent.at_ms, |previous| previous.first_ms);
            intent.round = state
                .intents
                .get(&self.name)
                .map_or(0, |previous| previous.round + 1);
            let mut record = Record::new(&self.name, Kind::Intent, intent.sentence());
            record.data = json!(intent);
            records.push(record);
            state.intents.insert(self.name.clone(), intent.clone());
            let mut found = Vec::new();
            for (other, theirs) in &state.intents {
                if other == &self.name
                    || state.members.get(other).is_some_and(|member| {
                        // A change that won't land overlaps no one; a
                        // finished one still has a branch to merge.
                        member
                            .finished
                            .as_deref()
                            .is_some_and(|how| !matches!(how, "passed" | "done"))
                    })
                {
                    continue;
                }
                let Some(overlap) = overlap::between(&intent, theirs) else {
                    continue;
                };
                let key = overlap.pair_key();
                let told = overlap.shared().join(",");
                if state.told.get(&key) != Some(&told) {
                    let mut notice = Record::new(SYSTEM, Kind::Overlap, overlap.sentence());
                    notice.concerns = vec![overlap.a.clone(), overlap.b.clone()];
                    notice.data = json!(overlap);
                    records.push(notice);
                    state.told.insert(key, told);
                }
                found.push(overlap);
            }
            found
        })
    }

    /// Records how an overlap with `with` is settled. For `first`, `first`
    /// names who goes first (this member when not given); for `merge`, the
    /// pull request both go into is `first`'s.
    ///
    /// # Errors
    /// `with` isn't a member, or the channel can't be written.
    pub fn agree(
        &self,
        with: &str,
        outcome: Outcome,
        first: Option<&str>,
        on: &[String],
        note: &str,
    ) -> Result<Agreement, String> {
        if with == self.name {
            return Err("An agreement is between two members.".into());
        }
        self.channel.update(|state, records| {
            if !state.members.contains_key(with) {
                return Err(format!("{with} isn't in this session."));
            }
            let first = first.unwrap_or(&self.name);
            if first != self.name && first != with {
                return Err(format!("{first} isn't part of this agreement."));
            }
            let then = if first == self.name {
                with
            } else {
                self.name.as_str()
            };
            let on = if on.is_empty() {
                // What they overlap on, when they do.
                match (state.intents.get(&self.name), state.intents.get(with)) {
                    (Some(a), Some(b)) => overlap::between(a, b)
                        .map(|o| o.shared())
                        .unwrap_or_default(),
                    _ => Vec::new(),
                }
            } else {
                on.to_vec()
            };
            let agreement = Agreement {
                order: vec![first.to_owned(), then.to_owned()],
                outcome,
                on,
                decided_by: self.name.clone(),
                note: note.trim().to_owned(),
                at_ms: now_ms(),
            };
            let pair = agreement.pair();
            state.agreements.retain(|existing| existing.pair() != pair);
            state.agreements.push(agreement.clone());
            records.push(agreement_record(&self.name, &agreement));
            Ok(agreement)
        })?
    }

    /// Takes every lease in `requests`, or none: they are taken in the
    /// fixed resource order, and none may sort before a lease this member
    /// already holds (release those first). A lease this member holds is
    /// renewed. Each lease runs for `ttl_ms`.
    ///
    /// # Errors
    /// [`lease::Refusal`]: one is held by another member (who, and until
    /// when), the order would be broken, or the channel can't be written.
    pub fn acquire(
        &self,
        requests: &[LeaseRequest],
        ttl_ms: u64,
    ) -> Result<Vec<Lease>, lease::Refusal> {
        self.acquire_at(requests, ttl_ms, now_ms())
    }

    /// [`Member::acquire`] at the time `now`.
    ///
    /// # Errors
    /// As [`Member::acquire`].
    pub fn acquire_at(
        &self,
        requests: &[LeaseRequest],
        ttl_ms: u64,
        now: u64,
    ) -> Result<Vec<Lease>, lease::Refusal> {
        self.channel
            .update(|state, records| {
                Channel::expire(state, records, now);
                lease::acquire(state, records, &self.name, requests, ttl_ms, now)
            })
            .map_err(lease::Refusal::Channel)?
    }

    /// Waits until every lease in `requests` is free and takes them, asking
    /// a holder once over the channel when it'll be done. It waits as long
    /// as it takes (no time limit) unless `cancel` is set.
    ///
    /// # Errors
    /// The order would be broken, the request is invalid, `cancel` was set,
    /// or the channel can't be written.
    pub async fn acquire_waiting(
        &self,
        requests: &[LeaseRequest],
        ttl_ms: u64,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<Lease>, lease::Refusal> {
        let mut asked = BTreeSet::new();
        loop {
            match self.acquire(requests, ttl_ms) {
                Err(lease::Refusal::Held {
                    resource, holder, ..
                }) => {
                    if asked.insert(holder.clone()) {
                        let _ = self.post(
                            Some(&holder),
                            &format!("I'm waiting for {resource}, which you hold. When will you be done with it?"),
                        );
                    }
                }
                other => return other,
            }
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(lease::Refusal::Invalid(
                    "Stopped while waiting for a lease.".into(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    /// Extends a lease this member holds by `ttl_ms` from now.
    ///
    /// # Errors
    /// It holds no such lease (it may have expired), or the channel can't
    /// be written.
    pub fn renew(&self, resource: &str, ttl_ms: u64) -> Result<Lease, String> {
        self.renew_at(resource, ttl_ms, now_ms())
    }

    /// [`Member::renew`] at the time `now`.
    ///
    /// # Errors
    /// As [`Member::renew`].
    pub fn renew_at(&self, resource: &str, ttl_ms: u64, now: u64) -> Result<Lease, String> {
        self.channel.update(|state, records| {
            Channel::expire(state, records, now);
            lease::renew(state, &self.name, resource, ttl_ms, now)
        })?
    }

    /// Gives a lease back. Returns whether it held it.
    ///
    /// # Errors
    /// The channel can't be written.
    pub fn release(&self, resource: &str) -> Result<bool, String> {
        self.channel
            .update(|state, records| lease::release(state, records, &self.name, resource))
    }

    /// The leases this member holds now.
    #[must_use]
    pub fn leases(&self) -> Vec<Lease> {
        let now = now_ms();
        self.channel
            .read_state()
            .map(|state| {
                state
                    .leases
                    .into_values()
                    .filter(|lease| lease.holder == self.name && lease.expires_ms > now)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Ends this member: its leases are released and the session is told
    /// how it ended. Its intent stays: a change that passed still has
    /// a branch to merge, so it still overlaps; one that failed doesn't.
    ///
    /// # Errors
    /// The channel can't be written.
    pub fn finish(&self, how: &str) -> Result<(), String> {
        self.channel.update(|state, records| {
            let held: Vec<String> = state
                .leases
                .iter()
                .filter(|(_, lease)| lease.holder == self.name)
                .map(|(key, _)| key.clone())
                .collect();
            for key in held {
                lease::release(state, records, &self.name, &key);
            }
            if let Some(member) = state.members.get_mut(&self.name) {
                member.finished = Some(how.to_owned());
            }
            records.push(Record::new(
                &self.name,
                Kind::Finish,
                format!("{} finished: {how}", self.name),
            ));
        })
    }

    /// The records since line `cursor` this member should see, moving the
    /// cursor past them.
    #[must_use]
    pub fn inbox(&self, cursor: &mut usize) -> Vec<Record> {
        let (records, next) = self.channel.records(*cursor);
        *cursor = next;
        records
            .into_iter()
            .filter(|record| record.concerns(&self.name))
            .collect()
    }

    /// What a hand that can't read the channel itself is told: the
    /// overlaps and agreements about this member and the messages to it.
    /// Empty when there is nothing.
    #[must_use]
    pub fn notes(&self) -> String {
        let (records, _) = self.channel.records(0);
        let lines: Vec<String> = records
            .iter()
            .filter(|record| {
                record.concerns(&self.name)
                    && matches!(record.kind, Kind::Overlap | Kind::Agreement | Kind::Message)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(20)
            .rev()
            .map(|record| match record.kind {
                Kind::Message if record.to == self.name => {
                    format!("- {} to you: {}", record.from, record.text)
                }
                Kind::Message => format!("- {}: {}", record.from, record.text),
                _ => format!("- {}", record.text),
            })
            .collect();
        if lines.is_empty() {
            return String::new();
        }
        format!(
            "\n\n# Other agents in this session\n\nYou are {}. Other agents are working on this repository at the same time. What the session's channel says about you:\n\n{}\n\nKeep to what was agreed: when another agent goes first on a shared file, change only what your issue needs there so the rebase stays small.",
            self.name,
            lines.join("\n")
        )
    }
}

#[cfg(test)]
mod tests;
