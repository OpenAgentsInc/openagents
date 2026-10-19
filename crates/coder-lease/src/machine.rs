//! Machine actions (#12033). Since #12135 no grant is required to act; the
//! grants below are still recorded and shown, and nothing else changed.
//!
//! Original design (#12033): pruning a build cache, stopping a process, and
//! upgrading the host app as scopes the owner grants to one agent ID,
//! refused outside them, and logged.
//!
//! On 2026-10-17 a coordinating agent deleted about 640 GiB of build caches
//! and stopped another session's production deploy script on its own
//! judgment, with no scope and no record. Both were right calls, but powers
//! like these are granted before they are used, not after. So:
//!
//! - The owner grants [`Scope`]s to an **agent ID**: the chat's ID for an
//!   app chat, else its session (`claude-code:ID`, `codex:ID`). A grant
//!   never goes by a role name such as "Foreman"; any agent the owner names
//!   gets the same powers, and a fork of an agent gets none until granted.
//!   Grants live in `grants/machine.json` in the lease root, written only by
//!   the owner at a terminal (`openagents lease grant machine`).
//! - [`act`] checks the grant first and refuses an action outside it. In
//!   scope, it still refuses what the build sweep would never do: a target
//!   a build holds, the owner's dev-app target, agent scratch, a path that
//!   isn't a build cache, a process that runs a deploy (deploys stay with
//!   the owner), init, or the caller's own ancestors.
//! - Every action and every refusal is a line in `machine-actions.jsonl` in
//!   the lease root ([`LOG_FILE`]): who, what, the size or the pid and
//!   command name, the reason given, the outcome, and the Chicago time.
//!   `openagents lease actions` and the `machine_action` tool read it, so
//!   the owner and the agent see the same record.
//!
//! - A grant to a chat ID is usable only by that chat ([`proof`], #12067):
//!   the engine gives each chat a proof of its ID, and a command that names
//!   a chat without that chat's proof is refused and logged. The owner at an
//!   interactive terminal acts as the owner.
//!
//! - The owner may grant one coordinator [`Scope::Delegate`] (#12163). That
//!   agent then grants and revokes the [`Scope::DELEGABLE`] scopes to other
//!   chats on any of the owner's computers ([`delegate`]); a subgrant reaches
//!   the chat's own host through the device channel. Delegation never goes
//!   further: `delegate` itself isn't delegable, and money, keys and secrets
//!   are never machine scopes. Every subgrant and its end is logged with the
//!   delegating agent, the target chat, the scopes and the reason, and the
//!   owner's revoke of the coordinator ends everything it issued.
//!
//! On one Unix account this stops accidents and borrowed IDs, not a process
//! set on forging a grant, like the screen grant ([`crate::Grant`]).

pub mod delegate;
pub mod proof;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::agent_builds::sweep::{self, Kind, Layout, Proc};

/// The grants file, in the lease root.
pub const GRANTS_FILE: &str = "grants/machine.json";
/// The action log, one JSON object a line, in the lease root.
pub const LOG_FILE: &str = "machine-actions.jsonl";
/// The grants file's schema.
pub const GRANTS_SCHEMA: &str = "openagents.machine-grants.v1";
/// Each log line's schema.
pub const ENTRY_SCHEMA: &str = "openagents.machine-action.v1";
/// The log is moved aside to `.1` past this size.
const LOG_LIMIT: u64 = 8 << 20;

/// A machine power the owner grants to an agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Remove a build cache: a target folder or a folder inside one.
    #[serde(rename = "build.prune")]
    BuildPrune,
    /// Stop a process with `SIGTERM`.
    #[serde(rename = "process.stop")]
    ProcessStop,
    /// Upgrade the host app from `main` (#12056).
    #[serde(rename = "host.upgrade")]
    HostUpgrade,
    /// Grant and revoke the [`Scope::DELEGABLE`] scopes to other chats on
    /// the owner's computers (#12163). Only the owner grants it.
    #[serde(rename = "delegate")]
    Delegate,
}

impl Scope {
    pub const ALL: [Scope; 4] = [
        Scope::BuildPrune,
        Scope::ProcessStop,
        Scope::HostUpgrade,
        Scope::Delegate,
    ];
    /// What a `delegate` holder may grant: the machine actions, never
    /// `delegate` itself. Money, keys and secrets are never machine scopes.
    pub const DELEGABLE: [Scope; 3] = [Scope::BuildPrune, Scope::ProcessStop, Scope::HostUpgrade];

    /// Whether a `delegate` holder may grant it.
    #[must_use]
    pub fn delegable(self) -> bool {
        Scope::DELEGABLE.contains(&self)
    }

    /// The scope's wire name, such as `build.prune`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Scope::BuildPrune => "build.prune",
            Scope::ProcessStop => "process.stop",
            Scope::HostUpgrade => "host.upgrade",
            Scope::Delegate => "delegate",
        }
    }

    /// The scope a wire name names; `prune`, `stop`, and `upgrade` are
    /// short for the three.
    #[must_use]
    pub fn parse(word: &str) -> Option<Scope> {
        match word.trim() {
            "build.prune" | "prune" => Some(Scope::BuildPrune),
            "process.stop" | "stop" => Some(Scope::ProcessStop),
            "host.upgrade" | "upgrade" => Some(Scope::HostUpgrade),
            "delegate" => Some(Scope::Delegate),
            _ => None,
        }
    }
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.word())
    }
}

/// The owner's grant of scopes to one agent ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineGrant {
    /// The agent ID: a chat ID, or a session such as `claude-code:ID`.
    pub agent: String,
    pub scopes: Vec<Scope>,
    /// Unix milliseconds.
    pub granted_at_ms: u64,
    /// Unix milliseconds; never ends when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    /// The `delegate` holder that issued it (#12163); the owner when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_by: Option<String>,
    /// The reason the delegating agent gave.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl MachineGrant {
    /// Whether it lets `agent` use `scope` at `now_ms`.
    #[must_use]
    pub fn allows(&self, agent: &str, scope: Scope, now_ms: u64) -> bool {
        self.agent == agent
            && self.scopes.contains(&scope)
            && self.expires_at_ms.is_none_or(|end| now_ms < end)
    }
}

/// Every machine grant on this computer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    pub schema: String,
    pub grants: Vec<MachineGrant>,
}

impl Default for Grants {
    fn default() -> Grants {
        Grants {
            schema: GRANTS_SCHEMA.to_owned(),
            grants: Vec::new(),
        }
    }
}

impl Grants {
    /// The grants in the lease root `root`; none when the file is absent.
    ///
    /// # Errors
    /// The file can't be read or isn't a grants file.
    pub fn load(root: &Path) -> Result<Grants, String> {
        let path = root.join(GRANTS_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("{} is unreadable: {error}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Grants::default()),
            Err(error) => Err(format!("{} can't be read: {error}", path.display())),
        }
    }

    /// Whether a grant lets `agent` use `scope` at `now_ms`.
    #[must_use]
    pub fn allows(&self, agent: &str, scope: Scope, now_ms: u64) -> bool {
        self.grants
            .iter()
            .any(|grant| grant.allows(agent, scope, now_ms))
    }

    /// The scopes `agent` holds at `now_ms`.
    #[must_use]
    pub fn scopes_of(&self, agent: &str, now_ms: u64) -> Vec<Scope> {
        Scope::ALL
            .into_iter()
            .filter(|&scope| self.allows(agent, scope, now_ms))
            .collect()
    }

    pub(crate) fn save(&self, root: &Path) -> std::io::Result<()> {
        let path = root.join(GRANTS_FILE);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let staging = path.with_extension(format!("json.{}.new", std::process::id()));
        std::fs::write(&staging, json)?;
        std::fs::rename(&staging, &path)
    }
}

/// Grants `scopes` to `agent` in the lease root `root` from now, for
/// `duration` or with no end, replacing that agent's earlier grant. The
/// caller asks the owner first ([`crate::grant_refusal_for`]).
///
/// # Errors
/// The agent ID is empty or has spaces, no scope is named, or the file
/// can't be read or written.
pub fn grant(
    root: &Path,
    agent: &str,
    scopes: &[Scope],
    duration: Option<Duration>,
) -> Result<MachineGrant, String> {
    let agent = agent.trim();
    if agent.is_empty() || agent.contains(char::is_whitespace) {
        return Err("a machine grant needs one agent ID, such as a chat ID".to_owned());
    }
    if scopes.is_empty() {
        return Err("a machine grant needs at least one scope".to_owned());
    }
    let mut grants = Grants::load(root)?;
    let now = crate::now_ms();
    let mut scopes = scopes.to_vec();
    scopes.sort();
    scopes.dedup();
    let granted = MachineGrant {
        agent: agent.to_owned(),
        scopes,
        granted_at_ms: now,
        expires_at_ms: duration.map(|duration| {
            now.saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        }),
        issued_by: None,
        reason: None,
    };
    // A coordinator's subgrants to the same agent stay; they end with it.
    grants
        .grants
        .retain(|grant| grant.agent != agent || grant.issued_by.is_some());
    grants.grants.push(granted.clone());
    grants
        .save(root)
        .map_err(|error| format!("the machine grants can't be written: {error}"))?;
    Ok(granted)
}

/// Ends every machine grant `agent` holds in the lease root `root`, and
/// every grant it issued as a `delegate` holder (#12163); whether there
/// was any. Each subgrant ended this way is logged.
///
/// # Errors
/// The file can't be read or written.
pub fn revoke(root: &Path, agent: &str) -> Result<bool, String> {
    let agent = agent.trim();
    let mut grants = Grants::load(root)?;
    let (ended, kept): (Vec<MachineGrant>, Vec<MachineGrant>) =
        grants.grants.into_iter().partition(|grant| {
            grant.agent == agent || grant.issued_by.as_deref() == Some(agent)
        });
    grants.grants = kept;
    if ended.is_empty() {
        return Ok(false);
    }
    grants
        .save(root)
        .map_err(|error| format!("the machine grants can't be written: {error}"))?;
    for grant in ended.iter().filter(|grant| grant.agent != agent) {
        delegate::log_end(root, grant, &format!("the owner revoked {agent}"));
    }
    Ok(true)
}

/// Logs a refusal of a command that named `agent` without proving it
/// ([`proof::identify`]), and returns the reason.
pub fn refuse_unproven(
    root: &Path,
    agent: &str,
    scope: Scope,
    reason: &str,
    why: String,
) -> String {
    let mut entry = Entry::new(&System, agent.trim(), scope, reason.trim());
    entry.outcome = Outcome::Refused;
    entry.detail.clone_from(&why);
    let _ = log(root, &entry);
    why
}

/// What an agent asks to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Remove this build cache.
    Prune(PathBuf),
    /// Stop this process.
    Stop(u32),
}

impl Action {
    /// The scope it needs.
    #[must_use]
    pub fn scope(&self) -> Scope {
        match self {
            Action::Prune(_) => Scope::BuildPrune,
            Action::Stop(_) => Scope::ProcessStop,
        }
    }
}

/// How an action ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Done,
    Refused,
    Failed,
}

/// One line of the action log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub schema: String,
    /// Chicago time, such as `2026-10-17 6:41:03 PM CDT`.
    pub at: String,
    /// Unix milliseconds.
    pub at_ms: u64,
    /// The agent ID that asked.
    pub agent: String,
    pub scope: Scope,
    pub outcome: Outcome,
    /// The reason the agent gave.
    pub reason: String,
    /// The folder pruned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The bytes it held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// The process stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The process's command name, never its arguments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Why it was refused or failed, or what it did.
    pub detail: String,
    /// The chat a `delegate` holder granted to or revoked (#12163).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl Entry {
    /// A blank entry for `agent` using `scope` now on `host`'s clock, for
    /// callers outside [`act`] such as the host upgrade.
    #[must_use]
    pub fn new(host: &dyn Hands, agent: &str, scope: Scope, reason: &str) -> Entry {
        let now = host.now();
        Entry {
            schema: ENTRY_SCHEMA.to_owned(),
            at: host.stamp(now),
            at_ms: now
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(0)),
            agent: agent.to_owned(),
            scope,
            outcome: Outcome::Refused,
            reason: reason.to_owned(),
            path: None,
            bytes: None,
            pid: None,
            command: None,
            detail: String::new(),
            target: None,
        }
    }

    /// The entry as the owner reads it.
    #[must_use]
    pub fn line(&self) -> String {
        let what = match (&self.path, self.pid) {
            (Some(path), _) => match self.bytes {
                Some(bytes) => format!("{} {}", path.display(), sweep::size(bytes)),
                None => path.display().to_string(),
            },
            (None, Some(pid)) => match &self.command {
                Some(command) => format!("pid {pid} ({command})"),
                None => format!("pid {pid}"),
            },
            (None, None) => self
                .target
                .as_ref()
                .map_or_else(String::new, |target| format!("to {target}")),
        };
        let outcome = match self.outcome {
            Outcome::Done => "done",
            Outcome::Refused => "refused",
            Outcome::Failed => "failed",
        };
        format!(
            "{}  {}  {}  {outcome}  {what}  reason: {}  ({})",
            self.at, self.agent, self.scope, self.reason, self.detail
        )
    }
}

/// Appends `entry` to the log in the lease root `root`, moving a log past
/// 8 MiB aside first.
///
/// # Errors
/// The log can't be written.
pub fn log(root: &Path, entry: &Entry) -> std::io::Result<()> {
    use std::io::Write as _;
    std::fs::create_dir_all(root)?;
    let path = root.join(LOG_FILE);
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > LOG_LIMIT) {
        let _ = std::fs::rename(&path, path.with_extension("jsonl.1"));
    }
    let mut line = serde_json::to_vec(entry).map_err(std::io::Error::other)?;
    line.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?
        .write_all(&line)
}

/// The last `limit` entries in the lease root `root`, oldest first. Lines
/// that don't parse are skipped.
///
/// # Errors
/// The log exists but can't be read.
pub fn read_log(root: &Path, limit: usize) -> std::io::Result<Vec<Entry>> {
    let text = match std::fs::read_to_string(root.join(LOG_FILE)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let entries: Vec<Entry> = text
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let skip = entries.len().saturating_sub(limit);
    Ok(entries.into_iter().skip(skip).collect())
}

/// A process as [`act`] sees it before stopping it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    /// The command's file name, such as `cargo`.
    pub name: String,
    /// Its arguments, read only to refuse a deploy and never recorded.
    pub args: Vec<String>,
    /// Whether it runs as this account's user.
    pub ours: bool,
}

/// What [`act`] needs from the machine.
pub trait Hands {
    /// The running build processes, as the sweep reads them.
    ///
    /// # Errors
    /// The process table can't be read; every target then counts as held.
    fn builds(&self) -> Result<Vec<Proc>, String>;
    /// The process `pid`, when it runs.
    fn process(&self, pid: u32) -> Option<ProcInfo>;
    /// This process's ancestors, nearest first.
    fn ancestors(&self) -> Vec<u32>;
    /// Sends `SIGTERM` to `pid`.
    ///
    /// # Errors
    /// The signal can't be sent.
    fn terminate(&self, pid: u32) -> std::io::Result<()>;
    /// Removes the folder `path` and everything in it.
    ///
    /// # Errors
    /// It can't be removed.
    fn remove(&self, path: &Path) -> std::io::Result<()>;
    /// Whether a build holds a cargo lock in `target`.
    fn cargo_locked(&self, target: &Path) -> bool;
    fn now(&self) -> SystemTime;
    /// `at` in Chicago time.
    fn stamp(&self, at: SystemTime) -> String;
}

/// This Mac: the sweep's process reading, `ps`, `kill`, and the disk.
pub struct System;

impl Hands for System {
    fn builds(&self) -> Result<Vec<Proc>, String> {
        sweep::Host::processes(&sweep::System)
    }

    fn process(&self, pid: u32) -> Option<ProcInfo> {
        let output = std::process::Command::new("ps")
            .args(["-ww", "-o", "uid=,command=", "-p", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text.lines().next()?.trim();
        let (uid, command) = line.split_once(char::is_whitespace)?;
        let args: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        #[cfg(unix)]
        // SAFETY: `getuid` has no preconditions.
        let ours = uid.trim().parse::<u32>().ok() == Some(unsafe { libc::getuid() });
        #[cfg(not(unix))]
        let ours = !uid.is_empty();
        Some(ProcInfo {
            pid,
            name: crate::command_name(command),
            args,
            ours,
        })
    }

    fn ancestors(&self) -> Vec<u32> {
        let mut found = vec![std::process::id()];
        found.extend(crate::ancestors().into_iter().map(|(pid, _)| pid));
        found
    }

    fn terminate(&self, pid: u32) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            let pid = libc::pid_t::try_from(pid).map_err(std::io::Error::other)?;
            // SAFETY: `kill` with a positive pid signals that one process.
            if unsafe { libc::kill(pid, libc::SIGTERM) } == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            Err(std::io::Error::other("stopping a process needs Unix"))
        }
    }

    fn remove(&self, path: &Path) -> std::io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn cargo_locked(&self, target: &Path) -> bool {
        sweep::cargo_locked(target)
    }

    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn stamp(&self, at: SystemTime) -> String {
        sweep::chicago(at)
    }
}

/// This machine's lease root and the build sweep's layout under the home
/// folder.
///
/// # Errors
/// No home folder is known.
pub fn here() -> Result<(PathBuf, Layout), String> {
    let home = user_home::home_in(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .ok_or_else(|| "no home folder is known".to_owned())?;
    let root = crate::root_from_env().unwrap_or_else(|_| home.join(".openagents/leases"));
    let layout = Layout::at(&home, &root);
    Ok((root, layout))
}

/// Does `action` for `agent` if its grant allows it and the action is safe,
/// and logs it in the lease root `root` either way. `layout` says where the
/// build caches are. The returned entry is what was logged; a refusal is
/// `Outcome::Refused` with the reason in `detail`.
///
/// # Errors
/// The grants can't be read or the log can't be written; nothing was done.
pub fn act(
    root: &Path,
    layout: &Layout,
    hands: &dyn Hands,
    agent: &str,
    action: &Action,
    reason: &str,
) -> Result<Entry, String> {
    let agent = agent.trim();
    let mut entry = Entry::new(hands, agent, action.scope(), reason.trim());
    match action {
        Action::Prune(path) => entry.path = Some(path.clone()),
        Action::Stop(pid) => entry.pid = Some(*pid),
    }
    // No grant is needed (owner, 2026-10-18, #12135): the action's own
    // safety checks below still refuse deploys, other users and the caller.
    let refusal = if agent.is_empty() {
        Some("a machine action needs the agent ID of the chat or session asking".to_owned())
    } else if entry.reason.is_empty() {
        Some("a machine action needs a reason, which goes in the log".to_owned())
    } else {
        None
    };
    if let Some(refusal) = refusal {
        entry.detail = refusal;
    } else {
        match action {
            Action::Prune(path) => prune(layout, hands, path, &mut entry),
            Action::Stop(pid) => stop(hands, *pid, &mut entry),
        }
    }
    log(root, &entry)
        .map_err(|error| format!("the machine action log can't be written: {error}"))?;
    Ok(entry)
}

fn prune(layout: &Layout, hands: &dyn Hands, path: &Path, entry: &mut Entry) {
    let path: PathBuf = path.components().collect();
    let refuse = |entry: &mut Entry, detail: String| {
        entry.outcome = Outcome::Refused;
        entry.detail = detail;
    };
    if !path.is_absolute() || path.components().any(|part| part.as_os_str() == "..") {
        return refuse(entry, "the path must be absolute, without `..`".to_owned());
    }
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return refuse(
                entry,
                "not a folder; links and files aren't pruned".to_owned(),
            );
        }
        Err(error) => return refuse(entry, format!("can't be read: {error}")),
    }
    if layout.protected(&path) {
        return refuse(
            entry,
            "agent scratch and kept paths are never pruned".to_owned(),
        );
    }
    let Some((target, kind)) = layout
        .targets()
        .into_iter()
        .find(|(target, _)| path.starts_with(target))
    else {
        return refuse(
            entry,
            format!(
                "not a build cache: only target folders in {} and {} (or folders inside them) are pruned",
                layout.work.display(),
                layout.gate_targets.display()
            ),
        );
    };
    if kind == Kind::DevApp {
        return refuse(
            entry,
            "the owner's dev-app target is left to the build sweep, which takes it only below 40 GiB free".to_owned(),
        );
    }
    match hands.builds() {
        Err(error) => {
            return refuse(
                entry,
                format!("the process table can't be read, so every target counts as held: {error}"),
            );
        }
        Ok(procs) => {
            if let Some(proc) = procs.iter().find(|proc| proc.holds(&target)) {
                return refuse(
                    entry,
                    format!(
                        "{} {} is building in {}",
                        proc.name,
                        proc.pid,
                        target.display()
                    ),
                );
            }
        }
    }
    if hands.cargo_locked(&target) {
        return refuse(
            entry,
            format!("a build holds a cargo lock in {}", target.display()),
        );
    }
    let bytes = sweep::bytes_under(&path);
    entry.bytes = Some(bytes);
    match hands.remove(&path) {
        Ok(()) => {
            entry.outcome = Outcome::Done;
            entry.detail = format!("removed {}", sweep::size(bytes));
        }
        Err(error) => {
            entry.outcome = Outcome::Failed;
            entry.detail = format!("couldn't remove it: {error}");
        }
    }
}

fn stop(hands: &dyn Hands, pid: u32, entry: &mut Entry) {
    let refuse = |entry: &mut Entry, detail: String| {
        entry.outcome = Outcome::Refused;
        entry.detail = detail;
    };
    if pid <= 1 {
        return refuse(entry, "init and the kernel are never stopped".to_owned());
    }
    if hands.ancestors().contains(&pid) {
        return refuse(entry, "that's the caller or one of its parents".to_owned());
    }
    let Some(proc) = hands.process(pid) else {
        return refuse(entry, "no such process".to_owned());
    };
    entry.command = Some(proc.name.clone());
    if !proc.ours {
        return refuse(entry, "it runs as another user".to_owned());
    }
    if runs_deploy(&proc) {
        return refuse(
            entry,
            "it runs a deploy, and deploys are the owner's: ask the owner to stop it".to_owned(),
        );
    }
    match hands.terminate(pid) {
        Ok(()) => {
            entry.outcome = Outcome::Done;
            entry.detail = "sent SIGTERM".to_owned();
        }
        Err(error) => {
            entry.outcome = Outcome::Failed;
            entry.detail = format!("couldn't signal it: {error}");
        }
    }
}

/// Whether a process runs a deploy: a word of its command line (a file
/// name, or a subcommand such as `wrangler deploy`) starts with `deploy`.
fn runs_deploy(proc: &ProcInfo) -> bool {
    std::iter::once(proc.name.as_str())
        .chain(proc.args.iter().map(String::as_str))
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .any(|word| word.to_ascii_lowercase().starts_with("deploy"))
}

#[cfg(all(test, unix))]
mod tests;
