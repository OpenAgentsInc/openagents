//! The build sweep (#12023): keeps the machine's floor of free disk while
//! agents build, with no agent session watching.
//!
//! The gate shares one target folder per workspace, but every worktree and
//! feature set still compiles new artifacts into it, and nothing removed the
//! old ones: on 2026-10-17 two shared targets grew to 328 and 208 GiB in four
//! hours. launchd runs `openagents-build-sweep` every five minutes
//! (`scripts/dev/install-build-sweep.sh`), and each run, in order:
//!
//! 1. Flags every target folder copy beyond the shared ones ([`Layout`]),
//!    and removes one that no build holds once nothing touched it for
//!    [`Policy::stray_idle`].
//! 2. Below the floor ([`Policy::floor`], 100 GiB free), removes whole
//!    target folders that no `cargo`, `rustc`, or `kache` process holds,
//!    copies first and then the least recently used, until the floor is
//!    back.
//! 3. In a target over its cap ([`Policy::cap`], 120 GiB), removes the least
//!    recently used artifacts in `deps`, `build`, and `incremental` that
//!    nothing read or wrote for [`Policy::idle`], down to three quarters of
//!    the cap.
//! 4. Still below the floor, does the same across every target, down to
//!    [`Policy::low_idle`], until the floor is back.
//!
//! A process holds a target when its `CARGO_TARGET_DIR`, its working
//! directory, or one of its arguments lies inside it, or when a build holds
//! one of a profile's cargo locks. When the process table can't be read,
//! every target counts as held. Artifacts go only while the sweep holds
//! their profile's cargo locks itself, so a build never loses a file
//! mid-compile: it waits the few seconds the removal takes, then rebuilds
//! what's missing.
//!
//! "Untouched" needs care on macOS, which updates a file's access time on a
//! read only when it is older than the file's change time: a new artifact,
//! whose two times are equal, never shows a read, and one read once keeps
//! that first read's time however often it is read again. So after each run
//! the sweep sets every artifact's access time to a second before its change
//! time, which makes the next read show, and it keeps each
//! artifact's last use in `~/.openagents/build-sweep.state.json`. Until the
//! sweep has watched an artifact for the idle time, it counts it as used
//! when the watching began, so a fresh install removes no artifact for
//! [`Policy::idle`].
//!
//! The owner's dev-app target
//! (`~/work/openagents-target-app`) is touched only below
//! [`Policy::critical`] (40 GiB free); agent scratch
//! (`~/.openagents/scratch`) and the paths in the disk janitor's keep file
//! never are, and no link is followed. Every removal and flag is a line in
//! `~/.openagents/logs/build-sweep.log` with the Chicago time, the size, the
//! path, and the reason.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::GIB;

/// Overrides the per-target cap, in GiB.
pub const CAP_VAR: &str = "OPENAGENTS_BUILD_SWEEP_CAP_GIB";
/// Overrides the free-disk floor, in GiB.
pub const FLOOR_VAR: &str = "OPENAGENTS_BUILD_SWEEP_FLOOR_GIB";
/// Overrides the critical level, in GiB.
pub const CRITICAL_VAR: &str = "OPENAGENTS_BUILD_SWEEP_CRITICAL_GIB";
/// Overrides how long an artifact sits untouched before a target over its
/// cap loses it, in minutes.
pub const IDLE_VAR: &str = "OPENAGENTS_BUILD_SWEEP_IDLE_MINUTES";
/// The file in the lease root that names extra target folders the gate
/// allows, one path per line.
pub const GRANTS_FILE: &str = "agent-target-grants";
/// The shared target folders in `~/work`: root crates, then app crates.
pub const SHARED: [&str; 2] = ["openagents-target-1", "openagents-target-1-app"];
/// The owner's dev-app target in `~/work`.
pub const DEV_APP: &str = "openagents-target-app";
/// The prefix of a target folder in `~/work`.
pub const PREFIX: &str = "openagents-target-";
/// The log is moved aside to `.1` past this size.
const LOG_LIMIT: u64 = 8 << 20;
/// The locks cargo holds on a profile folder while it builds there.
const LOCKS: [&str; 3] = [".cargo-lock", ".cargo-build-lock", ".cargo-artifact-lock"];

/// The commands whose `CARGO_TARGET_DIR`, working directory, and arguments
/// hold a target.
const BUILDERS: [&str; 7] = [
    "cargo",
    "rustc",
    "kache",
    "clippy-driver",
    "rustdoc",
    "cargo-clippy",
    "cargo-nextest",
];

/// The sizes and ages the sweep keeps to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Policy {
    /// The size, in bytes, above which a target loses its least recently
    /// used artifacts.
    pub cap: u64,
    /// The free space, in bytes, the machine keeps.
    pub floor: u64,
    /// The free space, in bytes, below which the dev-app target may go.
    pub critical: u64,
    /// How long an artifact sits untouched before a target over its cap
    /// loses it.
    pub idle: Duration,
    /// How long an artifact sits untouched before it goes below the floor.
    pub low_idle: Duration,
    /// How long a copy beyond the shared targets sits untouched before it
    /// goes.
    pub stray_idle: Duration,
    /// How long the sweep waits for a build to free a profile's cargo locks
    /// before it leaves that profile's artifacts to the next run.
    pub lock_wait: Duration,
}

impl Default for Policy {
    fn default() -> Policy {
        Policy {
            cap: 120 * GIB,
            floor: 100 * GIB,
            critical: 40 * GIB,
            idle: Duration::from_secs(2 * 3600),
            low_idle: Duration::from_secs(30 * 60),
            stray_idle: Duration::from_secs(3600),
            lock_wait: Duration::from_secs(20),
        }
    }
}

impl Policy {
    /// The default policy with [`CAP_VAR`], [`FLOOR_VAR`], [`CRITICAL_VAR`],
    /// and [`IDLE_VAR`] applied.
    #[must_use]
    pub fn from(env: &dyn Fn(&str) -> Option<String>) -> Policy {
        let number = |name: &str| env(name).and_then(|value| value.trim().parse::<u64>().ok());
        let mut policy = Policy::default();
        if let Some(gib) = number(CAP_VAR) {
            policy.cap = gib.saturating_mul(GIB);
        }
        if let Some(gib) = number(FLOOR_VAR) {
            policy.floor = gib.saturating_mul(GIB);
        }
        if let Some(gib) = number(CRITICAL_VAR) {
            policy.critical = gib.saturating_mul(GIB);
        }
        if let Some(minutes) = number(IDLE_VAR) {
            policy.idle = Duration::from_secs(minutes.saturating_mul(60));
        }
        policy
    }
}

/// Where the targets, the protected places, and the log are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// The folder of `openagents-target-*` folders, `~/work`.
    pub work: PathBuf,
    /// The gate's folder of shared targets, `~/work/agent-targets`.
    pub gate_targets: PathBuf,
    /// Target folders in [`Layout::work`] that are shared, not copies.
    pub shared: Vec<PathBuf>,
    /// The owner's dev-app target.
    pub dev_app: PathBuf,
    /// Places never touched: agent scratch and the keep file's paths.
    pub never: Vec<PathBuf>,
    /// The sweep's log.
    pub log: PathBuf,
    /// The last run's report, as JSON.
    pub last: PathBuf,
    /// When each artifact was last used, as the sweep has seen it.
    pub state: PathBuf,
    /// The file whose presence turns the sweep off.
    pub off: PathBuf,
}

impl Layout {
    /// The layout under `home`, with the gate's grants from the lease root
    /// `leases` and the disk janitor's keep file.
    #[must_use]
    pub fn at(home: &Path, leases: &Path) -> Layout {
        let work = home.join("work");
        let openagents = home.join(".openagents");
        let mut shared: Vec<PathBuf> = SHARED.iter().map(|name| work.join(name)).collect();
        shared.extend(paths_in(&leases.join(GRANTS_FILE), home));
        let mut never = vec![openagents.join("scratch")];
        never.extend(paths_in(&openagents.join("disk-janitor.keep"), home));
        Layout {
            gate_targets: work.join("agent-targets"),
            dev_app: work.join(DEV_APP),
            shared,
            never,
            log: openagents.join("logs/build-sweep.log"),
            last: openagents.join("logs/build-sweep.last.json"),
            state: openagents.join("build-sweep.state.json"),
            off: openagents.join("build-sweep.off"),
            work,
        }
    }

    pub(crate) fn kind(&self, path: &Path) -> Kind {
        if path == self.dev_app {
            Kind::DevApp
        } else if self.shared.iter().any(|shared| shared == path)
            || path.parent() == Some(self.gate_targets.as_path())
        {
            Kind::Shared
        } else {
            Kind::Copy
        }
    }

    pub(crate) fn protected(&self, path: &Path) -> bool {
        self.never
            .iter()
            .any(|never| path.starts_with(never) || never.starts_with(path))
    }

    /// The target folders on this machine: `openagents-target-*` in
    /// [`Layout::work`] and every folder in [`Layout::gate_targets`], never
    /// a link and never a protected place.
    #[must_use]
    pub fn targets(&self) -> Vec<(PathBuf, Kind)> {
        let mut found = Vec::new();
        for (dir, every) in [(&self.work, false), (&self.gate_targets, true)] {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let named = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(PREFIX));
                let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
                if (every || named) && is_dir && !self.protected(&path) {
                    let kind = self.kind(&path);
                    found.push((path, kind));
                }
            }
        }
        found.sort();
        found
    }
}

/// Paths named one a line in `file`, `~` meaning `home`; `#` starts a
/// comment.
fn paths_in(file: &Path, home: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .map(|line| match line.strip_prefix('~') {
            Some(rest) => home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(line),
        })
        .map(|path| path.components().collect::<PathBuf>())
        .collect()
}

/// What a target folder is to the sweep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A copy beyond the shared targets, removed first.
    Copy,
    /// A shared target.
    Shared,
    /// The owner's dev-app target, touched only when the disk is critical.
    DevApp,
}

/// A running build process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    /// The command's file name, such as `cargo`.
    pub name: String,
    pub args: Vec<String>,
    /// Its `CARGO_TARGET_DIR`, when set.
    pub target_dir: Option<PathBuf>,
    /// Its working directory, when known.
    pub cwd: Option<PathBuf>,
}

impl Proc {
    /// Whether this process holds `target`: its `CARGO_TARGET_DIR`
    /// (relative ones from its working directory), its working directory,
    /// or an argument (or the part after an argument's last `=`) lies
    /// inside.
    #[must_use]
    pub fn holds(&self, target: &Path) -> bool {
        let inside = |path: &Path| path.starts_with(target);
        let resolved = self.target_dir.as_ref().map(|dir| match &self.cwd {
            Some(cwd) if dir.is_relative() => cwd.join(dir),
            _ => dir.clone(),
        });
        resolved.as_deref().is_some_and(inside)
            || self.cwd.as_deref().is_some_and(inside)
            || self.args.iter().any(|arg| {
                let value = arg
                    .rsplit_once('=')
                    .map_or(arg.as_str(), |(_, value)| value);
                value.starts_with('/') && inside(Path::new(value))
            })
    }
}

/// Whether a command name is a build the sweep must not pull a target from
/// under: [`BUILDERS`] and build scripts.
#[must_use]
pub fn is_builder(name: &str) -> bool {
    BUILDERS.contains(&name) || name.starts_with("build-script-")
}

/// The build processes in `ps -axww -o pid=,command=` output (`plain`)
/// and the same with `-E` (`with_env`, which appends each environment),
/// without working directories yet.
#[must_use]
pub fn parse_ps(plain: &str, with_env: &str) -> Vec<Proc> {
    let split = |line: &str| -> Option<(u32, String)> {
        let line = line.trim_start();
        let (pid, rest) = line.split_once(char::is_whitespace)?;
        Some((pid.parse().ok()?, rest.trim().to_owned()))
    };
    let envs: BTreeMap<u32, String> = with_env.lines().filter_map(split).collect();
    let mut found = Vec::new();
    for (pid, command) in plain.lines().filter_map(split) {
        let args: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        let Some(first) = args.first() else {
            continue;
        };
        let name = first.rsplit('/').next().unwrap_or(first).to_owned();
        if !is_builder(&name) {
            continue;
        }
        let env = envs.get(&pid).map_or("", |line| {
            line.strip_prefix(command.as_str()).unwrap_or(line)
        });
        let target_dir = env
            .split_whitespace()
            .filter_map(|token| token.strip_prefix("CARGO_TARGET_DIR="))
            .next_back()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        found.push(Proc {
            pid,
            name,
            args,
            target_dir,
            cwd: None,
        });
    }
    found
}

/// Sets each process's working directory from `lsof -a -d cwd -p PIDS -Fpn`
/// output.
pub fn add_cwds(procs: &mut [Proc], lsof: &str) {
    let mut cwds = BTreeMap::new();
    let mut pid = 0u32;
    for line in lsof.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            pid = rest.trim().parse().unwrap_or(0);
        } else if let Some(name) = line.strip_prefix('n')
            && pid != 0
        {
            cwds.insert(pid, PathBuf::from(name));
        }
    }
    for proc in procs {
        if let Some(cwd) = cwds.get(&proc.pid) {
            proc.cwd = Some(cwd.clone());
        }
    }
}

/// What the sweep needs from the machine.
pub trait Host {
    /// The running build processes.
    ///
    /// # Errors
    /// The process table can't be read; every target then counts as held.
    fn processes(&self) -> Result<Vec<Proc>, String>;
    /// Free bytes on the volume holding `path`.
    ///
    /// # Errors
    /// The volume can't be read.
    fn free(&self, path: &Path) -> std::io::Result<u64>;
    /// Now.
    fn now(&self) -> SystemTime;
    /// `at` as the log shows it, in Chicago time.
    fn stamp(&self, at: SystemTime) -> String;
}

/// This Mac: `ps` and `lsof`, `statvfs`, and the clock.
pub struct System;

impl Host for System {
    fn processes(&self) -> Result<Vec<Proc>, String> {
        let run = |program: &str, args: &[&str]| -> Result<String, String> {
            let output = std::process::Command::new(program)
                .args(args)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .map_err(|error| format!("{program}: {error}"))?;
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        };
        let plain = run("ps", &["-axww", "-o", "pid=,command="])?;
        if plain.trim().is_empty() {
            return Err("ps listed nothing".into());
        }
        let with_env = run("ps", &["-axww", "-E", "-o", "pid=,command="])?;
        let mut procs = parse_ps(&plain, &with_env);
        if !procs.is_empty() {
            let pids: Vec<String> = procs.iter().map(|proc| proc.pid.to_string()).collect();
            // `-a` ANDs the filters: only these processes' working
            // directories. Without it lsof ORs them.
            let lsof = run("lsof", &["-a", "-d", "cwd", "-p", &pids.join(","), "-Fpn"])?;
            add_cwds(&mut procs, &lsof);
        }
        Ok(procs)
    }

    fn free(&self, path: &Path) -> std::io::Result<u64> {
        crate::free_disk(path)
    }

    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn stamp(&self, at: SystemTime) -> String {
        chicago(at)
    }
}

/// `at` in Chicago time, such as `2026-10-17 6:41:03 PM CDT`. The zone's
/// offset and name come from `date` once a process; UTC when it can't run.
#[must_use]
pub fn chicago(at: SystemTime) -> String {
    static ZONE: std::sync::OnceLock<Option<(i64, String)>> = std::sync::OnceLock::new();
    let zone = ZONE.get_or_init(|| {
        let output = std::process::Command::new("/bin/date")
            .env("TZ", "America/Chicago")
            .arg("+%z %Z")
            .output()
            .ok()
            .filter(|output| output.status.success())?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let (offset, name) = text.split_once(' ')?;
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let digits = offset.trim_start_matches(['+', '-']);
        let hours: i64 = digits.get(..2)?.parse().ok()?;
        let minutes: i64 = digits.get(2..4)?.parse().ok()?;
        Some((sign * (hours * 3600 + minutes * 60), name.to_owned()))
    });
    let seconds = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
    match zone {
        Some((offset, name)) => local_time(seconds + offset, name),
        None => local_time(seconds, "UTC"),
    }
}

/// Seconds since 1970 on a zone's clock as `2026-10-17 6:41:03 PM CDT`.
#[must_use]
pub fn local_time(seconds: i64, zone: &str) -> String {
    let days = seconds.div_euclid(86_400);
    let second = seconds.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (hour, minute, sec) = (second / 3600, second / 60 % 60, second % 60);
    let half = if hour < 12 { "AM" } else { "PM" };
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{year}-{month:02}-{day:02} {twelve}:{minute:02}:{sec:02} {half} {zone}")
}

/// A group of artifacts removed together: one crate unit's files in
/// `deps` with its fingerprint, one `build` folder with its fingerprint, or
/// one incremental session.
#[derive(Clone, Debug)]
struct Unit {
    paths: Vec<PathBuf>,
    /// The path the log shows: `deps/libfoo-1a2b.*` for a crate unit.
    shown: PathBuf,
    /// The profile folder whose cargo locks cover it.
    profile: PathBuf,
    bytes: u64,
    used: SystemTime,
    /// Files whose next read wouldn't show yet ([`rearm`]).
    arm: Vec<PathBuf>,
}

impl Unit {
    fn new(profile: &Path) -> Unit {
        Unit {
            paths: Vec::new(),
            shown: PathBuf::new(),
            profile: profile.to_owned(),
            bytes: 0,
            used: SystemTime::UNIX_EPOCH,
            arm: Vec::new(),
        }
    }

    fn add(&mut self, path: PathBuf, measured: Measured) {
        if self.paths.is_empty() {
            self.shown.clone_from(&path);
        }
        self.paths.push(path);
        self.bytes += measured.bytes;
        self.used = self.used.max(measured.used);
        self.arm.extend(measured.arm);
    }

    fn key(&self) -> String {
        self.paths
            .first()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    }
}

/// When each artifact was last used, kept between runs.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Watch {
    /// When the sweep began watching, in Unix seconds.
    since: u64,
    /// Each unit's last use, in Unix seconds, by its first path.
    used: BTreeMap<String, u64>,
}

fn unix(at: SystemTime) -> u64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn at_unix(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

/// One target folder as the sweep measured it.
#[derive(Clone, Debug)]
struct Target {
    path: PathBuf,
    kind: Kind,
    bytes: u64,
    used: SystemTime,
    units: Vec<Unit>,
    holder: Option<String>,
    gone: bool,
}

/// What the sweep did, or would do, to one path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Action {
    /// `removed`, `would remove`, or `flagged`.
    pub verb: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub reason: String,
    /// When, in Chicago time.
    pub at: String,
}

/// A target's size after the run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TargetSize {
    pub path: PathBuf,
    pub kind: Kind,
    pub bytes: u64,
    /// What holds it, such as `cargo 4211`, when something does.
    pub holder: Option<String>,
}

/// One run of the sweep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Report {
    /// When it started, in Chicago time.
    pub at: String,
    pub dry_run: bool,
    /// Free bytes when it started, when the volume could be read.
    pub free_before: Option<u64>,
    /// Free bytes after, estimated as before plus what it removed.
    pub free_after: Option<u64>,
    pub freed: u64,
    /// When the sweep began watching artifact use, in Chicago time.
    pub watching_since: String,
    pub policy: Policy,
    pub actions: Vec<Action>,
    pub targets: Vec<TargetSize>,
}

impl Report {
    /// The report as log lines: one for the run, then one per action.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let free = |bytes: Option<u64>| bytes.map_or_else(|| "unknown".to_owned(), size);
        let sizes: Vec<String> = self
            .targets
            .iter()
            .map(|target| {
                let name = target.path.file_name().map_or_else(
                    || target.path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                match &target.holder {
                    Some(holder) => format!("{name} {} (held by {holder})", size(target.bytes)),
                    None => format!("{name} {}", size(target.bytes)),
                }
            })
            .collect();
        let mut lines = vec![format!(
            "{}  {}  free {} -> {}, freed {}; artifact use watched since {}; targets: {}",
            self.at,
            if self.dry_run { "dry run" } else { "sweep" },
            free(self.free_before),
            free(self.free_after),
            size(self.freed),
            self.watching_since,
            if sizes.is_empty() {
                "none".to_owned()
            } else {
                sizes.join(", ")
            }
        )];
        lines.extend(self.actions.iter().map(|action| {
            format!(
                "{}  {}  {}  {}  ({})",
                action.at,
                action.verb,
                size(action.bytes),
                action.path.display(),
                action.reason
            )
        }));
        lines
    }
}

/// Bytes as a person reads them: `12.3 GiB`, `512 MiB`, `4 KiB`.
#[must_use]
pub fn size(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let gib = bytes as f64 / GIB as f64;
    if bytes >= GIB {
        format!("{gib:.1} GiB")
    } else if bytes >= 1 << 20 {
        format!("{} MiB", bytes >> 20)
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    }
}

fn ago(now: SystemTime, then: SystemTime) -> Duration {
    now.duration_since(then).unwrap_or_default()
}

/// A duration as `5h`, `40m`.
fn span(duration: Duration) -> String {
    let minutes = duration.as_secs() / 60;
    if minutes >= 120 {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

/// One sweep over `layout` under `policy`. With `dry_run` it removes
/// nothing and reports what it would remove.
#[must_use]
pub fn sweep(layout: &Layout, policy: &Policy, host: &dyn Host, dry_run: bool) -> Report {
    let now = host.now();
    let free_before = host.free(&layout.work).ok();
    let procs = host.processes();
    let mut targets: Vec<Target> = layout
        .targets()
        .into_iter()
        .map(|(path, kind)| {
            let holder = match &procs {
                Ok(procs) => procs
                    .iter()
                    .find(|proc| proc.holds(&path))
                    .map(|proc| format!("{} {}", proc.name, proc.pid))
                    .or_else(|| cargo_locked(&path).then(|| "a build's cargo lock".to_owned())),
                Err(error) => Some(format!("unknown, the process table can't be read: {error}")),
            };
            scan(path, kind, holder, layout)
        })
        .collect();
    let watch = std::fs::read(&layout.state)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Watch>(&bytes).ok())
        .unwrap_or_else(|| Watch {
            since: unix(now),
            used: BTreeMap::new(),
        });
    for unit in targets
        .iter_mut()
        .flat_map(|target| target.units.iter_mut())
    {
        let known = watch.used.get(&unit.key()).copied().unwrap_or(0);
        unit.used = unit.used.max(at_unix(known.max(watch.since)));
    }
    let mut run = Run {
        layout,
        host,
        dry_run,
        lock_wait: policy.lock_wait,
        free: free_before,
        freed: 0,
        actions: Vec::new(),
        locks: BTreeMap::new(),
    };
    let critical = free_before.is_some_and(|free| free < policy.critical);
    let touchable = |target: &Target| target.kind != Kind::DevApp || critical;

    // 1. Copies beyond the shared targets.
    for target in targets
        .iter_mut()
        .filter(|target| target.kind == Kind::Copy)
    {
        let idle = ago(now, target.used);
        if let Some(holder) = &target.holder {
            let reason = format!(
                "a target copy beyond the shared ones, held by {holder}; build in the shared targets"
            );
            run.flag(&target.path, target.bytes, reason);
        } else if idle >= policy.stray_idle && !cargo_locked(&target.path) {
            let reason = format!(
                "a target copy beyond the shared ones, untouched {}",
                span(idle)
            );
            run.remove_target(target, reason);
        } else {
            let reason = format!(
                "a target copy beyond the shared ones, untouched {}; removed once untouched {} and no build holds it",
                span(idle),
                span(policy.stray_idle)
            );
            run.flag(&target.path, target.bytes, reason);
        }
    }

    // 2. Below the floor: whole targets no build holds.
    let mut order: Vec<usize> = (0..targets.len())
        .filter(|&i| !targets[i].gone && targets[i].holder.is_none() && touchable(&targets[i]))
        .collect();
    order.sort_by_key(|&i| (targets[i].kind, targets[i].used));
    for i in order {
        let Some(free) = run.free.filter(|&free| free < policy.floor) else {
            break;
        };
        if cargo_locked(&targets[i].path) {
            continue;
        }
        let reason = format!(
            "{} free is below the {} floor; no build holds it",
            size(free),
            size(policy.floor)
        );
        run.remove_target(&mut targets[i], reason);
    }

    // 3. Targets over their cap: their least recently used artifacts.
    let low_water = policy.cap / 4 * 3;
    for target in targets
        .iter_mut()
        .filter(|target| !target.gone && touchable(target))
    {
        if target.bytes <= policy.cap {
            continue;
        }
        let held = target.bytes;
        let mut units = std::mem::take(&mut target.units);
        units.sort_by_key(|unit| unit.used);
        let mut kept = Vec::new();
        for unit in units {
            let idle = ago(now, unit.used);
            if target.bytes > low_water && idle >= policy.idle {
                let reason = format!(
                    "the target held {}, over its {} cap; untouched {}",
                    size(held),
                    size(policy.cap),
                    span(idle)
                );
                if run.remove_unit(&unit, reason) {
                    target.bytes = target.bytes.saturating_sub(unit.bytes);
                } else {
                    kept.push(unit);
                }
            } else {
                kept.push(unit);
            }
        }
        target.units = kept;
    }

    // 4. Still below the floor: the least recently used artifacts anywhere.
    if run.free.is_some_and(|free| free < policy.floor) {
        let mut units: Vec<(Kind, Unit)> = targets
            .iter_mut()
            .filter(|target| !target.gone && touchable(target))
            .flat_map(|target| {
                let kind = target.kind;
                std::mem::take(&mut target.units)
                    .into_iter()
                    .map(move |unit| (kind, unit))
            })
            .collect();
        units.sort_by_key(|(kind, unit)| (*kind == Kind::DevApp, unit.used));
        let mut kept = Vec::new();
        for (kind, unit) in units {
            let idle = ago(now, unit.used);
            let Some(free) = run.free.filter(|&free| free < policy.floor) else {
                kept.push((kind, unit));
                continue;
            };
            if idle < policy.low_idle {
                kept.push((kind, unit));
                continue;
            }
            let reason = format!(
                "{} free is below the {} floor; untouched {}",
                size(free),
                size(policy.floor),
                span(idle)
            );
            if !run.remove_unit(&unit, reason) {
                kept.push((kind, unit));
                continue;
            }
            if let Some(target) = targets
                .iter_mut()
                .find(|target| unit.paths[0].starts_with(&target.path))
            {
                target.bytes = target.bytes.saturating_sub(unit.bytes);
            }
        }
        for (_, unit) in kept {
            if let Some(target) = targets
                .iter_mut()
                .find(|target| unit.paths[0].starts_with(&target.path))
            {
                target.units.push(unit);
            }
        }
    }

    // Builds wait no longer than the removals took.
    run.locks.clear();
    if !dry_run {
        let mut used = BTreeMap::new();
        for target in targets.iter().filter(|target| !target.gone) {
            for unit in &target.units {
                used.insert(unit.key(), unix(unit.used));
                if touchable(target) {
                    unit.arm.iter().for_each(|path| rearm(path));
                }
            }
        }
        let watch = Watch {
            since: watch.since,
            used,
        };
        if let Err(error) = save(&layout.state, &watch) {
            let reason = format!("the sweep's record of artifact use can't be written: {error}");
            run.flag(&layout.state, 0, reason);
        }
    }

    Report {
        at: host.stamp(now),
        dry_run,
        free_before,
        free_after: run.free,
        freed: run.freed,
        watching_since: host.stamp(at_unix(watch.since)),
        policy: policy.clone(),
        actions: run.actions,
        targets: targets
            .into_iter()
            .filter(|target| !target.gone)
            .map(|target| TargetSize {
                path: target.path,
                kind: target.kind,
                bytes: target.bytes,
                holder: target.holder,
            })
            .collect(),
    }
}

struct Run<'a> {
    layout: &'a Layout,
    host: &'a dyn Host,
    dry_run: bool,
    lock_wait: Duration,
    free: Option<u64>,
    freed: u64,
    actions: Vec<Action>,
    /// The cargo locks the sweep holds, by profile; `None` when a build
    /// kept them.
    locks: BTreeMap<PathBuf, Option<Vec<std::fs::File>>>,
}

impl Run<'_> {
    fn flag(&mut self, path: &Path, bytes: u64, reason: String) {
        self.push("flagged", path, bytes, reason);
    }

    fn push(&mut self, verb: &str, path: &Path, bytes: u64, reason: String) {
        self.actions.push(Action {
            verb: verb.to_owned(),
            path: path.to_owned(),
            bytes,
            reason,
            at: self.host.stamp(self.host.now()),
        });
    }

    /// Removes `path`, unless it is protected or a link, and counts it.
    fn remove(&mut self, path: &Path, bytes: u64, reason: String) -> bool {
        if self.layout.protected(path) || !path.starts_with(&self.layout.work) {
            return false;
        }
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return false;
        };
        if !self.dry_run {
            let removed = if meta.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            if let Err(error) = removed {
                self.push(
                    "failed to remove",
                    path,
                    bytes,
                    format!("{reason}; {error}"),
                );
                return false;
            }
        }
        self.freed += bytes;
        self.free = self.free.map(|free| free + bytes);
        let verb = if self.dry_run {
            "would remove"
        } else {
            "removed"
        };
        self.push(verb, path, bytes, reason);
        true
    }

    fn remove_target(&mut self, target: &mut Target, reason: String) {
        let path = target.path.clone();
        if self.remove(&path, target.bytes, reason) {
            target.gone = true;
        }
    }

    /// Whether the sweep holds `profile`'s cargo locks, taking them when it
    /// doesn't yet. A dry run takes none.
    fn locked(&mut self, profile: &Path) -> bool {
        if self.dry_run {
            return true;
        }
        if !self.locks.contains_key(profile) {
            let guard = lock_profile(profile, self.lock_wait);
            if guard.is_none() {
                let reason = format!(
                    "a build held its cargo lock the {}s the sweep waited; its artifacts wait for the next run",
                    self.lock_wait.as_secs()
                );
                self.push("skipped", profile, 0, reason);
            }
            self.locks.insert(profile.to_owned(), guard);
        }
        self.locks.get(profile).is_some_and(Option::is_some)
    }

    /// Removes a unit's files under its profile's cargo locks, and whether
    /// it did.
    fn remove_unit(&mut self, unit: &Unit, reason: String) -> bool {
        let Some(first) = unit.paths.first() else {
            return false;
        };
        if self.layout.protected(first) || !self.locked(&unit.profile) {
            return false;
        }
        let mut removed = 0;
        for path in &unit.paths {
            if self.dry_run || self.layout.protected(path) {
                continue;
            }
            let gone = match std::fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
                Ok(_) => std::fs::remove_file(path),
                Err(_) => Ok(()),
            };
            if gone.is_ok() {
                removed += 1;
            }
        }
        if !self.dry_run && removed == 0 {
            return false;
        }
        self.freed += unit.bytes;
        self.free = self.free.map(|free| free + unit.bytes);
        let verb = if self.dry_run {
            "would remove"
        } else {
            "removed"
        };
        self.push(verb, &unit.shown, unit.bytes, reason);
        true
    }
}

/// Whether a build holds a cargo lock in one of `target`'s profiles.
pub(crate) fn cargo_locked(target: &Path) -> bool {
    profiles(target)
        .iter()
        .flat_map(|profile| LOCKS.iter().map(|name| profile.join(name)))
        .any(|lock| lock_held(&lock))
}

/// Takes every cargo lock of `profile` without blocking, retrying until
/// `wait` has passed; `None` when a build kept one that long. Holding none
/// while it waits, it can't deadlock with a build taking them in another
/// order.
#[cfg(unix)]
fn lock_profile(profile: &Path, wait: Duration) -> Option<Vec<std::fs::File>> {
    use std::os::unix::io::AsRawFd as _;
    let deadline = std::time::Instant::now() + wait;
    loop {
        let mut held = Vec::new();
        let mut busy = false;
        for name in LOCKS {
            let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(profile.join(name))
            else {
                continue;
            };
            // SAFETY: `flock` on a descriptor `held` keeps open.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                busy = true;
                break;
            }
            held.push(file);
        }
        if !busy {
            return Some(held);
        }
        drop(held);
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(not(unix))]
fn lock_profile(_: &Path, _: Duration) -> Option<Vec<std::fs::File>> {
    Some(Vec::new())
}

/// Sets a file's access time to a second before its change time, so that
/// macOS records its next read.
fn rearm(path: &Path) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    let (true, Ok(modified)) = (meta.is_file(), meta.modified()) else {
        return;
    };
    if let Ok(file) = std::fs::File::open(path) {
        let before = modified - Duration::from_secs(1);
        let _ = file.set_times(std::fs::FileTimes::new().set_accessed(before));
    }
}

fn save(path: &Path, watch: &Watch) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec(watch).map_err(std::io::Error::other)?;
    let staging = path.with_extension(format!("json.{}.new", std::process::id()));
    std::fs::write(&staging, json)?;
    std::fs::rename(&staging, path)
}

#[cfg(unix)]
fn lock_held(path: &Path) -> bool {
    use std::os::unix::io::AsRawFd as _;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    // SAFETY: `flock` on a descriptor this function owns for the call.
    let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } == 0;
    if taken {
        // SAFETY: as above; closing the file would release it as well.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
    }
    !taken
}

#[cfg(not(unix))]
fn lock_held(_: &Path) -> bool {
    false
}

/// A target's profile folders: `debug`, `release`, and
/// `<triple>/<profile>`, the folders that hold `deps` or `.fingerprint`.
fn profiles(target: &Path) -> Vec<PathBuf> {
    let real_dir = |path: PathBuf| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir());
    let is_profile = |dir: &Path| real_dir(dir.join("deps")) || real_dir(dir.join(".fingerprint"));
    let mut found = Vec::new();
    for child in dirs(target) {
        if is_profile(&child) {
            found.push(child);
        } else {
            found.extend(dirs(&child).into_iter().filter(|dir| is_profile(dir)));
        }
    }
    found
}

/// The real folders (not links) directly in `dir`.
fn dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect();
    found.sort();
    found
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    found.sort();
    found
}

/// What [`measure`] found under a path.
#[derive(Clone, Debug)]
struct Measured {
    bytes: u64,
    used: SystemTime,
    /// Files whose access time isn't older than their change time, so a
    /// read wouldn't show.
    arm: Vec<PathBuf>,
}

/// The bytes under `path` (a hard-linked file's blocks shared among its
/// links) and when a file in it was last read or written. Folders' times
/// are left out, since listing one changes its access time, except an empty
/// folder's own change time. Links aren't followed.
fn measure(path: &Path) -> Measured {
    let mut found = Measured {
        bytes: 0,
        used: SystemTime::UNIX_EPOCH,
        arm: Vec::new(),
    };
    let mut stack = vec![path.to_owned()];
    let own = std::fs::symlink_metadata(path).and_then(|meta| meta.modified());
    while let Some(path) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        found.bytes += allocated(&meta);
        if meta.is_dir() {
            stack.extend(entries(&path));
            continue;
        }
        if let (Ok(modified), Ok(accessed)) = (meta.modified(), meta.accessed()) {
            found.used = found.used.max(modified).max(accessed);
            if accessed >= modified {
                found.arm.push(path);
            }
        }
    }
    if found.used == SystemTime::UNIX_EPOCH
        && let Ok(own) = own
    {
        found.used = own;
    }
    found
}

/// The bytes under `path`, as [`measure`] counts them.
pub(crate) fn bytes_under(path: &Path) -> u64 {
    measure(path).bytes
}

#[cfg(unix)]
fn allocated(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    let links = if meta.is_dir() {
        1
    } else {
        meta.nlink().max(1)
    };
    meta.blocks().saturating_mul(512) / links
}

#[cfg(not(unix))]
fn allocated(meta: &std::fs::Metadata) -> u64 {
    meta.len()
}

/// The name a `deps` file shares with its fingerprint: `libfoo-1a2b.rlib`
/// and `foo-1a2b.d` are both `foo-1a2b`.
fn unit_name(file: &str) -> String {
    let stem = file.split('.').next().unwrap_or(file);
    let library = [".rlib", ".rmeta", ".dylib", ".so", ".a", ".dll"]
        .iter()
        .any(|ext| file.ends_with(ext));
    match stem.strip_prefix("lib") {
        Some(rest) if library && !rest.is_empty() => rest.to_owned(),
        _ => stem.to_owned(),
    }
}

/// Measures a target and groups its artifacts into units.
fn scan(path: PathBuf, kind: Kind, holder: Option<String>, layout: &Layout) -> Target {
    let mut target = Target {
        path,
        kind,
        bytes: 0,
        used: SystemTime::UNIX_EPOCH,
        units: Vec::new(),
        holder,
        gone: false,
    };
    let profiles = profiles(&target.path);
    let mut loose = vec![target.path.clone()];
    // The target folder itself and everything outside the profiles.
    let mut stack = Vec::new();
    while let Some(dir) = loose.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            continue;
        };
        target.bytes += allocated(&meta);
        for child in entries(&dir) {
            if profiles.contains(&child) {
                continue;
            }
            let is_dir = std::fs::symlink_metadata(&child).is_ok_and(|meta| meta.is_dir());
            if is_dir && profiles.iter().any(|profile| profile.starts_with(&child)) {
                loose.push(child);
            } else {
                stack.push(child);
            }
        }
    }
    for path in stack {
        let measured = measure(&path);
        target.bytes += measured.bytes;
        target.used = target.used.max(measured.used);
    }
    for profile in profiles {
        scan_profile(&profile, &mut target, layout);
    }
    target
}

fn scan_profile(profile: &Path, target: &mut Target, layout: &Layout) {
    let name_of = |path: &Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut fingerprints: BTreeMap<String, (PathBuf, Measured)> = BTreeMap::new();
    for path in entries(&profile.join(".fingerprint")) {
        let measured = measure(&path);
        fingerprints.insert(name_of(&path), (path, measured));
    }
    let mut units: Vec<(Option<String>, Unit)> = Vec::new();
    let mut deps: BTreeMap<String, Unit> = BTreeMap::new();
    for child in entries(profile) {
        let name = name_of(&child);
        let is_dir = std::fs::symlink_metadata(&child).is_ok_and(|meta| meta.is_dir());
        if name == ".fingerprint" {
            continue;
        } else if name == "deps" && is_dir {
            for file in entries(&child) {
                let measured = measure(&file);
                let unit = deps
                    .entry(unit_name(&name_of(&file)))
                    .or_insert_with(|| Unit::new(profile));
                unit.add(file, measured);
            }
        } else if name == "build" && is_dir {
            for dir in entries(&child) {
                let mut unit = Unit::new(profile);
                let key = name_of(&dir);
                unit.add(dir.clone(), measure(&dir));
                units.push((Some(key), unit));
            }
        } else if name.starts_with("incremental") && is_dir {
            for session in entries(&child) {
                // kache keeps its sessions one level down, in `v1/`.
                let nested = !name_of(&session).contains('-')
                    && std::fs::symlink_metadata(&session).is_ok_and(|meta| meta.is_dir());
                let sessions = if nested {
                    entries(&session)
                } else {
                    vec![session]
                };
                for session in sessions {
                    let mut unit = Unit::new(profile);
                    unit.add(session.clone(), measure(&session));
                    units.push((None, unit));
                }
            }
        } else {
            let measured = measure(&child);
            target.bytes += measured.bytes;
            target.used = target.used.max(measured.used);
        }
    }
    for unit in deps.values_mut() {
        // A crate unit's files share the name before the first `.`.
        let stem = name_of(&unit.shown)
            .split('.')
            .next()
            .unwrap_or("")
            .to_owned();
        unit.shown = unit.shown.with_file_name(format!("{stem}.*"));
    }
    units.extend(deps.into_iter().map(|(name, unit)| (Some(name), unit)));
    for (name, mut unit) in units {
        if let Some((path, measured)) = name.and_then(|name| fingerprints.remove(&name)) {
            unit.add(path, measured);
        }
        target.bytes += unit.bytes;
        target.used = target.used.max(unit.used);
        if !unit.paths.iter().any(|path| layout.protected(path)) {
            target.units.push(unit);
        }
    }
    for (_, (_, measured)) in fingerprints {
        target.bytes += measured.bytes;
        target.used = target.used.max(measured.used);
    }
    if let Ok(meta) = std::fs::symlink_metadata(profile) {
        target.bytes += allocated(&meta);
    }
}

/// Appends the report to the log, moving a log past 8 MiB aside first, and
/// writes it as the last run.
///
/// # Errors
/// The log can't be written.
pub fn record(layout: &Layout, report: &Report) -> std::io::Result<()> {
    use std::io::Write as _;
    if let Some(dir) = layout.log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(&layout.log).is_ok_and(|meta| meta.len() > LOG_LIMIT) {
        let _ = std::fs::rename(&layout.log, layout.log.with_extension("log.1"));
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&layout.log)?;
    for line in report.lines() {
        writeln!(log, "{line}")?;
    }
    let json = serde_json::to_vec_pretty(report).map_err(std::io::Error::other)?;
    let staging = layout
        .last
        .with_extension(format!("json.{}.new", std::process::id()));
    std::fs::write(&staging, json)?;
    std::fs::rename(&staging, &layout.last)
}

#[cfg(all(test, unix))]
mod tests;
