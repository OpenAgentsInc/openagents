//! One landing queue for every machine (#11227).
//!
//! Agents on the Mac, CoderOS, GCE hosts, Boat sandboxes and cloud
//! environments each land their own changes with [`super::landing::land`],
//! and Git arbitrates their races. That lands each change, but nothing held
//! one queue across hosts, so an integrator could not take branches one at
//! a time. This module is that queue:
//!
//! - **Submit.** A machine pushes its change to a branch on `origin`
//!   (`land/<id>` by default) and writes an [`Entry`] into the queue's
//!   [`Store`]: a folder, or a prefix of a Cloud Storage bucket that every
//!   machine with the project's account can reach.
//! - **Work.** One worker (the integrator, on a cloud environment) takes
//!   the oldest queued entry: fetches `main` and the branch into its own
//!   worktree, rebases, runs the touched-crate checks (the issue flow's
//!   [`super::issue_run::Gate`]), and lands it with
//!   [`super::landing::land`], which retries a lost push race and rechecks
//!   only what newer commits can affect. A rebase conflict gets the
//!   landing's one bounded repair turn (#10418); a conflict that stays, or
//!   red checks, **bounce** the branch back to its author with the reason.
//! - **Record.** Every try is a [`Record`] beside the entry, and the entry's
//!   state follows it; `openagents land status` reads both.
//! - **Issues.** A landed entry closes its issue (or comments on it) and
//!   moves the board; a bounced one comments the reason.
//!
//! The queue is plain JSON objects, so the fleet view (#11228) reads it the
//! same way.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::issue_run::{Checks, Policy};
use super::land_plan::{self, Lane};
use super::landing;

/// The variable naming the queue: `gs://bucket/prefix` or a folder.
pub const QUEUE_ENV: &str = "OPENAGENTS_LAND_QUEUE";
/// The queue every machine shares when nothing else is named.
pub const DEFAULT_QUEUE: &str = "gs://openagentsgemini-coder-artifacts/land-queue/openagents";
/// How many times a landing that gave up on pushes goes back in line
/// before the entry bounces.
pub const REQUEUES: u32 = 3;
/// A worker whose heartbeat is older than this is gone.
pub const WORKER_STALE_SECS: u64 = 300;

/// Where an entry stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Waiting its turn.
    #[default]
    Queued,
    /// The worker has it now.
    Landing,
    /// On `main`.
    Landed,
    /// Back with its author: a conflict the repair did not fix, red
    /// checks, or pushes that kept failing.
    Bounced,
    /// Taken out of the queue by its author.
    Withdrawn,
}

impl State {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Landing => "landing",
            State::Landed => "landed",
            State::Bounced => "bounced",
            State::Withdrawn => "withdrawn",
        }
    }

    #[must_use]
    pub fn open(self) -> bool {
        matches!(self, State::Queued | State::Landing)
    }
}

/// One change waiting to land.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// `<UTC time>-<machine>-<suffix>`: entries sort oldest first by id.
    pub id: String,
    /// The branch on `origin` that holds the change.
    pub branch: String,
    /// The branch it lands on.
    pub target: String,
    /// The issue it is for, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    /// Close the issue once landed (else only comment on it).
    #[serde(default)]
    pub close: bool,
    /// Who submitted it (the Git identity).
    pub author: String,
    /// The machine or environment it came from.
    pub machine: String,
    /// One line: what the change does.
    pub summary: String,
    /// The branch commit submitted.
    pub head: String,
    pub submitted_at: u64,
    pub state: State,
    pub updated_at: u64,
    /// How many times the worker took it.
    #[serde(default)]
    pub tries: u32,
    /// The commit on the target once landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Why it bounced, or why the last try did not land.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The worker that has or had it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// The lane the worker planned for it ([`land_plan::Plan`]), #11248.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    /// The worker's slot that has or had it: `fast`, `code-1`, `code-2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    /// When the current or last try began.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    /// The earlier entry it waits for: one whose change can affect its
    /// checks lands first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<String>,
}

/// One try at landing an entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub entry: String,
    pub number: u32,
    pub worker: String,
    pub started_at: u64,
    pub ended_at: u64,
    /// `landed`, `bounced`, or `requeued`.
    pub outcome: String,
    /// The target's tip when the try began.
    pub target_before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Each landing attempt in a sentence ([`landing::Attempt::describe`]).
    #[serde(default)]
    pub landing: Vec<String>,
    /// What the checks ran.
    #[serde(default)]
    pub checks: Vec<String>,
    /// What the checks or the landing found wrong.
    #[serde(default)]
    pub problems: Vec<String>,
    /// Whether the repair turn ran on a conflict.
    #[serde(default)]
    pub repaired: bool,
    /// The landing's progress notes.
    #[serde(default)]
    pub notes: Vec<String>,
    /// The lane and slot it ran in (#11248).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lane: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub slot: String,
    /// Generated files the worker wrote again and folded into the change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regenerated: Vec<String>,
}

/// The worker's heartbeat, so submitters and the status view know whether
/// anyone is taking entries, and where to wake it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub machine: String,
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    /// The GCE instance it runs on, to start it when a submit finds it
    /// stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<Instance>,
    /// What each busy slot is landing (#11248).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Busy>,
    /// How many code entries it checks at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u32>,
    /// Finishing what it has and taking nothing new.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub draining: bool,
}

/// One busy slot in the heartbeat.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Busy {
    /// `fast`, `code-1`, ...
    pub slot: String,
    pub lane: String,
    pub entry: String,
    pub since: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub project: String,
    pub zone: String,
    pub name: String,
}

// ---------------------------------------------------------------------------
// The store.

/// Where the queue's objects live.
pub trait Store: Send + Sync {
    /// The names of the objects directly under `dir` (no trailing slash),
    /// without the `dir/` prefix.
    fn list(&self, dir: &str) -> Result<Vec<String>, String>;
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String>;
    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String>;
    /// Writes `name` only when it does not exist yet; `false` when it did.
    fn create(&self, name: &str, bytes: &[u8]) -> Result<bool, String>;
    /// Where the queue is, for people.
    fn location(&self) -> String;
}

/// The queue `url` names: `gs://bucket/prefix`, or a folder.
pub fn open(url: &str) -> Result<Box<dyn Store>, String> {
    let url = url.trim();
    if let Some(rest) = url.strip_prefix("gs://") {
        let rest = rest.trim_end_matches('/');
        if rest.is_empty() || rest.starts_with('/') {
            return Err(format!("`{url}` names no bucket."));
        }
        return Ok(Box::new(Gcs {
            root: format!("gs://{rest}"),
        }));
    }
    let url = url.trim_end_matches('/');
    let path = url.strip_prefix("file://").unwrap_or(url);
    if path.is_empty() {
        return Err("The queue names no place.".into());
    }
    Ok(Box::new(Dir(PathBuf::from(path))))
}

/// The queue this machine uses: `OPENAGENTS_LAND_QUEUE`, else the shared one.
pub fn open_default(named: Option<&str>) -> Result<Box<dyn Store>, String> {
    let env = std::env::var(QUEUE_ENV).ok();
    open(named.or(env.as_deref()).unwrap_or(DEFAULT_QUEUE))
}

/// A folder (tests, one machine, or a shared mount).
pub struct Dir(pub PathBuf);

impl Store for Dir {
    fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        match std::fs::read_dir(self.0.join(dir)) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if entry.path().is_file() {
                        names.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        names.sort();
        Ok(names)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(self.0.join(name)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    fn create(&self, name: &str, bytes: &[u8]) -> Result<bool, String> {
        use std::io::Write;
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(bytes).map_err(|e| e.to_string())?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }

    fn location(&self) -> String {
        self.0.display().to_string()
    }
}

/// A Cloud Storage prefix, through `gcloud storage` (so `CLOUDSDK_CONFIG`
/// picks the account, as [`super::run_artifacts`] does).
pub struct Gcs {
    root: String,
}

impl Gcs {
    fn gcloud(args: &[&str], input: Option<&[u8]>) -> Result<std::process::Output, String> {
        use std::io::Write;
        let mut child = Command::new("gcloud")
            .args(args)
            .arg("--quiet")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| "cannot run gcloud".to_owned())?;
        if let Some(bytes) = input
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin.write_all(bytes).map_err(|e| e.to_string())?;
        }
        child.wait_with_output().map_err(|e| e.to_string())
    }

    fn put(&self, name: &str, bytes: &[u8], only_new: bool) -> Result<bool, String> {
        let url = format!("{}/{name}", self.root);
        let mut args = vec!["storage", "cp", "-", url.as_str()];
        if only_new {
            args.push("--if-generation-match=0");
        }
        let output = Self::gcloud(&args, Some(bytes))?;
        if output.status.success() {
            return Ok(true);
        }
        let err = String::from_utf8_lossy(&output.stderr);
        if only_new && (err.contains("412") || err.contains("recondition")) {
            return Ok(false);
        }
        Err(format!("gcloud could not write {url}: {}", err.trim()))
    }
}

impl Store for Gcs {
    fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        let url = format!("{}/{dir}/", self.root);
        let output = Self::gcloud(&["storage", "ls", &url], None)?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("matched no objects") || err.contains("not found") {
                return Ok(Vec::new());
            }
            return Err(format!("gcloud could not list {url}: {}", err.trim()));
        }
        let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.trim().strip_prefix(url.as_str()))
            .filter(|name| !name.is_empty() && !name.ends_with('/'))
            .map(str::to_owned)
            .collect();
        names.sort();
        Ok(names)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let url = format!("{}/{name}", self.root);
        let output = Self::gcloud(&["storage", "cat", &url], None)?;
        if output.status.success() {
            return Ok(Some(output.stdout));
        }
        let err = String::from_utf8_lossy(&output.stderr);
        if err.contains("matched no objects") || err.contains("not found") || err.contains("404") {
            return Ok(None);
        }
        Err(format!("gcloud could not read {url}: {}", err.trim()))
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.put(name, bytes, false).map(|_| ())
    }

    fn create(&self, name: &str, bytes: &[u8]) -> Result<bool, String> {
        self.put(name, bytes, true)
    }

    fn location(&self) -> String {
        self.root.clone()
    }
}

// ---------------------------------------------------------------------------
// The queue over a store.

fn entry_name(id: &str) -> String {
    format!("entries/{id}.json")
}

fn record_name(id: &str, number: u32) -> String {
    format!("attempts/{id}/{number:03}.json")
}

/// Reads and writes entries, records and the heartbeat.
pub struct Queue<'a> {
    pub store: &'a dyn Store,
}

impl Queue<'_> {
    /// Every entry, oldest first.
    pub fn entries(&self) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        for name in self.store.list("entries")? {
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if let Some(entry) = self.entry(id)? {
                entries.push(entry);
            }
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(entries)
    }

    pub fn entry(&self, id: &str) -> Result<Option<Entry>, String> {
        match self.store.read(&entry_name(id))? {
            Some(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| format!("entry {id} is not readable: {e}")),
            None => Ok(None),
        }
    }

    pub fn put(&self, entry: &Entry) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(entry).map_err(|e| e.to_string())?;
        self.store.write(&entry_name(&entry.id), &bytes)
    }

    /// Adds a new entry; refused when its id is taken.
    pub fn submit(&self, entry: &Entry) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(entry).map_err(|e| e.to_string())?;
        if self.store.create(&entry_name(&entry.id), &bytes)? {
            Ok(())
        } else {
            Err(format!("an entry {} is already queued", entry.id))
        }
    }

    pub fn records(&self, id: &str) -> Result<Vec<Record>, String> {
        let mut records = Vec::new();
        for name in self.store.list(&format!("attempts/{id}"))? {
            if let Some(bytes) = self.store.read(&format!("attempts/{id}/{name}"))?
                && let Ok(record) = serde_json::from_slice::<Record>(&bytes)
            {
                records.push(record);
            }
        }
        records.sort_by_key(|record| record.number);
        Ok(records)
    }

    pub fn record(&self, record: &Record) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
        self.store
            .write(&record_name(&record.entry, record.number), &bytes)
    }

    pub fn worker(&self) -> Result<Option<Heartbeat>, String> {
        Ok(self
            .store
            .read("worker.json")?
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()))
    }

    pub fn beat(&self, worker: &Heartbeat) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(worker).map_err(|e| e.to_string())?;
        self.store.write("worker.json", &bytes)
    }

    /// Every entry, oldest first, reading again only those still open:
    /// `known` keeps the closed ones between calls, so a worker polling a
    /// long queue reads only what can change.
    pub fn entries_cached(
        &self,
        known: &mut std::collections::HashMap<String, Entry>,
    ) -> Result<Vec<Entry>, String> {
        let mut entries = Vec::new();
        for name in self.store.list("entries")? {
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if let Some(entry) = known.get(id)
                && !entry.state.open()
            {
                entries.push(entry.clone());
                continue;
            }
            if let Some(entry) = self.entry(id)? {
                if !entry.state.open() {
                    known.insert(id.to_owned(), entry.clone());
                }
                entries.push(entry);
            }
        }
        entries.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(entries)
    }

    /// The oldest entry waiting, or one this worker left mid-landing (a
    /// restart): those go first, in id order.
    pub fn next(&self, machine: &str) -> Result<Option<Entry>, String> {
        Ok(self.entries()?.into_iter().find(|entry| {
            entry.state == State::Queued
                || (entry.state == State::Landing && entry.worker.as_deref() == Some(machine))
        }))
    }
}

/// A new entry's id: the time, the machine, and a random suffix.
#[must_use]
pub fn new_id(now: u64, machine: &str) -> String {
    let machine: String = machine
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(24)
        .collect();
    let suffix = {
        use std::hash::{BuildHasher, Hasher};
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(now);
        hasher.write_u32(std::process::id());
        format!("{:04x}", hasher.finish() & 0xffff)
    };
    format!("{}-{machine}-{suffix}", stamp(now))
}

/// `20261010T150102Z`.
#[must_use]
pub fn stamp(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// This machine's name: `OPENAGENTS_MACHINE`, else the host name.
#[must_use]
pub fn machine() -> String {
    if let Ok(name) = std::env::var("OPENAGENTS_MACHINE")
        && !name.trim().is_empty()
    {
        return name.trim().to_owned();
    }
    Command::new("hostname")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

// ---------------------------------------------------------------------------
// The worker.

/// What the worker does to issues and branches outside Git's landing.
pub trait Effects {
    /// The entry landed: close or comment on its issue and move the board.
    fn landed(&mut self, top: &Path, entry: &Entry, text: &str) -> Result<(), String>;
    /// The entry bounced: tell its author why.
    fn bounced(&mut self, top: &Path, entry: &Entry, text: &str) -> Result<(), String>;
    /// One bounded repair turn on the conflicted rebase in `worktree`.
    fn repair(&mut self, worktree: &Path, request: &str) -> Result<(), String>;
}

/// What happened to one entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Landed(String),
    Bounced(String),
    Requeued(String),
}

/// The integrator for one slot: one checkout, its own worktree, one entry
/// at a time. [`super::land_lanes::Lanes`] runs several side by side.
pub struct Integrator<'a> {
    pub queue: Queue<'a>,
    /// The checkout whose `origin` the branches are on.
    pub top: PathBuf,
    /// This slot's own worktree (created when missing).
    pub worktree: PathBuf,
    pub machine: String,
    pub checks: &'a dyn Checks,
    pub effects: &'a mut dyn Effects,
    /// Landing tries per entry ([`landing::Plan::attempts`]).
    pub attempts: u32,
    pub backoff: landing::Backoff,
    /// The lane and slot this integrator lands in (#11248).
    pub lane: Lane,
    pub slot: String,
    /// Packages the entry reaches without touching their folders; the
    /// checks test them too ([`land_plan::Plan::also`]).
    pub also: Vec<String>,
    /// The lock every slot takes to push; `None` with one slot.
    pub push: Option<PushLock>,
    /// Write generated files again before the checks
    /// ([`land_plan::Generator`]).
    pub regenerate: bool,
}

/// The push lock the slots share: held only for fetch → rebase → push
/// ([`landing::Hooks::enter`]), never while checks run.
#[derive(Clone, Default)]
pub struct PushLock(std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>);

impl PushLock {
    /// Waits for the lock; it is let go when the guard drops.
    #[must_use]
    pub fn hold(&self) -> PushGuard {
        let (held, freed) = &*self.0;
        let mut taken = held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *taken {
            taken = freed
                .wait(taken)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *taken = true;
        PushGuard(self.0.clone())
    }
}

/// A held [`PushLock`].
pub struct PushGuard(std::sync::Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>);

impl Drop for PushGuard {
    fn drop(&mut self) {
        let (held, freed) = &*self.0;
        *held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = false;
        freed.notify_one();
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    super::local::git_out(dir, args).map(|out| out.trim().to_owned())
}

/// Worktrees of one checkout are added one at a time.
static WORKTREES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Makes `worktree` a detached worktree of `top` when it is not one yet.
pub(super) fn ensure_worktree(top: &Path, worktree: &Path) -> Result<(), String> {
    if worktree.join(".git").exists() {
        return Ok(());
    }
    let _one = WORKTREES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if worktree.join(".git").exists() {
        return Ok(());
    }
    if let Some(parent) = worktree.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let _ = git(top, &["worktree", "prune"]);
    git(
        top,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &worktree.to_string_lossy(),
            "HEAD",
        ],
    )
    .map(|_| ())
}

/// Fetches the target and the entry's branch into `worktree`'s refs,
/// under the repository's fetch lock (the slots share refs).
pub(super) fn fetch_entry(worktree: &Path, entry: &Entry) -> Result<(), String> {
    let _lock = landing::fetch_lock(worktree)?;
    git(
        worktree,
        &[
            "fetch",
            "-q",
            "origin",
            &format!("+refs/heads/{0}:refs/remotes/origin/{0}", entry.target),
            &format!("+refs/heads/{0}:refs/remotes/origin/{0}", entry.branch),
        ],
    )
    .map(|_| ())
    .map_err(|why| format!("Git could not fetch `{}`: {why}", entry.branch))
}

/// Problems that say the worker could not run the checks, not that the
/// change is wrong: the entry goes back in line instead of bouncing (at
/// most [`REQUEUES`] times, so a change that really fails to link still
/// bounces).
fn infrastructure(problems: &[String]) -> bool {
    problems.iter().any(|p| {
        p.contains("cargo is not on PATH")
            || p.starts_with("the tests could not run")
            || p.starts_with("the checks could not start")
            || p.contains("cargo fmt could not run")
            // A build folder emptied or a disk filled under a running
            // build (seen with two slots, #11248), not the change.
            || p.contains("linking with `cc` failed")
            || p.contains("No space left on device")
            || p.contains("Text file busy")
    })
}

impl Integrator<'_> {
    /// Takes the next entry and lands or bounces it; `None` when the queue
    /// is empty.
    pub fn step(&mut self) -> Result<Option<(Entry, Outcome)>, String> {
        let Some(entry) = self.queue.next(&self.machine)? else {
            return Ok(None);
        };
        self.run(entry).map(Some)
    }

    /// Lands or bounces `entry`, recording the try.
    pub fn run(&mut self, mut entry: Entry) -> Result<(Entry, Outcome), String> {
        entry.state = State::Landing;
        entry.tries += 1;
        entry.worker = Some(self.machine.clone());
        entry.lane = Some(self.lane);
        entry.slot = Some(self.slot.clone());
        entry.started_at = Some(now());
        entry.waiting_for = None;
        entry.updated_at = now();
        self.queue.put(&entry)?;
        let mut record = Record {
            entry: entry.id.clone(),
            number: entry.tries,
            worker: self.machine.clone(),
            started_at: now(),
            lane: self.lane.word().to_owned(),
            slot: self.slot.clone(),
            ..Record::default()
        };
        let outcome = self.land(&entry, &mut record);
        record.ended_at = now();
        match &outcome {
            Outcome::Landed(commit) => {
                record.outcome = "landed".into();
                record.commit = Some(commit.clone());
                entry.state = State::Landed;
                entry.commit = Some(commit.clone());
                entry.reason = None;
            }
            Outcome::Bounced(why) => {
                record.outcome = "bounced".into();
                entry.state = State::Bounced;
                entry.reason = Some(why.clone());
            }
            Outcome::Requeued(why) => {
                record.outcome = "requeued".into();
                entry.state = State::Queued;
                entry.reason = Some(why.clone());
            }
        }
        entry.updated_at = now();
        self.queue.record(&record)?;
        self.queue.put(&entry)?;
        match &outcome {
            Outcome::Landed(commit) => {
                if entry.branch.starts_with("land/") {
                    let _ = git(
                        &self.top,
                        &[
                            "push",
                            "-q",
                            "origin",
                            &format!(":refs/heads/{}", entry.branch),
                        ],
                    );
                }
                let text = landed_text(&entry, commit, &record);
                if let Err(why) = self.effects.landed(&self.top, &entry, &text) {
                    eprintln!(
                        "land: {} landed, but the issue was not updated: {why}",
                        entry.id
                    );
                }
            }
            Outcome::Bounced(why) => {
                let text = bounced_text(&entry, why, &record);
                if let Err(why) = self.effects.bounced(&self.top, &entry, &text) {
                    eprintln!(
                        "land: {} bounced, but its author was not told: {why}",
                        entry.id
                    );
                }
            }
            Outcome::Requeued(_) => {}
        }
        Ok((entry, outcome))
    }

    fn prepare(&self, entry: &Entry) -> Result<(), String> {
        ensure_worktree(&self.top, &self.worktree)?;
        let w = &self.worktree;
        let _ = git(w, &["rebase", "--abort"]);
        git(w, &["reset", "-q", "--hard"])?;
        git(w, &["clean", "-q", "-fd"])?;
        fetch_entry(w, entry)?;
        git(
            w,
            &[
                "checkout",
                "-q",
                "--detach",
                &format!("origin/{}", entry.branch),
            ],
        )?;
        Ok(())
    }

    fn land(&mut self, entry: &Entry, record: &mut Record) -> Outcome {
        let again = |why: String| {
            if entry.tries >= REQUEUES {
                Outcome::Bounced(why)
            } else {
                Outcome::Requeued(why)
            }
        };
        if let Err(why) = self.prepare(entry) {
            return again(why);
        }
        let target = format!("origin/{}", entry.target);
        record.target_before = git(&self.worktree, &["rev-parse", &target]).unwrap_or_default();
        let policy = Policy::load(&self.worktree).unwrap_or_default();
        let mut hooks = Hooks {
            worktree: self.worktree.clone(),
            target: target.clone(),
            checks: self.checks,
            policy,
            effects: &mut *self.effects,
            record,
            also: self.also.clone(),
            push: self.push.clone(),
            regenerate: self.regenerate,
        };
        // Rebase first so the checks run on the change as it would land;
        // a conflict is left to the landing, whose one repair turn takes it.
        let clean = git(&hooks.worktree, &["rebase", "-q", &target]).is_ok();
        if !clean {
            let _ = git(&hooks.worktree, &["rebase", "--abort"]);
            landing::Hooks::note(
                &mut hooks,
                "The branch conflicts with the target; the landing's repair turn takes it.",
            );
        } else {
            let problems = landing::Hooks::check(&mut hooks);
            if !problems.is_empty() {
                hooks.record.problems = problems.clone();
                if infrastructure(&problems) {
                    return again(format!(
                        "The worker could not run the checks: {}",
                        problems.join("; ")
                    ));
                }
                return Outcome::Bounced(format!(
                    "The checks fail on the change rebased onto `{}`: {}",
                    entry.target,
                    problems.join("; ")
                ));
            }
        }
        let plan = landing::Plan {
            worktree: &self.worktree,
            branch: &entry.target,
            attempts: self.attempts,
            backoff: self.backoff,
        };
        let result = landing::land(&plan, &mut hooks);
        let attempts = match &result {
            Ok(landed) => &landed.attempts,
            Err(not) => &not.attempts,
        };
        record.landing = attempts
            .iter()
            .map(|attempt| attempt.describe(&entry.target))
            .collect();
        match result {
            Ok(landed) => Outcome::Landed(landed.commit),
            Err(not) => match not.failure {
                landing::Failure::Red(problems) => {
                    record.problems.clone_from(&problems);
                    if infrastructure(&problems) {
                        return again(format!(
                            "The worker could not run the checks: {}",
                            problems.join("; ")
                        ));
                    }
                    Outcome::Bounced(format!(
                        "The checks fail after rebasing onto the newer `{}`: {}",
                        entry.target,
                        problems.join("; ")
                    ))
                }
                landing::Failure::Conflict(why) => Outcome::Bounced(why),
                landing::Failure::Stopped => Outcome::Requeued("The worker stopped.".into()),
                landing::Failure::GaveUp(why) | landing::Failure::Unreadable(why) => {
                    record.problems.push(why.clone());
                    again(why)
                }
            },
        }
    }
}

/// The worker's landing hooks: the issue flow's checks on the change as
/// one staged diff against the target, and the effects' repair turn.
struct Hooks<'a, 'b> {
    worktree: PathBuf,
    target: String,
    checks: &'a dyn Checks,
    policy: Policy,
    effects: &'a mut dyn Effects,
    record: &'b mut Record,
    also: Vec<String>,
    push: Option<PushLock>,
    regenerate: bool,
}

impl Hooks<'_, '_> {
    /// Writes the generated files the change is due again and folds them
    /// into its last commit.
    fn regenerate(&mut self) {
        let w = self.worktree.clone();
        let generators = land_plan::generators(&w);
        if generators.is_empty() {
            return;
        }
        let files: Vec<String> = git(&w, &["diff", "--name-only", &self.target, "HEAD"])
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        let read = |file: &str| std::fs::read_to_string(w.join(file)).ok();
        let due = land_plan::due(&generators, &files, &read);
        if due.is_empty() {
            return;
        }
        // The regenerating build takes a build slot as the checks do, so
        // it shares their warm target folders and the slot budget.
        let gate = super::issue_run::Gate {
            jev: None,
            store: Some(super::local::default_store()),
        };
        let lease = gate.slot(&w).ok().flatten();
        let mut rewritten = Vec::new();
        for generator in due {
            let mut command = Command::new("bash");
            command
                .args(["-c", &generator.regenerate])
                .current_dir(&w)
                .stdin(Stdio::null());
            if let Some(lease) = &lease {
                command.env("CARGO_TARGET_DIR", &lease.path);
            }
            match command.output() {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    let tail: String = String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .rev()
                        .take(6)
                        .collect::<Vec<_>>()
                        .join(" / ");
                    landing::Hooks::note(
                        self,
                        &format!("Could not write {} again: {tail}", generator.name),
                    );
                    continue;
                }
                Err(error) => {
                    landing::Hooks::note(
                        self,
                        &format!("Could not run {}'s command: {error}", generator.name),
                    );
                    continue;
                }
            }
            let mut args = vec!["status", "--porcelain", "--"];
            args.extend(generator.files.iter().map(String::as_str));
            if !git(&w, &args).unwrap_or_default().is_empty() {
                let mut add = vec!["add", "--"];
                add.extend(generator.files.iter().map(String::as_str));
                if git(&w, &add).is_ok() {
                    rewritten.extend(generator.files.iter().cloned());
                }
            }
        }
        drop(lease);
        if rewritten.is_empty() {
            return;
        }
        match git(&w, &["commit", "-q", "--amend", "--no-edit", "--no-verify"]) {
            Ok(_) => {
                landing::Hooks::note(
                    self,
                    &format!(
                        "Wrote {} again from the change's sources and folded it into the change.",
                        rewritten.join(", ")
                    ),
                );
                self.record.regenerated.extend(rewritten);
            }
            Err(why) => {
                let _ = git(&w, &["reset", "-q"]);
                landing::Hooks::note(
                    self,
                    &format!("Could not fold in the generated files: {why}"),
                );
            }
        }
    }
}

impl landing::Hooks for Hooks<'_, '_> {
    fn check(&mut self) -> Vec<String> {
        let w = self.worktree.clone();
        if git(&w, &["rev-parse", "HEAD"]).is_err() {
            return vec!["Git could not read the change.".into()];
        }
        let base = git(&w, &["merge-base", "HEAD", &self.target]).unwrap_or_default();
        if base.is_empty() || git(&w, &["rev-parse", "HEAD"]).ok().as_deref() == Some(base.as_str())
        {
            return vec!["The branch holds no change against the target.".into()];
        }
        if self.regenerate {
            self.regenerate();
        }
        let held = match git(&w, &["rev-parse", "HEAD"]) {
            Ok(head) => head,
            Err(why) => return vec![format!("Git could not read the change: {why}")],
        };
        // Every commit of the branch as one staged diff, which the gate
        // reads; then the commits come back exactly as they were.
        if let Err(why) = git(&w, &["reset", "-q", "--soft", &base]) {
            return vec![format!("Git could not stage the change: {why}")];
        }
        let checked = self.checks.check_also(&w, &self.policy, &self.also);
        let restored = git(&w, &["reset", "-q", "--soft", &held]);
        let _ = git(&w, &["reset", "-q"]);
        self.record.checks.extend(checked.ran.iter().cloned());
        let mut problems = checked.problems;
        if let Err(why) = restored {
            problems.push(format!("Git could not restore the change's commits: {why}"));
        }
        // Whatever the checks left behind is not part of the change.
        let _ = git(&w, &["checkout", "-q", "--", "."]);
        let _ = git(&w, &["clean", "-q", "-fd"]);
        problems
    }

    fn fix_conflict(&mut self, request: &str) -> Result<(), String> {
        // A conflict only in generated files is settled without a repair
        // turn: take the target's copy; the checks that follow write them
        // again from both sides' sources.
        if self.regenerate {
            let unmerged: Vec<String> =
                git(&self.worktree, &["diff", "--name-only", "--diff-filter=U"])
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_owned)
                    .collect();
            let generators = land_plan::generators(&self.worktree);
            if land_plan::all_generated(&generators, &unmerged) {
                let mut args = vec!["checkout", "--ours", "--"];
                args.extend(unmerged.iter().map(String::as_str));
                git(&self.worktree, &args)?;
                landing::Hooks::note(
                    self,
                    &format!(
                        "The rebase conflicts only in generated files ({}); took the target's \
                         copy to write again.",
                        unmerged.join(", ")
                    ),
                );
                return Ok(());
            }
        }
        self.record.repaired = true;
        self.effects.repair(&self.worktree, request)
    }

    fn note(&mut self, text: &str) {
        eprintln!("land: {text}");
        self.record.notes.push(text.to_owned());
    }

    fn stopping(&self) -> bool {
        false
    }

    fn enter(&mut self) -> Option<Box<dyn std::any::Any>> {
        self.push
            .as_ref()
            .map(|lock| Box::new(lock.hold()) as Box<dyn std::any::Any>)
    }
}

/// The fast lane's checks: no build, only the diff checks a document
/// change can fail — conflict markers, broken links, and figures with no
/// source (#11248).
pub struct DocChecks;

impl Checks for DocChecks {
    fn check(&self, worktree: &Path, _policy: &Policy) -> super::issue_run::Checked {
        let diff = git(worktree, &["diff", "--cached", "-U0"]).unwrap_or_default();
        let mut problems = Vec::new();
        let mut file = String::new();
        for line in diff.lines() {
            if let Some(path) = line.strip_prefix("+++ b/") {
                file = path.to_owned();
            } else if line.starts_with("+<<<<<<< ") || line.starts_with("+>>>>>>> ") {
                problems.push(format!("{file}: a conflict marker is left in"));
            }
        }
        problems.extend(coder_delegate::issue::broken_links(worktree, &diff));
        problems.extend(coder_delegate::issue::unsourced_figures(worktree, &diff));
        super::issue_run::Checked {
            problems,
            ran: vec![
                "Fast lane: the change holds no compiled code, so the build was skipped; the \
                 diff checks ran (conflict markers, broken links, figures with no source)."
                    .to_owned(),
            ],
        }
    }
}

fn landed_text(entry: &Entry, commit: &str, record: &Record) -> String {
    let mut text = format!(
        "Landed on `{}` as {} by the landing queue (entry `{}`, from `{}` on {}, worker {}).\n\n",
        entry.target,
        commit.get(..10).unwrap_or(commit),
        entry.id,
        entry.branch,
        entry.machine,
        record.worker
    );
    if !record.lane.is_empty() {
        text.push_str(&format!(
            "Lane: {} (slot {}), {} s from the start of the try.\n\n",
            record.lane,
            record.slot,
            record.ended_at.saturating_sub(record.started_at)
        ));
    }
    if !record.regenerated.is_empty() {
        text.push_str(&format!(
            "Generated files written again and folded in: {}.\n\n",
            record.regenerated.join(", ")
        ));
    }
    if !record.checks.is_empty() {
        text.push_str("Checks:\n");
        for line in &record.checks {
            text.push_str(&format!("- {line}\n"));
        }
    }
    if record.repaired {
        text.push_str(
            "\nThe rebase conflicted; one repair turn resolved it and the checks ran again.\n",
        );
    }
    if !record.landing.is_empty() {
        text.push_str("\nLanding:\n");
        for line in &record.landing {
            text.push_str(&format!("- {line}\n"));
        }
    }
    text
}

fn bounced_text(entry: &Entry, why: &str, record: &Record) -> String {
    let mut text = format!(
        "The landing queue bounced `{}` back to its author (entry `{}`, from {}, try {}): {}\n\n\
         Rebase the branch onto `{}`, fix it, and submit again with `openagents land submit`.\n",
        entry.branch, entry.id, entry.machine, record.number, why, entry.target
    );
    if !record.landing.is_empty() {
        text.push_str("\nLanding:\n");
        for line in &record.landing {
            text.push_str(&format!("- {line}\n"));
        }
    }
    text
}

/// The issue flow's checks as this computer runs them: the touched
/// packages' tests in one of the task store's build slots, the diff
/// checks, and Jev when it is reachable.
#[must_use]
pub fn gate() -> super::issue_run::Gate {
    let (jev, _) = crate::delegate_door::jev_from(&crate::delegate_door::env_value);
    super::issue_run::Gate {
        jev,
        store: Some(super::local::default_store()),
    }
}

// ---------------------------------------------------------------------------
// The real effects: `gh`, the board script, and a Claude Code repair turn.

/// Issue updates through `scripts/dev/issue-board.sh` (closing moves the
/// board to Done) or `gh`, and the repair turn through `claude -p`.
pub struct Live {
    /// `OWNER/NAME` for `gh`.
    pub repo: String,
    /// The repair command; `None` bounces conflicts without a repair turn.
    pub repair: Option<String>,
}

impl Live {
    fn gh(args: &[&str], dir: &Path) -> Result<(), String> {
        let out = Command::new("gh")
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .output()
            .map_err(|_| "cannot run gh".to_owned())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }
}

impl Effects for Live {
    fn landed(&mut self, top: &Path, entry: &Entry, text: &str) -> Result<(), String> {
        let Some(issue) = entry.issue else {
            return Ok(());
        };
        let issue = issue.to_string();
        if entry.close {
            let board = top.join("scripts/dev/issue-board.sh");
            if board.exists() {
                let out = Command::new("bash")
                    .arg(&board)
                    .args(["close", &issue, text])
                    .env("ISSUE_BOARD_REPO", &self.repo)
                    .current_dir(top)
                    .stdin(Stdio::null())
                    .output()
                    .map_err(|e| e.to_string())?;
                return if out.status.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&out.stdout).trim().to_owned())
                };
            }
            return Self::gh(
                &[
                    "issue",
                    "close",
                    &issue,
                    "--repo",
                    &self.repo,
                    "--comment",
                    text,
                ],
                top,
            );
        }
        Self::gh(
            &[
                "issue", "comment", &issue, "--repo", &self.repo, "--body", text,
            ],
            top,
        )
    }

    fn bounced(&mut self, top: &Path, entry: &Entry, text: &str) -> Result<(), String> {
        match entry.issue {
            Some(issue) => Self::gh(
                &[
                    "issue",
                    "comment",
                    &issue.to_string(),
                    "--repo",
                    &self.repo,
                    "--body",
                    text,
                ],
                top,
            ),
            None => Ok(()),
        }
    }

    fn repair(&mut self, worktree: &Path, request: &str) -> Result<(), String> {
        let Some(command) = self.repair.as_deref() else {
            return Err(
                "This worker has no repair turn; the conflict goes back to its author.".into(),
            );
        };
        let prompt = format!(
            "{request}\n\nYou are the landing queue's one repair turn. Resolve every conflict \
             in this worktree so both sides' intent survives, remove all conflict markers, and \
             leave the files saved. Do not commit, do not run `git rebase --continue`, and do \
             not touch files outside the conflict."
        );
        let out = Command::new(command)
            .args(["-p", &prompt, "--permission-mode", "acceptEdits"])
            .current_dir(worktree)
            .stdin(Stdio::null())
            .output()
            .map_err(|_| format!("cannot run {command}"))?;
        if !out.status.success() {
            return Err(format!(
                "the repair turn failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let markers = Command::new("git")
            .args(["grep", "-l", "-E", "^(<<<<<<<|>>>>>>>) "])
            .current_dir(worktree)
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .unwrap_or_default();
        if !markers.is_empty() {
            return Err(format!("conflict markers remain in {markers}"));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "land_queue_tests.rs"]
mod tests;
