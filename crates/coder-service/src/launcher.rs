//! The launcher: runs the host, trials a new version against a state
//! snapshot, and commits or rolls back.
//!
//! # States
//!
//! The launcher keeps one durable record, `launcher.json`, whose `phase` is
//! one of four states. Every transition is a single atomic write.
//!
//! | Phase          | Meaning                                                     | Recovery after a crash                                   |
//! | -------------- | ----------------------------------------------------------- | -------------------------------------------------------- |
//! | `idle`         | The committed version runs, or runs next.                   | Remove unreferenced snapshots.                           |
//! | `prepared`     | A complete snapshot exists; the trial has not started.      | Roll back.                                               |
//! | `trial`        | The target runs as generation *n* with a ready deadline.    | Commit if the ready record for *n* exists, else roll back. |
//! | `rolling-back` | The snapshot is being restored.                             | Restore again; a restore is idempotent.                  |
//!
//! A trial commits when the host writes a ready record naming its
//! generation and version before the deadline. The prepared/committed
//! handshake is therefore: the launcher records `prepared`, starts the
//! trial, the host reports ready, and the launcher records the commit. A
//! host that exits, misses the deadline, or reports another generation
//! rolls back: the launcher stops it, restores the snapshot, and starts the
//! previous version again.
//!
//! Before any recovery, the launcher stops a host process group that its
//! record names and that still runs, so a host a crashed launcher left
//! behind never runs beside a restore or a second host.
//!
//! # The host contract
//!
//! The launcher starts `<bundle>/coder <host args>` with a cleared
//! environment holding `HOME`, `PATH` set to [`Config::search_path`], and
//! these variables:
//!
//! - `OPENAGENTS_HOST_READY_FILE`: where the host writes its ready record.
//! - `OPENAGENTS_HOST_GENERATION` and `OPENAGENTS_HOST_VERSION`: the values
//!   the ready record must repeat. The launcher reserves the generation from
//!   the host root's one counter, [`crate::generation`], which a standalone
//!   host advances too, so the two never hand out the same or a lower value.
//! - `OPENAGENTS_HOST_GENERATION_ROOT`: the launcher's host root, which
//!   holds that counter, so the host claims its generation there.
//! - `OPENAGENTS_HOST_LISTEN`: the loopback address to bind.
//! - `OPENAGENTS_HOST_TRIAL`: `1` during a trial, else `0`.
//!
//! The ready record is [`ReadyRecord`]. A host writes it atomically, for
//! example to a temporary name followed by a rename.

use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::descriptor::{
    self, DESCRIPTOR_SCHEMA, HostDescriptor, HostState, LAUNCHER_CAPABILITIES, UpdateState,
    UpdateView,
};
use crate::service::Platform;
use crate::{Error, Result, bundle, fsx, generation, snapshot};

/// The service configuration schema.
pub const CONFIG_SCHEMA: &str = "openagents.coder.host-service.v1";

/// The system directories every service search path starts with, and the
/// whole search path of a configuration written before it recorded one.
pub const BASE_SEARCH_PATH: &str = "/usr/bin:/bin";

/// The longest search path a configuration holds, in bytes.
const SEARCH_PATH_MAX: usize = 8192;

fn base_search_path() -> String {
    BASE_SEARCH_PATH.into()
}

/// The search path install records: [`BASE_SEARCH_PATH`], then each
/// absolute directory of `inherited` (the installing shell's `PATH`) that
/// is not already listed. Relative and empty entries, entries with a
/// control character, and entries past the length limit are dropped.
///
/// Some systems keep few tools in the base directories. On NixOS,
/// `/usr/bin` holds only `env` and `/bin` only `sh`, so a host limited to
/// them could not run `mv`, `sleep`, `git`, or a terminal's tools.
#[must_use]
pub fn search_path(inherited: Option<&str>) -> String {
    let mut entries: Vec<&str> = BASE_SEARCH_PATH.split(':').collect();
    let mut length = BASE_SEARCH_PATH.len();
    for entry in inherited.unwrap_or_default().split(':') {
        if !entry.starts_with('/')
            || entry.chars().any(char::is_control)
            || entries.contains(&entry)
            || length + 1 + entry.len() > SEARCH_PATH_MAX
        {
            continue;
        }
        length += 1 + entry.len();
        entries.push(entry);
    }
    entries.join(":")
}

fn validate_search_path(path: &str) -> Result<()> {
    let valid = !path.is_empty()
        && path.len() <= SEARCH_PATH_MAX
        && path
            .split(':')
            .all(|entry| entry.starts_with('/') && !entry.chars().any(char::is_control));
    if valid {
        Ok(())
    } else {
        Err(Error::refused(
            "the search path is 1 to 8192 bytes of absolute directories separated by colons",
        ))
    }
}
/// The launcher record schema.
pub const STATE_SCHEMA: &str = "openagents.coder.host-launcher.v1";
/// The ready record schema a host writes.
pub const READY_SCHEMA: &str = "openagents.coder.host-ready.v1";
/// The update request schema.
pub const REQUEST_SCHEMA: &str = "openagents.coder.host-update-request.v1";

/// How often the launcher checks its host, its stop flag, and requests.
const POLL: Duration = Duration::from_millis(50);

/// Where everything under one host root lives.
#[derive(Clone, Debug)]
pub struct Layout {
    root: PathBuf,
}

impl Layout {
    /// The layout under `root`, normally `~/.openagents/host`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Layout { root: root.into() }
    }

    /// The host root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// The service configuration.
    #[must_use]
    pub fn config(&self) -> PathBuf {
        self.root.join("service.json")
    }
    /// The launcher's durable record.
    #[must_use]
    pub fn state(&self) -> PathBuf {
        self.root.join("launcher.json")
    }
    /// The descriptor clients read.
    #[must_use]
    pub fn descriptor(&self) -> PathBuf {
        self.root.join("descriptor.json")
    }
    /// The pending update request.
    #[must_use]
    pub fn request(&self) -> PathBuf {
        self.root.join("update-request.json")
    }
    /// Snapshot directories.
    #[must_use]
    pub fn snapshots(&self) -> PathBuf {
        self.root.join("snapshots")
    }
    /// Ready records.
    #[must_use]
    pub fn run_dir(&self) -> PathBuf {
        self.root.join("run")
    }
    /// The ready record for one generation.
    #[must_use]
    pub fn ready(&self, generation: u64) -> PathBuf {
        self.run_dir().join(format!("ready-{generation}.json"))
    }
    /// Host and launcher logs.
    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    /// Rendered service definitions.
    #[must_use]
    pub fn service_dir(&self) -> PathBuf {
        self.root.join("service")
    }
    /// Installed launcher binaries.
    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }
    fn lock(&self) -> PathBuf {
        self.root.join("launcher.lock")
    }
}

/// The service configuration, `service.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Always [`CONFIG_SCHEMA`].
    pub schema: String,
    /// The launchd label, and the systemd unit name without `.service`.
    pub label: String,
    /// The service platform.
    pub platform: Platform,
    /// The directory the operating system reads service definitions from.
    pub registration_dir: PathBuf,
    /// The installed launcher binary the service runs.
    pub launcher: PathBuf,
    /// The bundle root `scripts/coder-host.py` stages into.
    pub bundle_root: PathBuf,
    /// The arguments after the host binary.
    pub host_args: Vec<String>,
    /// The state directories a trial snapshots and a rollback restores.
    pub state_dirs: Vec<PathBuf>,
    /// The loopback address the host binds.
    pub listen: String,
    /// The host's public key.
    pub host_key: String,
    /// How long a trial has to report ready.
    pub ready_timeout_secs: u64,
    /// How long a host has to exit after `SIGTERM`.
    pub stop_grace_secs: u64,
    /// The most file bytes one snapshot copies.
    pub snapshot_max_bytes: u64,
    /// The `PATH` of the launcher and the host it starts. A configuration
    /// without one uses [`BASE_SEARCH_PATH`].
    #[serde(default = "base_search_path")]
    pub search_path: String,
}

impl Config {
    /// Checks the configuration against the host root it lives in.
    pub fn validate(&self, layout: &Layout) -> Result<()> {
        if self.schema != CONFIG_SCHEMA {
            return Err(Error::refused("unsupported service configuration schema"));
        }
        crate::service::validate_label(&self.label)?;
        descriptor::validate_host_key(&self.host_key)?;
        descriptor::validate_loopback(&self.listen)?;
        if !(1..=3600).contains(&self.ready_timeout_secs)
            || !(1..=300).contains(&self.stop_grace_secs)
        {
            return Err(Error::refused(
                "ready_timeout_secs is 1 to 3600 and stop_grace_secs is 1 to 300",
            ));
        }
        if self.host_args.len() > 64
            || self
                .host_args
                .iter()
                .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        {
            return Err(Error::refused("host arguments are malformed"));
        }
        for path in [&self.bundle_root, &self.launcher, &self.registration_dir] {
            if !path.is_absolute() {
                return Err(Error::refused("service paths must be absolute"));
            }
        }
        validate_search_path(&self.search_path)?;
        if self.state_dirs.len() > 16 {
            return Err(Error::refused("at most 16 state directories"));
        }
        for state in &self.state_dirs {
            if !state.is_absolute() || state.parent().is_none() || state.file_name().is_none() {
                return Err(Error::refused(
                    "a state directory is an absolute path with a parent",
                ));
            }
            if state.starts_with(layout.root()) || layout.root().starts_with(state) {
                return Err(Error::refused(
                    "a state directory must not contain or sit inside the host root",
                ));
            }
        }
        Ok(())
    }

    /// Reads and validates the configuration under `layout`.
    pub fn load(layout: &Layout) -> Result<Self> {
        let bytes = fsx::read_optional(&layout.config())?.ok_or_else(|| {
            Error::refused(format!(
                "no host service is configured under {}; run `openagents service install` first",
                layout.root().display()
            ))
        })?;
        let config: Config = serde_json::from_slice(&bytes)?;
        config.validate(layout)?;
        Ok(config)
    }
}

/// Where an update is. See the module documentation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Phase {
    /// The committed version runs, or runs next.
    Idle,
    /// A complete snapshot exists and the trial has not started.
    Prepared {
        /// The request this update answers.
        request: String,
        /// The committed version.
        from: String,
        /// The trial version.
        to: String,
        /// The snapshot identifier.
        snapshot: String,
    },
    /// The trial version runs.
    Trial {
        /// The request this update answers.
        request: String,
        /// The committed version.
        from: String,
        /// The trial version.
        to: String,
        /// The snapshot identifier.
        snapshot: String,
        /// The trial host's generation.
        generation: u64,
        /// When the trial must have reported ready, in Unix milliseconds.
        deadline_ms: u64,
    },
    /// The snapshot is being restored.
    RollingBack {
        /// The request this update answers.
        request: String,
        /// The committed version, which runs again afterwards.
        from: String,
        /// The failed trial version.
        to: String,
        /// The snapshot identifier.
        snapshot: String,
        /// Why the trial failed.
        reason: String,
    },
}

/// The launcher's durable record, `launcher.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LauncherState {
    /// Always [`STATE_SCHEMA`].
    pub schema: String,
    /// The committed version.
    pub committed: String,
    /// The version committed before it.
    pub previous: Option<String>,
    /// The generation of the host last started, as reserved from the host
    /// root's counter.
    pub generation: u64,
    /// Where an update is.
    pub phase: Phase,
    /// The latest update's outcome, as the descriptor shows it.
    pub last: UpdateView,
    /// The process group of the host last started, until it is stopped.
    pub host_group: Option<i32>,
    /// The identity (start time, and on Linux the boot) of the process that
    /// leads `host_group`, recorded when the host started. Recovery kills
    /// the group only when a live leader still matches it, so a reused
    /// group identifier never names an unrelated process group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_identity: Option<String>,
}

/// What a host writes when it is ready to serve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyRecord {
    /// Always [`READY_SCHEMA`].
    pub schema: String,
    /// The generation from `OPENAGENTS_HOST_GENERATION`.
    pub generation: u64,
    /// The version from `OPENAGENTS_HOST_VERSION`.
    pub version: String,
    /// The host protocol version the host speaks.
    pub protocol_version: u32,
    /// The host's capability flags.
    pub capabilities: Vec<String>,
}

/// A request to update to another bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    /// Always [`REQUEST_SCHEMA`].
    pub schema: String,
    /// The request identifier the descriptor repeats.
    pub id: String,
    /// The bundle to trial.
    pub target: String,
}

/// Records a request to update to `target`, after checking that the bundle
/// is staged and intact. The running launcher picks it up.
pub fn request_update(layout: &Layout, target: &str) -> Result<UpdateRequest> {
    let config = Config::load(layout)?;
    bundle::release(&config.bundle_root, target)?;
    let request = UpdateRequest {
        schema: REQUEST_SCHEMA.into(),
        id: format!("u{}-{}", fsx::now_ms(), std::process::id()),
        target: target.into(),
    };
    fsx::atomic_write(
        &layout.request(),
        &serde_json::to_vec_pretty(&request)?,
        0o600,
    )?;
    Ok(request)
}

/// Writes the configuration and, when no launcher record exists, the
/// initial record committing `version`. An existing record with a
/// different committed version refuses: change versions with an update.
pub fn initialize(layout: &Layout, config: &Config, version: &str) -> Result<()> {
    fsx::private_dir(layout.root())?;
    config.validate(layout)?;
    bundle::release(&config.bundle_root, version)?;
    // launchd creates a missing log directory with its own permissions
    // before the launcher runs, so every directory exists privately first.
    for dir in [layout.logs(), layout.run_dir(), layout.snapshots()] {
        fsx::private_dir(&dir)?;
    }
    if let Some(bytes) = fsx::read_optional(&layout.state())? {
        let state: LauncherState = serde_json::from_slice(&bytes)?;
        if state.committed != version {
            return Err(Error::refused(format!(
                "the host already committed {}; use `openagents service update --to {version}` to change versions",
                state.committed
            )));
        }
    } else {
        let state = LauncherState {
            schema: STATE_SCHEMA.into(),
            committed: version.into(),
            previous: None,
            generation: 0,
            phase: Phase::Idle,
            last: UpdateView::none(),
            host_group: None,
            host_identity: None,
        };
        fsx::atomic_write(&layout.state(), &serde_json::to_vec_pretty(&state)?, 0o600)?;
    }
    fsx::atomic_write(&layout.config(), &serde_json::to_vec_pretty(config)?, 0o600)?;
    Ok(())
}

/// Reads the descriptor the launcher last wrote.
pub fn read_descriptor(layout: &Layout) -> Result<Option<HostDescriptor>> {
    fsx::read_optional(&layout.descriptor())?
        .map(|bytes| HostDescriptor::decode(&bytes))
        .transpose()
}

/// A host process the launcher started.
struct Host {
    child: Child,
    group: i32,
    generation: u64,
    version: String,
    ready: Option<ReadyRecord>,
}

type HookFn = Box<dyn FnMut(&'static str) -> Result<()> + Send>;

/// The launcher for one host root. It holds the root's lock while it
/// exists.
pub struct Launcher {
    layout: Layout,
    config: Config,
    state: LauncherState,
    host: Option<Host>,
    hook: HookFn,
    _lock: File,
}

impl Launcher {
    /// Opens the launcher under `layout`, taking its exclusive lock.
    pub fn open(layout: Layout) -> Result<Self> {
        let config = Config::load(&layout)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(layout.lock())?;
        // SAFETY: `flock` reads a descriptor this function owns and two
        // integer flags.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(Error::refused(
                "another launcher is running for this host root",
            ));
        }
        let bytes = fsx::read_optional(&layout.state())?.ok_or_else(|| {
            Error::refused("the launcher record is missing; reinstall the service")
        })?;
        let state: LauncherState = serde_json::from_slice(&bytes)?;
        if state.schema != STATE_SCHEMA {
            return Err(Error::refused("unsupported launcher record schema"));
        }
        for dir in [layout.snapshots(), layout.run_dir(), layout.logs()] {
            fsx::private_dir(&dir)?;
        }
        Ok(Launcher {
            layout,
            config,
            state,
            host: None,
            hook: Box::new(|_| Ok(())),
            _lock: lock,
        })
    }

    /// The durable record.
    #[must_use]
    pub fn state(&self) -> &LauncherState {
        &self.state
    }

    #[cfg(test)]
    fn crash_at(&mut self, point: &'static str) {
        self.hook = Box::new(move |at| {
            if at == point {
                Err(Error::Crash(at))
            } else {
                Ok(())
            }
        });
    }

    /// Stands in for a crash: releases the lock and leaves the host
    /// running, returning its handle so the test can reap it later.
    #[cfg(test)]
    fn crash(mut self) -> Option<Child> {
        self.host.take().map(|host| host.child)
    }

    /// Brings the record to a committed state after the launcher stopped
    /// in any phase, and returns the update outcome it recorded, if any.
    pub fn recover(&mut self) -> Result<Option<UpdateState>> {
        if let Some(group) = self.state.host_group {
            let identity = self.state.host_identity.clone();
            stop_orphan(
                group,
                identity.as_deref(),
                Duration::from_secs(self.config.stop_grace_secs),
            );
            self.state.host_group = None;
            self.state.host_identity = None;
            self.save()?;
        }
        let outcome = match self.state.phase.clone() {
            Phase::Idle => None,
            Phase::Prepared { .. } => {
                self.roll_back("the launcher stopped before the trial started")?;
                Some(UpdateState::RolledBack)
            }
            Phase::Trial { generation, to, .. } => {
                if let Some(ready) = self.read_ready(generation, &to) {
                    self.commit(&ready)?;
                    Some(UpdateState::Committed)
                } else {
                    self.roll_back(
                        "the launcher stopped during the trial, before the host reported ready",
                    )?;
                    Some(UpdateState::RolledBack)
                }
            }
            Phase::RollingBack { .. } => {
                self.finish_rollback()?;
                Some(UpdateState::RolledBack)
            }
        };
        self.collect_garbage()?;
        // Reading the pending request removes one this record already
        // answered, such as a request whose commit was recorded just
        // before a crash.
        self.pending_request()?;
        self.write_descriptor(HostState::Stopped)?;
        Ok(outcome)
    }

    /// Runs the host until `stop` is set or the host exits, handling update
    /// requests as they arrive. Returns the exit code the service should
    /// report: zero after a requested stop or a clean host exit.
    pub fn run(&mut self, stop: &AtomicBool) -> Result<i32> {
        self.recover()?;
        loop {
            if stop.load(Ordering::SeqCst) {
                self.stop_host();
                self.save()?;
                self.write_descriptor(HostState::Stopped)?;
                return Ok(0);
            }
            if let Some(request) = self.pending_request()? {
                self.stop_host();
                self.update(&request, stop)?;
                continue;
            }
            if self.host.is_none() {
                self.start_committed()?;
            }
            if let Some(code) = self.poll_host()? {
                self.state.host_group = None;
                self.save()?;
                self.write_descriptor(HostState::Stopped)?;
                return Ok(code);
            }
            std::thread::sleep(POLL);
        }
    }

    /// Runs one update to completion: snapshot, trial, then commit or
    /// rollback. The committed version is running afterwards.
    pub fn update(&mut self, request: &UpdateRequest, stop: &AtomicBool) -> Result<UpdateState> {
        if request.target == self.state.committed {
            let from = self.state.committed.clone();
            self.state.last = view(UpdateState::Committed, request, &from, None);
            self.save()?;
            fsx::remove_file_if_present(&self.layout.request())?;
            self.start_committed()?;
            return Ok(UpdateState::Committed);
        }
        if let Err(error) = self.prepare(request) {
            if matches!(error, Error::Crash(_)) || self.state.phase != Phase::Idle {
                return Err(error);
            }
            // Nothing ran and nothing changed; the committed version runs
            // again and the request is answered rather than retried forever.
            let from = self.state.committed.clone();
            let reason = format!("the update was refused before a trial: {error}");
            self.state.last = view(UpdateState::RolledBack, request, &from, Some(reason));
            self.save()?;
            fsx::remove_file_if_present(&self.layout.request())?;
            self.collect_garbage()?;
            self.start_committed()?;
            return Ok(UpdateState::RolledBack);
        }
        let trial = match self.start_trial() {
            Ok(()) => self.await_trial(stop),
            Err(Error::Crash(point)) => return Err(Error::Crash(point)),
            Err(error) => Err(format!("the trial host could not start: {error}")),
        };
        match trial {
            Ok(ready) => {
                self.commit(&ready)?;
                Ok(UpdateState::Committed)
            }
            Err(reason) => {
                self.roll_back(&reason)?;
                self.start_committed()?;
                Ok(UpdateState::RolledBack)
            }
        }
    }

    /// Takes the snapshot and records `prepared`.
    pub fn prepare(&mut self, request: &UpdateRequest) -> Result<()> {
        if self.state.phase != Phase::Idle {
            return Err(Error::refused("another update is in progress"));
        }
        descriptor::validate_request_id(&request.id)?;
        bundle::release(&self.config.bundle_root, &request.target)?;
        self.write_descriptor(HostState::Updating)?;
        let id = request.id.clone();
        snapshot::take(
            &self.layout.snapshots(),
            &id,
            &self.config.state_dirs,
            self.config.snapshot_max_bytes,
            &mut *self.hook,
        )?;
        self.state.phase = Phase::Prepared {
            request: request.id.clone(),
            from: self.state.committed.clone(),
            to: request.target.clone(),
            snapshot: id,
        };
        self.save()?;
        (self.hook)("prepared")?;
        self.write_descriptor(HostState::Updating)
    }

    /// Records `trial` and starts the target version.
    pub fn start_trial(&mut self) -> Result<()> {
        let Phase::Prepared {
            request,
            from,
            to,
            snapshot,
        } = self.state.phase.clone()
        else {
            return Err(Error::refused("no prepared update"));
        };
        let generation = generation::reserve(self.layout.root(), self.state.generation)?;
        let timeout_ms = self.config.ready_timeout_secs.saturating_mul(1000);
        self.state.generation = generation;
        self.state.phase = Phase::Trial {
            request,
            from,
            to: to.clone(),
            snapshot,
            generation,
            deadline_ms: fsx::now_ms().saturating_add(timeout_ms),
        };
        self.save()?;
        self.spawn(&to, generation, true)?;
        self.write_descriptor(HostState::Updating)?;
        (self.hook)("trial-started")
    }

    /// Waits for the trial host to report ready. `Err` holds the reason
    /// the trial failed.
    fn await_trial(&mut self, stop: &AtomicBool) -> std::result::Result<ReadyRecord, String> {
        let Phase::Trial { deadline_ms, .. } = self.state.phase else {
            return Err("no trial is running".into());
        };
        loop {
            match self.poll_host() {
                Ok(Some(code)) => {
                    return Err(format!(
                        "the trial host exited with code {code} before it reported ready"
                    ));
                }
                Ok(None) => {}
                Err(error) => return Err(format!("the trial host could not be observed: {error}")),
            }
            if let Some(ready) = self.host.as_ref().and_then(|host| host.ready.clone()) {
                return Ok(ready);
            }
            if fsx::now_ms() >= deadline_ms {
                return Err(format!(
                    "the trial host did not report ready within {} seconds",
                    self.config.ready_timeout_secs
                ));
            }
            if stop.load(Ordering::SeqCst) {
                return Err("the launcher was asked to stop during the trial".into());
            }
            std::thread::sleep(POLL);
        }
    }

    /// Records the commit: the trial version becomes the committed version
    /// and keeps running.
    pub fn commit(&mut self, ready: &ReadyRecord) -> Result<()> {
        let Phase::Trial {
            request,
            from,
            to,
            snapshot,
            ..
        } = self.state.phase.clone()
        else {
            return Err(Error::refused("no trial to commit"));
        };
        self.state.previous = Some(from.clone());
        self.state.committed = to.clone();
        self.state.phase = Phase::Idle;
        self.state.last = UpdateView {
            from: Some(from),
            reason: None,
            request: Some(request),
            state: UpdateState::Committed,
            target: Some(to),
        };
        self.save()?;
        (self.hook)("committed")?;
        fsx::remove_file_if_present(&self.layout.request())?;
        fsx::remove_tree_if_present(&self.layout.snapshots().join(snapshot))?;
        if let Some(host) = &mut self.host {
            host.ready = Some(ready.clone());
        }
        let state = if self.host.is_some() {
            HostState::Ready
        } else {
            HostState::Stopped
        };
        self.write_descriptor(state)
    }

    /// Stops any trial host, records `rolling-back`, and restores the
    /// snapshot.
    pub fn roll_back(&mut self, reason: &str) -> Result<()> {
        let (request, from, to, snapshot) = match self.state.phase.clone() {
            Phase::Prepared {
                request,
                from,
                to,
                snapshot,
            }
            | Phase::Trial {
                request,
                from,
                to,
                snapshot,
                ..
            }
            | Phase::RollingBack {
                request,
                from,
                to,
                snapshot,
                ..
            } => (request, from, to, snapshot),
            Phase::Idle => return Err(Error::refused("no update to roll back")),
        };
        self.stop_host();
        let reason: String = reason
            .chars()
            .filter(|c| !c.is_control())
            .take(512)
            .collect();
        self.state.phase = Phase::RollingBack {
            request,
            from,
            to,
            snapshot,
            reason,
        };
        self.save()?;
        (self.hook)("rolling-back")?;
        self.finish_rollback()
    }

    fn finish_rollback(&mut self) -> Result<()> {
        let Phase::RollingBack {
            request,
            from,
            to,
            snapshot,
            reason,
        } = self.state.phase.clone()
        else {
            return Err(Error::refused("no rollback in progress"));
        };
        let directory = self.layout.snapshots().join(&snapshot);
        snapshot::restore(&directory, &mut *self.hook)?;
        self.state.committed = from.clone();
        self.state.phase = Phase::Idle;
        self.state.last = UpdateView {
            from: Some(from),
            reason: Some(reason),
            request: Some(request),
            state: UpdateState::RolledBack,
            target: Some(to),
        };
        self.save()?;
        fsx::remove_file_if_present(&self.layout.request())?;
        fsx::remove_tree_if_present(&directory)?;
        self.write_descriptor(HostState::Stopped)
    }

    /// Starts the committed version as a new generation.
    fn start_committed(&mut self) -> Result<()> {
        self.state.generation = generation::reserve(self.layout.root(), self.state.generation)?;
        self.save()?;
        let version = self.state.committed.clone();
        self.spawn(&version, self.state.generation, false)?;
        self.write_descriptor(HostState::Starting)
    }

    fn spawn(&mut self, version: &str, generation: u64, trial: bool) -> Result<()> {
        let binary = bundle::release(&self.config.bundle_root, version)?;
        let ready = self.layout.ready(generation);
        fsx::remove_file_if_present(&ready)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.layout.logs().join("host.log"))?;
        let mut command = Command::new(&binary);
        command
            .args(&self.config.host_args)
            .env_clear()
            .env("PATH", &self.config.search_path)
            .env("HOME", std::env::var_os("HOME").unwrap_or_default())
            .env("OPENAGENTS_HOST_READY_FILE", &ready)
            .env("OPENAGENTS_HOST_GENERATION", generation.to_string())
            .env("OPENAGENTS_HOST_GENERATION_ROOT", self.layout.root())
            .env("OPENAGENTS_HOST_VERSION", version)
            .env("OPENAGENTS_HOST_LISTEN", &self.config.listen)
            .env("OPENAGENTS_HOST_TRIAL", if trial { "1" } else { "0" })
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        supervise::blocking::own_group(&mut command);
        let child = command.spawn()?;
        let group = i32::try_from(child.id())
            .map_err(|_| Error::refused("the host process identifier is out of range"))?;
        self.host = Some(Host {
            child,
            group,
            generation,
            version: version.into(),
            ready: None,
        });
        self.state.host_group = Some(group);
        self.state.host_identity = process_identity(group);
        self.save()
    }

    /// Checks the host: `Some(code)` once it exited, and records a ready
    /// report when one appears.
    fn poll_host(&mut self) -> Result<Option<i32>> {
        let Some(host) = &mut self.host else {
            return Ok(None);
        };
        if let Some(status) = host.child.try_wait()? {
            let _ = supervise::blocking::wait(&mut host.child, Duration::ZERO);
            self.host = None;
            return Ok(Some(status.code().unwrap_or(1)));
        }
        if host.ready.is_none() {
            let (generation, version) = (host.generation, host.version.clone());
            if let Some(ready) = self.read_ready(generation, &version) {
                if let Some(host) = &mut self.host {
                    host.ready = Some(ready);
                }
                if matches!(self.state.phase, Phase::Idle) {
                    self.write_descriptor(HostState::Ready)?;
                }
            }
        }
        Ok(None)
    }

    fn read_ready(&self, generation: u64, version: &str) -> Option<ReadyRecord> {
        let bytes = fsx::read_optional(&self.layout.ready(generation)).ok()??;
        let ready: ReadyRecord = serde_json::from_slice(&bytes).ok()?;
        let valid = ready.schema == READY_SCHEMA
            && ready.generation == generation
            && ready.version == version
            && ready.capabilities.len() <= descriptor::CAPABILITIES_MAX
            && ready
                .capabilities
                .iter()
                .all(|flag| descriptor::validate_capability(flag).is_ok());
        valid.then_some(ready)
    }

    fn stop_host(&mut self) {
        if let Some(mut host) = self.host.take() {
            let grace = Duration::from_secs(self.config.stop_grace_secs);
            // SAFETY: `killpg` reads two integers; the group is the one
            // `own_group` made for this host.
            unsafe { libc::killpg(host.group, libc::SIGTERM) };
            let deadline = Instant::now() + grace;
            while Instant::now() < deadline {
                if matches!(host.child.try_wait(), Ok(Some(_)) | Err(_)) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            // A zero wall ends the group and reaps the direct child.
            let _ = supervise::blocking::wait(&mut host.child, Duration::ZERO);
        }
        self.state.host_group = None;
        self.state.host_identity = None;
    }

    fn pending_request(&mut self) -> Result<Option<UpdateRequest>> {
        let Some(bytes) = fsx::read_optional(&self.layout.request())? else {
            return Ok(None);
        };
        let request = match serde_json::from_slice::<UpdateRequest>(&bytes) {
            Ok(request)
                if request.schema == REQUEST_SCHEMA
                    && descriptor::validate_request_id(&request.id).is_ok() =>
            {
                request
            }
            _ => {
                fsx::remove_file_if_present(&self.layout.request())?;
                return Ok(None);
            }
        };
        if self.state.last.request.as_deref() == Some(request.id.as_str()) {
            fsx::remove_file_if_present(&self.layout.request())?;
            return Ok(None);
        }
        Ok(Some(request))
    }

    fn collect_garbage(&mut self) -> Result<()> {
        let keep = match &self.state.phase {
            Phase::Idle => None,
            Phase::Prepared { snapshot, .. }
            | Phase::Trial { snapshot, .. }
            | Phase::RollingBack { snapshot, .. } => Some(snapshot.clone()),
        };
        for item in fs::read_dir(self.layout.snapshots())? {
            let item = item?;
            if keep.as_deref() != Some(item.file_name().to_string_lossy().as_ref()) {
                fsx::remove_tree_if_present(&item.path())?;
            }
        }
        for item in fs::read_dir(self.layout.run_dir())? {
            fsx::remove_tree_if_present(&item?.path())?;
        }
        Ok(())
    }

    fn save(&self) -> Result<()> {
        fsx::atomic_write(
            &self.layout.state(),
            &serde_json::to_vec_pretty(&self.state)?,
            0o600,
        )
    }

    /// Builds the descriptor for the current record and host.
    #[must_use]
    pub fn descriptor(&self, state: HostState) -> HostDescriptor {
        let ready = self.host.as_ref().and_then(|host| host.ready.as_ref());
        let mut capabilities: Vec<String> = LAUNCHER_CAPABILITIES
            .iter()
            .map(|flag| (*flag).to_string())
            .collect();
        if let Some(ready) = ready {
            capabilities.extend(ready.capabilities.iter().cloned());
        }
        capabilities.sort();
        capabilities.dedup();
        let update = match &self.state.phase {
            Phase::Idle => self.state.last.clone(),
            Phase::Prepared {
                request, from, to, ..
            } => UpdateView {
                from: Some(from.clone()),
                reason: None,
                request: Some(request.clone()),
                state: UpdateState::Prepared,
                target: Some(to.clone()),
            },
            // A rollback in progress still reads as the trial it ends; the
            // descriptor shows `rolled-back` once the restore is durable.
            Phase::Trial {
                request, from, to, ..
            }
            | Phase::RollingBack {
                request, from, to, ..
            } => UpdateView {
                from: Some(from.clone()),
                reason: None,
                request: Some(request.clone()),
                state: UpdateState::Trial,
                target: Some(to.clone()),
            },
        };
        HostDescriptor {
            capabilities,
            host_generation: self.state.generation,
            host_key: self.config.host_key.clone(),
            listen: self.config.listen.clone(),
            protocol_version: ready.map(|ready| ready.protocol_version),
            schema: DESCRIPTOR_SCHEMA.into(),
            state,
            update,
            version: Some(
                self.host
                    .as_ref()
                    .map_or_else(|| self.state.committed.clone(), |host| host.version.clone()),
            ),
        }
    }

    fn write_descriptor(&self, state: HostState) -> Result<()> {
        let bytes = self.descriptor(state).encode()?;
        fsx::atomic_write(&self.layout.descriptor(), &bytes, 0o600)
    }
}

fn view(
    state: UpdateState,
    request: &UpdateRequest,
    from: &str,
    reason: Option<String>,
) -> UpdateView {
    UpdateView {
        from: Some(from.into()),
        reason,
        request: Some(request.id.clone()),
        state,
        target: Some(request.target.clone()),
    }
}

/// Stops a host process group a previous launcher recorded and no longer
/// owns, but only when the group is still provably that host's.
///
/// The group identifier can be reused after that host exited, so the kill
/// is refused unless one of these holds:
/// - the group leader is alive and its identity (start time, and on Linux
///   the boot) equals the one recorded when the host started; or
/// - the group leader is gone but the group still has members. A process
///   identifier is never handed out while a process group of that number
///   exists, so such a group is the recorded host's leftover children.
///
/// A live leader with a different or unknown identity is some other
/// process that took the number; it is left alone. The launcher also
/// refuses its own group and group 1.
fn stop_orphan(group: i32, recorded: Option<&str>, grace: Duration) {
    // SAFETY: `getpgrp` takes no arguments and cannot fail.
    let own = unsafe { libc::getpgrp() };
    if group <= 1 || group == own || !supervise::running(group) {
        return;
    }
    if !orphan_is_ours(group, recorded) {
        eprintln!(
            "coder-service: not stopping process group {group}: its leader does not match the recorded host"
        );
        return;
    }
    // SAFETY: `killpg` reads two integers.
    unsafe { libc::killpg(group, libc::SIGTERM) };
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline && supervise::running(group) {
        std::thread::sleep(Duration::from_millis(25));
    }
    // SAFETY: as above.
    unsafe { libc::killpg(group, libc::SIGKILL) };
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && supervise::running(group) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether the live process group `group` is the host recorded with
/// `recorded` (see [`stop_orphan`]).
fn orphan_is_ours(group: i32, recorded: Option<&str>) -> bool {
    let Ok(leader) = u32::try_from(group) else {
        return false;
    };
    if !supervise::process_running(leader) {
        // Leaderless group: the number cannot have been reissued.
        return true;
    }
    match (recorded, process_identity(group)) {
        (Some(recorded), Some(live)) => recorded == live,
        _ => false,
    }
}

/// A string that names one process instance: the same process identifier
/// reused by another process yields a different string. `None` when the
/// process is gone or the platform offers no start time.
#[cfg(target_os = "linux")]
fn process_identity(pid: i32) -> Option<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name is parenthesized and may hold spaces or parentheses;
    // fields after the last `)` start at field 3 (`state`), so the start
    // time (field 22) is the 20th of them.
    let rest = &stat[stat.rfind(')')? + 1..];
    let start = rest.split_whitespace().nth(19)?;
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default();
    Some(format!("linux:{}:{start}", boot.trim()))
}

#[cfg(target_os = "macos")]
fn process_identity(pid: i32) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: the buffer is a zeroed `proc_bsdinfo` of exactly `size` bytes,
    // which is what `PROC_PIDTBSDINFO` fills.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    // SAFETY: the kernel filled the whole structure.
    let info = unsafe { info.assume_init() };
    Some(format!(
        "macos:{}.{:06}",
        info.pbi_start_tvsec, info.pbi_start_tvusec
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_identity(_pid: i32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests;
