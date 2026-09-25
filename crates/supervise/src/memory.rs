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
//!
//! # macOS
//!
//! macOS has no cgroups, and since macOS 26 it refuses an `RLIMIT_DATA`
//! below the process's mapped address space, which starts at hundreds of
//! gibibytes, so the resource limit can't hold a cap there either. On macOS
//! the helper watches the job instead: before it opens the gate it starts a
//! thread that samples the physical footprint of every process in the job's
//! process group every [`WATCH_EVERY`], and kills the group when the total
//! passes the cap. That counts the tree and reports a kill, as a scope does,
//! but a process that leaves the group is not counted, and a job can pass
//! the cap by what it allocates between two samples before it is killed.
//! A group the watch can't list is not an empty one: the watch kills it
//! rather than let it run uncapped, and [`Memory::unenforced`] says why.
//! Linux has a sampler too, over `/proc`, so the watch is tested where the
//! workspace's tests run; Linux jobs are not watched.
//!
//! Where the child still has to set the resource limit and the system
//! refuses it, the spawn fails with a message that names the cap and
//! [`MEMORY_ENV`], so no job runs uncapped because the limit was refused.

#![cfg_attr(not(feature = "job"), allow(dead_code))]

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
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

/// How often a watched job's footprint is sampled.
const WATCH_EVERY: Duration = Duration::from_millis(25);

/// What held one job's memory, and whether the job ran into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    /// The cap, in bytes.
    pub max: u64,
    /// What enforced it.
    pub enforcement: Enforcement,
    /// Whether a process in the job was killed for passing the cap. A scope
    /// and a watch can say so; under [`Enforcement::DataLimit`] this is
    /// always `false`, and a job its limit ended reads as a failure.
    pub exceeded: bool,
    /// Why the cap stopped holding before the job ended, when it did. A
    /// watch that can't read the job's memory kills the job rather than let
    /// it run uncapped, and says why here. `None` means the cap held for the
    /// whole run.
    pub unenforced: Option<String>,
}

/// What enforced a job's memory cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enforcement {
    /// A transient systemd scope, named here, held the job's whole tree to
    /// the cap.
    Scope(String),
    /// Each process in the job had `RLIMIT_DATA` set to the cap.
    DataLimit,
    /// The supervisor sampled the physical footprint of the job's process
    /// group and killed the group when the total passed the cap. This is
    /// what holds a job on macOS.
    Watch,
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
        .map_err(|_| format!("`{text}` is not a byte count; use a number with an optional K, M, G, or T suffix, or none"))?;
    let bytes = count.checked_mul(1u64 << shift).ok_or_else(|| {
        format!("`{text}` is larger than the largest byte count this program can hold")
    })?;
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

/// Whether a job without a scope is watched rather than given the resource
/// limit: on macOS, which refuses a useful `RLIMIT_DATA`.
fn watches() -> bool {
    cfg!(target_os = "macos")
}

/// The parent's half of one job's handshake.
pub(crate) struct Handshake {
    max: u64,
    scope: bool,
    /// Whether a job without a scope is watched; [`watches`] unless a test
    /// says otherwise.
    watch: bool,
    /// Read end of the report pipe, and write end of the gate pipe.
    report: OwnedFd,
    gate: OwnedFd,
    /// The child's ends. They close once the spawn has returned, so a
    /// child that never reached the handshake reads as an empty report.
    child_ends: Option<(OwnedFd, OwnedFd)>,
}

/// The helper serving one handshake while the spawn runs.
pub(crate) struct Serving {
    helper: JoinHandle<(Enforcement, Option<Arc<Watch>>)>,
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
            // Anything but a scope's or a watch's answer — a refusal, a
            // closed pipe, a parent that went away — gets the resource
            // limit, so no job runs uncapped because the handshake failed.
            if read != 1 || (answer != SCOPED && answer != WATCHED) {
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
        watch: watches(),
        report: report_read,
        gate: gate_write,
        child_ends: Some((report_write, gate_read)),
    })
}

/// The gate byte that tells the child it is in its scope.
const SCOPED: u8 = b's';
/// The gate byte that tells the child it is watched.
const WATCHED: u8 = b'w';
/// The gate byte that tells the child to cap itself.
const LIMITED: u8 = b'r';

/// Whether the last call failed only because a signal interrupted it.
fn interrupted() -> bool {
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
}

impl Handshake {
    /// Watches the job where no scope holds it, as macOS does, so a test
    /// takes that path on any platform.
    #[cfg(test)]
    pub(crate) fn watched(mut self) -> Self {
        self.watch = true;
        self
    }

    /// Starts the helper. Call this just before the spawn, and
    /// [`Serving::finish`] just after it.
    pub(crate) fn serve(mut self) -> Serving {
        let child_ends = self.child_ends.take();
        let (max, scope, watches) = (self.max, self.scope, self.watch);
        let helper = std::thread::spawn(move || {
            let mut report = std::fs::File::from(self.report);
            let mut gate = std::fs::File::from(self.gate);
            let Some(pid) = read_pid(&mut report) else {
                // The child never reached the handshake, so the spawn
                // failed; there is nothing to place.
                return (Enforcement::DataLimit, None);
            };
            let placed = if scope { place(pid, max) } else { None };
            // The child leads its own process group by now, since the
            // group is set before the handshake runs, so the group the
            // watch samples is the job's from its first instruction.
            let watch = (placed.is_none() && watches).then(|| Watch::start(pid, max));
            let answer = match (&placed, &watch) {
                (Some(_), _) => SCOPED,
                (None, Some(_)) => WATCHED,
                (None, None) => LIMITED,
            };
            // A failed write means the child is gone, and a gone child needs
            // no answer.
            let _ = gate.write_all(&[answer]);
            let enforcement = match (placed, &watch) {
                (Some(unit), _) => Enforcement::Scope(unit),
                (None, Some(_)) => Enforcement::Watch,
                (None, None) => Enforcement::DataLimit,
            };
            (enforcement, watch)
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
        let (enforcement, watch) = self.helper.join().unwrap_or((Enforcement::DataLimit, None));
        Placed {
            max: self.max,
            enforcement,
            watch,
        }
    }
}

/// Where one job's cap ended up, before the job ran.
#[derive(Clone, Debug)]
pub(crate) struct Placed {
    max: u64,
    enforcement: Enforcement,
    /// The running watch, under [`Enforcement::Watch`].
    watch: Option<Arc<Watch>>,
}

impl Placed {
    /// The cap as it stood before the job ran, with nothing learned about
    /// how the job ended.
    pub(crate) fn unsettled(&self) -> Memory {
        Memory {
            max: self.max,
            enforcement: self.enforcement.clone(),
            exceeded: false,
            unenforced: None,
        }
    }

    /// Why a spawn under this cap failed, in words that say what to do
    /// when the system refused the resource limit.
    pub(crate) fn refusal(&self, error: &std::io::Error) -> String {
        refusal(&self.enforcement, self.max, error)
    }

    /// Settles the cap once the job's tree is gone: whether the kernel
    /// killed inside the scope, and the scope cleared away.
    ///
    /// This blocks for up to [`SETTLE_WALL`] while systemd notices the
    /// scope is empty; the asynchronous callers run it on a blocking thread.
    pub(crate) fn settle(self) -> Memory {
        let (exceeded, unenforced) = match (&self.enforcement, &self.watch) {
            (Enforcement::Scope(unit), _) => (settle_scope(unit), None),
            (Enforcement::Watch, Some(watch)) => watch.stop(),
            _ => (false, None),
        };
        Memory {
            max: self.max,
            enforcement: self.enforcement,
            exceeded,
            unenforced,
        }
    }
}

/// Why a spawn failed. A child told to set the resource limit fails with
/// `EINVAL` or `EPERM` when the system refuses it, and a bare
/// "Invalid argument" says nothing about memory, so that failure names the
/// cap and the way past it.
fn refusal(enforcement: &Enforcement, max: u64, error: &std::io::Error) -> String {
    let refused = matches!(error.raw_os_error(), Some(libc::EINVAL | libc::EPERM));
    if *enforcement == Enforcement::DataLimit && refused {
        format!(
            "couldn't start the job under its memory cap of {max} bytes: this system refused `RLIMIT_DATA` ({error}). Set {MEMORY_ENV} to a cap this system accepts, or to `none` to run without one"
        )
    } else {
        error.to_string()
    }
}

/// A watch on one job's process group.
#[derive(Debug)]
pub(crate) struct Watch {
    stop: AtomicBool,
    exceeded: AtomicBool,
    /// Why the watch stopped holding the cap, when a sample failed.
    unenforced: std::sync::Mutex<Option<String>>,
    thread: std::sync::Mutex<Option<JoinHandle<()>>>,
}

/// What one sample of a group found: its total footprint in bytes, `None`
/// when the group is empty, or why the group couldn't be listed.
type Sample = Result<Option<u64>, String>;

impl Watch {
    /// Starts sampling the process group `group`, and kills it once the
    /// group's footprint passes `max` bytes.
    fn start(group: i32, max: u64) -> Arc<Self> {
        Self::start_with(group, max, footprint)
    }

    /// [`Watch::start`] with the sampler named, so a test can make one fail.
    fn start_with(
        group: i32,
        max: u64,
        sample: fn(i32, &mut Vec<libc::pid_t>) -> Sample,
    ) -> Arc<Self> {
        let watch = Arc::new(Watch {
            stop: AtomicBool::new(false),
            exceeded: AtomicBool::new(false),
            unenforced: std::sync::Mutex::new(None),
            thread: std::sync::Mutex::new(None),
        });
        let watching = Arc::clone(&watch);
        let thread = std::thread::spawn(move || {
            let mut pids = Vec::new();
            while !watching.stop.load(Ordering::Relaxed) {
                let footprint = match sample(group, &mut pids) {
                    Ok(Some(footprint)) => footprint,
                    // An empty group is a finished job, and its identifier
                    // may be reused, so the watch ends with it.
                    Ok(None) => return,
                    // A group the watch can't list may still be running, and
                    // nothing else holds its memory, so it ends here.
                    Err(why) => {
                        crate::group::end(group);
                        *watching
                            .unenforced
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(format!(
                            "the supervisor couldn't read the job's memory, so it ended the job rather than run it without its cap: {why}"
                        ));
                        return;
                    }
                };
                if footprint > max {
                    watching.exceeded.store(true, Ordering::Relaxed);
                    crate::group::end(group);
                    return;
                }
                std::thread::sleep(WATCH_EVERY);
            }
        });
        *watch
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(thread);
        watch
    }

    /// Stops the watch and says whether it killed the job for passing the
    /// cap, and why the cap stopped holding when it did.
    fn stop(&self) -> (bool, Option<String>) {
        self.stop.store(true, Ordering::Relaxed);
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
        let unenforced = self
            .unenforced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        (self.exceeded.load(Ordering::Relaxed), unenforced)
    }
}

/// The total physical footprint of the processes in `group`, in bytes,
/// `None` when the group is empty, or why the group couldn't be listed.
/// `pids` is scratch space the watch keeps between samples.
#[cfg(target_os = "macos")]
fn footprint(group: i32, pids: &mut Vec<libc::pid_t>) -> Sample {
    if pids.is_empty() {
        pids.resize(256, 0);
    }
    let listed = loop {
        let room = libc::c_int::try_from(std::mem::size_of_val(pids.as_slice()))
            .map_err(|_| "the process list outgrew its buffer".to_string())?;
        // `proc_listpgrppids` returns 0 both for an empty group and for a
        // failed call, and only the failure sets `errno`.
        // SAFETY: `__error` returns this thread's `errno`, which is writable.
        unsafe { *libc::__error() = 0 };
        // SAFETY: the buffer has `room` bytes, and the call writes at most
        // that many.
        let listed = unsafe { libc::proc_listpgrppids(group, pids.as_mut_ptr().cast(), room) };
        if listed <= 0 {
            let error = std::io::Error::last_os_error();
            if listed < 0 || error.raw_os_error().is_some_and(|code| code != 0) {
                return Err(format!("couldn't list process group {group}: {error}"));
            }
            return Ok(None);
        }
        let listed = usize::try_from(listed).unwrap_or(0);
        // A full buffer may have left members out.
        if listed < pids.len() || pids.len() >= 1 << 16 {
            break listed.min(pids.len());
        }
        let grown = pids.len() * 2;
        pids.resize(grown, 0);
    };
    let mut total = 0u64;
    for &pid in &pids[..listed] {
        // SAFETY: an all-zero `rusage_info_v2` is a valid value of a struct
        // of integers.
        let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
        // SAFETY: the flavor names the struct the pointer points to. A
        // process that exited since the listing fails the call, and counts
        // for nothing.
        let read = unsafe {
            libc::proc_pid_rusage(
                pid,
                libc::RUSAGE_INFO_V2,
                (&raw mut info).cast::<libc::rusage_info_t>(),
            )
        };
        if read == 0 {
            total = total.saturating_add(info.ri_phys_footprint);
        }
    }
    Ok(Some(total))
}

/// The total resident memory of the processes in `group`, in bytes, from
/// `/proc`, `None` when the group is empty, or why `/proc` couldn't be read.
/// Linux jobs are not watched; this is how the watch is tested here.
#[cfg(target_os = "linux")]
fn footprint(group: i32, _pids: &mut Vec<libc::pid_t>) -> Sample {
    let entries =
        std::fs::read_dir("/proc").map_err(|error| format!("couldn't read /proc: {error}"))?;
    // SAFETY: `sysconf` reads one integer and returns one.
    let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).unwrap_or(4096);
    let mut total = None::<u64>;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .filter(|pid| pid.bytes().all(|byte| byte.is_ascii_digit()))
        else {
            continue;
        };
        // A process that exited since the listing counts for nothing.
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        if process_group(&stat) != Some(group) {
            continue;
        }
        let resident = std::fs::read_to_string(format!("/proc/{pid}/statm"))
            .ok()
            .and_then(|statm| statm.split_whitespace().nth(1)?.parse::<u64>().ok())
            .unwrap_or(0);
        total = Some(
            total
                .unwrap_or(0)
                .saturating_add(resident.saturating_mul(page)),
        );
    }
    Ok(total)
}

/// The process group in a `/proc/<pid>/stat` line: the third field after
/// the command, which is parenthesized and may hold spaces or parentheses.
#[cfg(target_os = "linux")]
fn process_group(stat: &str) -> Option<i32> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(2)?.parse().ok()
}

/// No sampler on this platform. Nothing here watches a job, and a watch
/// that did would end it at the first sample rather than run it uncapped.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn footprint(_group: i32, _pids: &mut Vec<libc::pid_t>) -> Sample {
    Err("this platform has no way to read a process group's memory".to_string())
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
            "couldn't create the pipe that enforces the memory limit: {}",
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
    #[cfg(not(target_os = "macos"))]
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
    #[cfg(target_os = "macos")]
    fn on_macos_the_job_is_watched_and_its_data_limit_left_alone() {
        let mut command = Command::new("sh");
        command.args(["-c", "ulimit -d"]).stdout(Stdio::piped());
        let serving = arm_with(&mut command, 256 << 20, false).unwrap().serve();
        let spawned = command.spawn();
        let placed = serving.finish();
        let output = spawned.unwrap().wait_with_output().unwrap();
        assert_eq!(placed.enforcement, Enforcement::Watch);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "unlimited");
        let memory = placed.settle();
        assert_eq!(memory.enforcement, Enforcement::Watch);
        assert!(!memory.exceeded);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn a_footprint_counts_every_process_in_the_group() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 5 & sleep 5"]);
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command.spawn().unwrap();
        let group = i32::try_from(child.id()).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let mut pids = Vec::new();
        let total = footprint(group, &mut pids)
            .unwrap()
            .expect("the group is running");
        assert!(total > 0);
        crate::group::end(group);
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(footprint(group, &mut pids), Ok(None));
    }

    /// Runs `script` watched, as macOS runs every job without a scope,
    /// under a cap of `max` bytes.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn watched(script: &str, max: u64) -> (std::process::ExitStatus, Memory) {
        let mut command = Command::new("sh");
        command.args(["-c", script]).stdout(Stdio::null());
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let serving = arm_with(&mut command, max, false)
            .unwrap()
            .watched()
            .serve();
        let spawned = command.spawn();
        let placed = serving.finish();
        assert_eq!(placed.enforcement, Enforcement::Watch);
        let status = spawned.unwrap().wait().unwrap();
        (status, placed.settle())
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn a_watched_group_past_its_cap_is_killed_and_reported() {
        // `tail` holds a line it hasn't seen the end of, and a gibibyte of
        // zeros has no newline, so it grows until something stops it.
        let (status, memory) = watched("head -c 1073741824 /dev/zero | tail", 64 << 20);
        assert!(!status.success(), "{status:?}");
        assert!(memory.exceeded, "{memory:?}");
        assert_eq!(memory.unenforced, None);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn a_watched_group_under_its_cap_runs_and_the_cap_held() {
        let (status, memory) = watched("echo held", 256 << 20);
        assert!(status.success(), "{status:?}");
        assert!(!memory.exceeded);
        assert_eq!(memory.unenforced, None);
    }

    #[test]
    fn a_group_the_watch_cannot_list_is_killed_and_the_lapse_recorded() {
        fn unlistable(group: i32, _pids: &mut Vec<libc::pid_t>) -> Sample {
            Err(format!("couldn't list process group {group}: denied"))
        }
        let mut command = Command::new("sleep");
        command.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command.spawn().unwrap();
        let group = i32::try_from(child.id()).unwrap();
        let watch = Watch::start_with(group, 256 << 20, unlistable);
        // The watch kills the group at its first sample rather than wait.
        let status = child.wait().unwrap();
        assert!(!status.success(), "{status:?}");
        let (exceeded, unenforced) = watch.stop();
        assert!(!exceeded, "a lapse is not a job past its cap");
        let why = unenforced.expect("the record says the cap stopped holding");
        assert!(why.contains("rather than run it without its cap"), "{why}");
        assert!(why.ends_with("denied"), "{why}");
    }

    #[test]
    fn an_empty_group_ends_the_watch_without_a_lapse() {
        fn empty(_group: i32, _pids: &mut Vec<libc::pid_t>) -> Sample {
            Ok(None)
        }
        let watch = Watch::start_with(-1, 256 << 20, empty);
        std::thread::sleep(WATCH_EVERY * 2);
        assert_eq!(watch.stop(), (false, None));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn the_process_group_is_read_past_an_awkward_command_name() {
        assert_eq!(process_group("42 (sh) S 1 42 42 0 -1"), Some(42));
        assert_eq!(process_group("42 (a) b (c)) R 7 99 42 0"), Some(99));
        assert_eq!(process_group("42 (sh"), None);
    }

    #[test]
    fn a_refused_data_limit_names_the_cap_and_the_way_past_it() {
        let refused = std::io::Error::from_raw_os_error(libc::EINVAL);
        let message = refusal(&Enforcement::DataLimit, 256 << 20, &refused);
        assert!(
            message.contains("memory cap of 268435456 bytes"),
            "{message}"
        );
        assert!(message.contains(MEMORY_ENV), "{message}");
        assert!(message.contains("`none`"), "{message}");
        // Other failures, and failures under a scope or a watch, keep their
        // own words.
        let missing = std::io::Error::from_raw_os_error(libc::ENOENT);
        assert_eq!(
            refusal(&Enforcement::DataLimit, 256 << 20, &missing),
            missing.to_string()
        );
        assert_eq!(
            refusal(&Enforcement::Watch, 256 << 20, &refused),
            refused.to_string()
        );
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
