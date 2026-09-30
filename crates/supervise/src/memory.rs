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
//!
//! # Windows
//!
//! Windows has neither process groups nor resource limits, and it has
//! something better than both: a job object. Each job's direct child
//! starts suspended, joins a job object of its own, and only then runs, so
//! everything it starts is in the job. The job object holds the whole
//! tree's committed memory to the cap (`JobMemoryLimit`), and the kernel
//! reports a process that ran into it on the job's completion port, so a
//! job its cap stopped is told apart from one that failed
//! ([`Enforcement::JobObject`]). A process at the cap sees a failed
//! allocation rather than being killed, as under a resource limit.

#![cfg_attr(not(feature = "job"), allow(dead_code))]

use std::sync::OnceLock;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
mod unix;
#[cfg(all(unix, feature = "job"))]
pub(crate) use unix::{Handshake, Placed, Serving, arm};

#[cfg(all(windows, feature = "job"))]
mod windows;
#[cfg(all(windows, feature = "job"))]
pub(crate) use windows::Placed;

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
#[cfg(unix)]
const HELPER_WALL: Duration = Duration::from_secs(3);

/// How long the supervisor waits, after a job's tree is gone, for systemd
/// to settle the scope's result.
#[cfg(unix)]
const SETTLE_WALL: Duration = Duration::from_secs(2);

/// How often a watched job's footprint is sampled.
#[cfg(unix)]
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
    /// A Windows job object held the committed memory of the job's whole
    /// tree to the cap; a process that asked for more saw its allocation
    /// fail.
    JobObject,
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
}
