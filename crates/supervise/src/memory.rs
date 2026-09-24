//! A memory cap on one job.
//!
//! A job with a cap runs in a transient systemd scope of its own, and the
//! scope's cgroup holds the whole tree to `MemoryMax`. A cgroup is the
//! right unit for this, and a resource limit on the process is not:
//!
//! - It counts the tree. `RLIMIT_AS` and `RLIMIT_DATA` are per process and
//!   inherited, so a shell that starts four compilers gives each of them
//!   the whole cap, and together they can take four times it.
//! - It counts memory in use, not address space. A JavaScript engine, the
//!   Go runtime, and CUDA reserve tens of gigabytes they never touch, and
//!   `RLIMIT_AS` fails them at start.
//! - It reports. The kernel kills inside the cgroup and systemd records the
//!   scope's result as `oom-kill`, so a job ended by its cap is told apart
//!   from one that crashed. A process past a resource limit sees a failed
//!   allocation instead, and what it does next — an abort, an exception, an
//!   exit code of its own choosing — reads the same as any other failure.
//!
//! # Getting the job into the scope before it runs
//!
//! systemd creates a scope around processes that already exist, so the
//! child has to exist before it can be moved, and it must not run the
//! program until it has been: a shell forks its first command within a
//! millisecond, and a process forked before the move stays outside the
//! cap. The standard library's spawn does not return until the child has
//! executed the program, so the handshake runs through two pipes the child
//! uses between `fork` and `exec`:
//!
//! 1. The child writes its process identifier to the report pipe and waits
//!    on the gate pipe.
//! 2. A helper thread reads the identifier, asks the user's systemd manager
//!    for the scope over D-Bus with `busctl`, and waits until
//!    `/proc/<pid>/cgroup` names it and the cgroup has a `memory.max`.
//! 3. The helper opens the gate. The child executes the program inside the
//!    scope, so everything it starts is counted.
//!
//! Rewrapping the command in `systemd-run --scope` would be shorter, but
//! the wrapper runs the program with its own environment, and a prepared
//! command's `env_clear` cannot be read back to rebuild it. The handshake
//! leaves the command's environment policy as the caller set it.
//!
//! # Where a scope is not available
//!
//! No user manager, no `busctl`, a controller the manager was not
//! delegated, or a platform without systemd: the helper tells the child so,
//! and the child sets `RLIMIT_DATA` to the cap before it executes the
//! program, or to an enclosing cgroup's `memory.max` when that is larger,
//! since inside a container the container's limit already protects the
//! host. That still stops a runaway process, but only per process, and a
//! job it ends is not told apart from one that failed. [`Memory`] says
//! which of the two held the job. `SUPERVISE_MEMORY_SCOPE=off` forces the
//! fallback.

#![cfg_attr(not(feature = "job"), allow(dead_code))]

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::GRACE;

/// The memory one job may take when neither the caller nor
/// [`MEMORY_ENV`] says otherwise: 16 GiB.
pub const MEMORY_MAX: u64 = 16 * 1024 * 1024 * 1024;

/// The variable that overrides [`MEMORY_MAX`] for every job this process
/// supervises. It takes a byte count with an optional `K`, `M`, `G`, or
/// `T` suffix (powers of 1024), or `none` for no cap.
pub const MEMORY_ENV: &str = "SUPERVISE_MEMORY_MAX";

/// The variable that, set to `off`, keeps jobs out of systemd scopes and
/// caps each process with `RLIMIT_DATA` instead.
pub const SCOPE_ENV: &str = "SUPERVISE_MEMORY_SCOPE";

/// How long a `busctl` or `systemctl` call, or the move into a scope, may
/// take before the job falls back to the resource limit.
const HELPER_WALL: Duration = Duration::from_secs(3);

/// How long the supervisor waits, after a job's tree is gone, for systemd
/// to settle the scope's result.
const SETTLE_WALL: Duration = Duration::from_secs(2);

/// What held one job's memory, and whether the job ran into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    /// The cap, in bytes.
    pub max: u64,
    /// What enforced it.
    pub enforcement: Enforcement,
    /// Whether the kernel killed a process in the job for passing the cap.
    /// Only a scope can say so; under [`Enforcement::DataLimit`] this is
    /// always `false`, and a job its limit ended reads as a failure.
    pub exceeded: bool,
}

/// What enforced a job's memory cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enforcement {
    /// A transient systemd scope, named here, held the job's whole tree to
    /// the cap.
    Scope(String),
    /// Each process in the job had `RLIMIT_DATA` set to the cap.
    DataLimit,
}

/// The cap a job gets when its caller names none: [`MEMORY_ENV`] when it
/// is set and reads, [`MEMORY_MAX`] otherwise.
#[must_use]
pub fn default_max() -> Option<u64> {
    static DEFAULT: OnceLock<Option<u64>> = OnceLock::new();
    *DEFAULT.get_or_init(|| match std::env::var(MEMORY_ENV) {
        Ok(text) => parse_bytes(&text).unwrap_or(Some(MEMORY_MAX)),
        Err(_) => Some(MEMORY_MAX),
    })
}

/// Reads a byte count: a number with an optional `K`, `M`, `G`, or `T`
/// suffix (powers of 1024, case ignored, an optional trailing `B` or
/// `iB`), or `none`, `off`, or `0` for no cap.
///
/// # Errors
///
/// Returns a message when the text is neither.
pub fn parse_bytes(text: &str) -> Result<Option<u64>, String> {
    let text = text.trim();
    if matches!(text.to_ascii_lowercase().as_str(), "none" | "off" | "0") {
        return Ok(None);
    }
    let lower = text.to_ascii_lowercase();
    let unit = lower.trim_end_matches("ib").trim_end_matches('b');
    let (digits, shift) = match unit.chars().last() {
        Some('k') => (&unit[..unit.len() - 1], 10),
        Some('m') => (&unit[..unit.len() - 1], 20),
        Some('g') => (&unit[..unit.len() - 1], 30),
        Some('t') => (&unit[..unit.len() - 1], 40),
        _ => (unit, 0),
    };
    let count: u64 = digits
        .trim()
        .parse()
        .map_err(|_| format!("`{text}` is not a byte count"))?;
    let bytes = count
        .checked_mul(1u64 << shift)
        .ok_or_else(|| format!("`{text}` is past the largest byte count"))?;
    Ok((bytes > 0).then_some(bytes))
}

/// Whether jobs may go into systemd scopes: the platform has them, the
/// operator did not turn them off, and no earlier attempt found the user
/// manager missing.
fn scopes_allowed() -> bool {
    static REFUSED: OnceLock<bool> = OnceLock::new();
    cfg!(target_os = "linux")
        && !SCOPES_UNAVAILABLE.load(Ordering::Relaxed)
        && !*REFUSED.get_or_init(|| {
            std::env::var(SCOPE_ENV).is_ok_and(|value| value.eq_ignore_ascii_case("off"))
        })
}

/// Set once a scope could not be made, so later jobs skip straight to the
/// resource limit instead of paying for the same failure again.
static SCOPES_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// The parent's half of one job's handshake.
pub(crate) struct Handshake {
    max: u64,
    scope: bool,
    /// Read end of the report pipe, and write end of the gate pipe.
    report: OwnedFd,
    gate: OwnedFd,
    /// The child's ends. They close once the spawn has returned, so a
    /// child that never reached the handshake reads as an empty report.
    child_ends: Option<(OwnedFd, OwnedFd)>,
}

/// The helper serving one handshake while the spawn runs.
pub(crate) struct Serving {
    helper: JoinHandle<Enforcement>,
    child_ends: Option<(OwnedFd, OwnedFd)>,
    max: u64,
}

/// Arms `command` so that, when spawned, its child waits to be placed under
/// a cap of `max` bytes before it executes the program.
///
/// # Errors
///
/// Returns a message when the pipes cannot be made.
pub(crate) fn arm(command: &mut Command, max: u64) -> Result<Handshake, String> {
    arm_with(command, max, scopes_allowed())
}

pub(crate) fn arm_with(command: &mut Command, max: u64, scope: bool) -> Result<Handshake, String> {
    use std::os::unix::process::CommandExt as _;

    // The fallback never cuts below what an enclosing cgroup already allows.
    // Inside a task container, the container's limit is what protects the
    // host, and a per-process limit under it would only fail a command the
    // task's own budget admits.
    let data_limit = enclosing_limit().map_or(max, |enclosing| enclosing.max(max));
    let (report_read, report_write) = pipe()?;
    let (gate_read, gate_write) = pipe()?;
    let fds = [
        report_read.as_raw_fd(),
        report_write.as_raw_fd(),
        gate_read.as_raw_fd(),
        gate_write.as_raw_fd(),
    ];
    // SAFETY: the closure runs in the child between `fork` and `exec`, so it
    // may only make async-signal-safe calls. It makes `close`, `getpid`,
    // `write`, `read`, and `setrlimit` on integers it captured by value, and
    // allocates nothing.
    unsafe {
        command.pre_exec(move || {
            let [report_read, report_write, gate_read, gate_write] = fds;
            libc::close(report_read);
            libc::close(gate_write);
            let pid = libc::getpid().to_ne_bytes();
            let mut sent = 0;
            while sent < pid.len() {
                let wrote =
                    libc::write(report_write, pid[sent..].as_ptr().cast(), pid.len() - sent);
                if wrote < 0 && interrupted() {
                    continue;
                }
                if wrote <= 0 {
                    break;
                }
                sent += wrote.cast_unsigned();
            }
            libc::close(report_write);
            let mut answer = 0u8;
            let read = loop {
                let read = libc::read(gate_read, (&raw mut answer).cast(), 1);
                if read < 0 && interrupted() {
                    continue;
                }
                break read;
            };
            libc::close(gate_read);
            // Anything but the scope's answer — a refusal, a closed pipe, a
            // parent that went away — gets the resource limit, so no job
            // runs uncapped because the handshake failed.
            if read != 1 || answer != SCOPED {
                let limit = libc::rlimit {
                    rlim_cur: data_limit as libc::rlim_t,
                    rlim_max: data_limit as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_DATA, &raw const limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(Handshake {
        max,
        scope,
        report: report_read,
        gate: gate_write,
        child_ends: Some((report_write, gate_read)),
    })
}

/// The gate byte that tells the child it is in its scope.
const SCOPED: u8 = b's';
/// The gate byte that tells the child to cap itself.
const LIMITED: u8 = b'r';

/// Whether the last call failed only because a signal interrupted it.
fn interrupted() -> bool {
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
}

impl Handshake {
    /// Starts the helper. Call this just before the spawn, and
    /// [`Serving::finish`] just after it.
    pub(crate) fn serve(mut self) -> Serving {
        let child_ends = self.child_ends.take();
        let (max, scope) = (self.max, self.scope);
        let helper = std::thread::spawn(move || {
            let mut report = std::fs::File::from(self.report);
            let mut gate = std::fs::File::from(self.gate);
            let Some(pid) = read_pid(&mut report) else {
                // The child never reached the handshake, so the spawn
                // failed; there is nothing to place.
                return Enforcement::DataLimit;
            };
            let placed = if scope { place(pid, max) } else { None };
            let answer = if placed.is_some() { SCOPED } else { LIMITED };
            // A failed write means the child is gone, and a gone child needs
            // no answer.
            let _ = gate.write_all(&[answer]);
            placed.map_or(Enforcement::DataLimit, Enforcement::Scope)
        });
        Serving {
            helper,
            child_ends,
            max,
        }
    }
}

impl Serving {
    /// Closes the child's ends and waits for the helper's answer.
    pub(crate) fn finish(mut self) -> Placed {
        drop(self.child_ends.take());
        let enforcement = self.helper.join().unwrap_or(Enforcement::DataLimit);
        Placed {
            max: self.max,
            enforcement,
        }
    }
}

/// Where one job's cap ended up, before the job ran.
#[derive(Clone, Debug)]
pub(crate) struct Placed {
    max: u64,
    enforcement: Enforcement,
}

impl Placed {
    /// The cap as it stood before the job ran, with nothing learned about
    /// how the job ended.
    pub(crate) fn unsettled(&self) -> Memory {
        Memory {
            max: self.max,
            enforcement: self.enforcement.clone(),
            exceeded: false,
        }
    }

    /// Settles the cap once the job's tree is gone: whether the kernel
    /// killed inside the scope, and the scope cleared away.
    ///
    /// This blocks for up to [`SETTLE_WALL`] while systemd notices the
    /// scope is empty; the asynchronous callers run it on a blocking thread.
    pub(crate) fn settle(self) -> Memory {
        let exceeded = match &self.enforcement {
            Enforcement::Scope(unit) => settle_scope(unit),
            Enforcement::DataLimit => false,
        };
        Memory {
            max: self.max,
            enforcement: self.enforcement,
            exceeded,
        }
    }
}

/// Makes a pipe whose ends close on `exec`, so the program the child runs
/// inherits neither.
fn pipe() -> Result<(OwnedFd, OwnedFd), String> {
    let mut fds: [RawFd; 2] = [-1, -1];
    // SAFETY: `pipe2` writes two descriptors into the array it is given,
    // which has room for exactly two.
    #[cfg(target_os = "linux")]
    let made = unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
    #[cfg(not(target_os = "linux"))]
    let made = {
        // SAFETY: as above; the flags are set before any fork can happen,
        // because the supervisor forks only after `arm` returns.
        let made = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if made == 0 {
            for fd in fds {
                // SAFETY: `fcntl` on a descriptor this call just made.
                unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
            }
        }
        made
    };
    if made != 0 {
        return Err(format!(
            "could not make the memory cap's pipe: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: both descriptors were just made by `pipe2` and nothing else
    // owns them.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Reads the child's process identifier, or `None` when the child closed
/// the pipe first or took longer than the helper's bound.
fn read_pid(report: &mut std::fs::File) -> Option<i32> {
    let mut poll = libc::pollfd {
        fd: report.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let wall = i32::try_from(HELPER_WALL.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `poll` reads one `pollfd` this function owns.
    if unsafe { libc::poll(&raw mut poll, 1, wall) } <= 0 {
        return None;
    }
    let mut pid = [0u8; 4];
    report.read_exact(&mut pid).ok()?;
    Some(i32::from_ne_bytes(pid))
}

/// Puts `pid` in a new scope capped at `max` bytes, and returns the scope's
/// name once the process is in it and the cap is in force.
fn place(pid: i32, max: u64) -> Option<String> {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let unit = format!(
        "supervise-{}-{}-{}.scope",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed),
        pid
    );
    let mut args: Vec<String> = [
        "--user",
        "--quiet",
        "call",
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
        "StartTransientUnit",
        "ssa(sv)a(sa(sv))",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    let slice = own_slice();
    args.extend([unit.clone(), "fail".to_string()]);
    args.push((5 + usize::from(slice.is_some())).to_string());
    args.extend(["Description", "s"].map(String::from));
    args.push(format!(
        "supervise job {pid} from process {}",
        std::process::id()
    ));
    args.extend(["PIDs", "au", "1"].map(String::from));
    args.push(pid.to_string());
    args.extend(["MemoryMax", "t"].map(String::from));
    args.push(max.to_string());
    // Swap would only slow a job on its way to the cap, and it takes the
    // room the desktop needs.
    args.extend(["MemorySwapMax", "t", "0"].map(String::from));
    // The kernel kills the whole tree together rather than one process in
    // it, which is what ends a job rather than leaving it half alive.
    args.extend(["OOMPolicy", "s", "kill"].map(String::from));
    if let Some(slice) = slice {
        // The scope stays under whatever slice bounds this process, such as
        // the operator's agents slice, instead of moving out from under it.
        args.extend(["Slice", "s"].map(String::from));
        args.push(slice);
    }
    args.push("0".to_string());
    if helper("busctl", &args).is_none() {
        SCOPES_UNAVAILABLE.store(true, Ordering::Relaxed);
        return None;
    }
    let deadline = Instant::now() + HELPER_WALL;
    loop {
        if let Some(path) = cgroup_of(pid).filter(|path| path.ends_with(&format!("/{unit}"))) {
            // A manager that was not delegated the memory controller makes
            // the scope and enforces nothing; the missing file is how that
            // shows.
            if std::path::Path::new(&format!("/sys/fs/cgroup{path}/memory.max")).exists() {
                return Some(unit);
            }
            SCOPES_UNAVAILABLE.store(true, Ordering::Relaxed);
            return None;
        }
        if Instant::now() >= deadline {
            let _ = helper("systemctl", &["--user", "stop", "--no-block", &unit]);
            return None;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The tightest finite `memory.max` on this process's cgroup or any cgroup
/// above it that this process can see.
fn enclosing_limit() -> Option<u64> {
    let own = cgroup_of(i32::try_from(std::process::id()).ok()?)?;
    let mut path = own.trim_end_matches('/');
    let mut tightest: Option<u64> = None;
    loop {
        let limit = std::fs::read_to_string(format!("/sys/fs/cgroup{path}/memory.max"))
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok());
        if let Some(limit) = limit {
            tightest = Some(tightest.map_or(limit, |seen| seen.min(limit)));
        }
        if path.is_empty() {
            return tightest;
        }
        path = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
}

/// The unified cgroup `pid` is in.
fn cgroup_of(pid: i32) -> Option<String> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::to_string)
}

/// The user-manager slice this process runs in, when it runs under one.
fn own_slice() -> Option<String> {
    let path = cgroup_of(i32::try_from(std::process::id()).ok()?)?;
    slice_of(&path)
}

/// The innermost slice in a cgroup path under a user manager.
fn slice_of(path: &str) -> Option<String> {
    let (_, managed) = path.split_once(".service/")?;
    let (parents, _) = managed.rsplit_once('/')?;
    parents
        .rsplit('/')
        .find(|part| part.ends_with(".slice"))
        .map(str::to_string)
}

/// Waits for systemd to settle a scope whose processes are gone, and says
/// whether its result was an out-of-memory kill.
///
/// A scope with processes still in it after [`GRACE`] holds something that
/// left the job's process group, such as a `setsid` daemon. The scope is
/// the job's tree, so those end here too.
fn settle_scope(unit: &str) -> bool {
    let started = Instant::now();
    let mut swept = false;
    loop {
        let state = helper(
            "systemctl",
            &[
                "--user",
                "show",
                "--property=LoadState",
                "--property=ActiveState",
                "--property=Result",
                "--property=ControlGroup",
                unit,
            ],
        )
        .unwrap_or_default();
        let field = |name: &str| {
            state
                .lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
                .unwrap_or_default()
                .to_string()
        };
        let oom = field("Result") == "oom-kill";
        match (field("LoadState").as_str(), field("ActiveState").as_str()) {
            // A scope that ended cleanly is collected at once.
            ("not-found" | "", _) => return false,
            (_, "failed") => {
                let _ = helper("systemctl", &["--user", "reset-failed", unit]);
                return oom;
            }
            (_, "inactive") => return oom,
            _ if started.elapsed() >= SETTLE_WALL => {
                let _ = helper("systemctl", &["--user", "stop", "--no-block", unit]);
                return oom;
            }
            _ if !swept && started.elapsed() >= GRACE && occupied(&field("ControlGroup")) => {
                swept = true;
                let _ = helper("systemctl", &["--user", "kill", "--signal=SIGKILL", unit]);
            }
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

/// Whether any process is still in a cgroup, named by its path under
/// `/sys/fs/cgroup`.
fn occupied(cgroup: &str) -> bool {
    !cgroup.is_empty()
        && std::fs::read_to_string(format!("/sys/fs/cgroup{cgroup}/cgroup.procs"))
            .is_ok_and(|procs| !procs.trim().is_empty())
}

/// Runs a systemd tool under [`HELPER_WALL`] and returns its standard
/// output when it exits zero.
fn helper<S: AsRef<std::ffi::OsStr>>(program: &str, args: &[S]) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + HELPER_WALL;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(2));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    status.success().then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_read_with_and_without_a_suffix() {
        assert_eq!(parse_bytes("1048576"), Ok(Some(1 << 20)));
        assert_eq!(parse_bytes("16G"), Ok(Some(16 << 30)));
        assert_eq!(parse_bytes("16GiB"), Ok(Some(16 << 30)));
        assert_eq!(parse_bytes("512m"), Ok(Some(512 << 20)));
        assert_eq!(parse_bytes("64KB"), Ok(Some(64 << 10)));
        assert_eq!(parse_bytes(" 2T "), Ok(Some(2 << 40)));
    }

    #[test]
    fn none_off_and_zero_mean_no_cap() {
        for text in ["none", "OFF", "0", "0G"] {
            assert_eq!(parse_bytes(text), Ok(None), "{text}");
        }
    }

    #[test]
    fn a_count_that_is_not_one_is_refused() {
        assert!(parse_bytes("lots").is_err());
        assert!(parse_bytes("").is_err());
        assert!(parse_bytes("99999999999T").is_err());
    }

    #[test]
    fn without_a_scope_the_child_caps_itself_before_it_runs() {
        let mut command = Command::new("sh");
        command.args(["-c", "ulimit -d"]).stdout(Stdio::piped());
        let serving = arm_with(&mut command, 256 << 20, false).unwrap().serve();
        let spawned = command.spawn();
        let placed = serving.finish();
        let output = spawned.unwrap().wait_with_output().unwrap();
        assert_eq!(placed.enforcement, Enforcement::DataLimit);
        // `ulimit -d` reports kibibytes. An enclosing cgroup with a larger
        // limit raises the floor.
        let expected = enclosing_limit().map_or(256 << 20, |limit| limit.max(256 << 20));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            (expected / 1024).to_string()
        );
        let memory = placed.settle();
        assert!(!memory.exceeded);
    }

    #[test]
    fn a_spawn_that_fails_leaves_no_helper_waiting() {
        // The working directory fails before the handshake, so the child
        // never reports and the helper must not wait on it.
        let mut command = Command::new("true");
        command.current_dir("/nonexistent/supervise-test");
        let serving = arm_with(&mut command, 256 << 20, true).unwrap().serve();
        assert!(command.spawn().is_err());
        assert_eq!(serving.finish().enforcement, Enforcement::DataLimit);
    }

    #[test]
    fn the_innermost_slice_under_a_user_manager_is_found() {
        assert_eq!(
            slice_of("/user.slice/user-1000.slice/user@1000.service/agents.slice/run-x.scope"),
            Some("agents.slice".to_string())
        );
        assert_eq!(
            slice_of(
                "/user.slice/user-1000.slice/user@1000.service/app.slice/app-a.slice/b.service"
            ),
            Some("app-a.slice".to_string())
        );
        assert_eq!(
            slice_of("/user.slice/user-1000.slice/user@1000.service/init.scope"),
            None
        );
        assert_eq!(
            slice_of("/user.slice/user-1000.slice/session-2.scope"),
            None
        );
    }
}
