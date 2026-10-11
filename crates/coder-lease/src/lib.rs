//! The host resource broker: a durable table of exclusive and counted
//! leases that Coder, its delegates, and any agent on the machine use to
//! share scarce resources.
//!
//! The broker is a file-backed table, not a daemon. It lives under
//! `~/.openagents/leases/` (or [`ROOT_VAR`]):
//!
//! - `table.json` holds every lease, held or waiting, and is read and
//!   written only under an exclusive `flock` on `table.lock`.
//! - `held/<id>.lock` is each lease's holder lock, which the holder keeps
//!   locked for the whole run. A lease whose holder lock another process can
//!   take is dead, and the next reader drops it. The kernel releases the lock
//!   when the holder exits or crashes, so a dead holder never keeps a lease.
//! - `receipts/<id>.json` records each released lease, with the disk a
//!   `build` lease's slot and worktree hold ([`DiskUse`]); [`usage`] sums
//!   them per session.
//! - `grants/screen.json` is the owner's grant of the real screen.
//! - `claims/<owner>/<name>/issue-<n>.json` is an issue claim held by an
//!   agent session, which outlives the command that took it ([`claims`]).
//! - `artifacts/<name>/` is a single-digest artifact's queue of submitted
//!   changes and the scratch worktree its runner lands them from
//!   ([`artifact`]).
//!
//! Exclusive resources ([`Shape::Exclusive`]) admit one holder. Counted
//! ones ([`Shape::Counted`]) admit holders while their amounts fit:
//! `build` slots (default `clamp(cores / 4, 1, 4)`), a `memory` budget (75
//! percent of physical memory, in GiB), and `disk` budgets above the
//! free-space floor. A `build` lease also reserves a disk budget above the
//! floor; when the space isn't there, the broker runs its reclaim hook
//! ([`Broker::with_reclaim`]) once before it refuses. A request that can't be admitted waits in its
//! resource's priority queue ([`Priority`]: `owner`, `push`, `normal`, then
//! `background`, first in, first out within a priority, and one level more
//! urgent for each aging step it waits), or fails at once under
//! [`Wait::No`].
//!
//! The `quiet` lease is admitted only when no `build` lease is held, and
//! while it is held or queued no new `build` lease is admitted. Running
//! builds drain on their own: the broker never signals, pauses, or slows a
//! process.
//!
//! [`shim`] writes the `cargo` shim Coder puts first on a delegate's
//! `PATH`, so a delegate's heavy builds take `build` leases, and the
//! `screencapture` shim beside it, which refuses without a `screen` lease.
//! [`screen_refusal`] is the same rule for a program about to open a
//! window: an agent environment needs a `screen` lease.
//!
//! [`scratch`] gives each session a durable scratch directory under
//! `~/.openagents/scratch/<session>/`, which a lease's command and Coder's
//! delegates get in `OPENAGENTS_SCRATCH`.
//!
//! [`placement`] decides whether a long job runs here under a lease or on
//! another computer, from its class and the `coder.placement` setting.
//!
//! `docs/coder/runtime/leases.md` is the operator's page.

pub mod artifact;
mod broker;
pub mod claims;
mod grant;
mod holder;
mod limits;
pub mod observe;
pub mod placement;
mod resource;
mod root;
pub mod scratch;
mod screen;
pub mod shim;
mod table;
mod usage;

pub use broker::{
    Blocked, Broker, LEASE_ID_VAR, LEASES_VAR, Lease, POLL, Queued, RECEIPT_SCHEMA, Receipt,
    Request, Wait, queue_of,
};
pub use grant::{DEFAULT_GRANT, GRANT_SCHEMA, Grant, parse_duration};
pub use holder::{
    AGENT_PROCESSES, AGENT_VARS, Holder, SESSION_VAR, Session, agent_ancestor, ancestors,
    command_name, grant_refusal,
};
pub use limits::{
    AGING_VAR, BUILD_DISK_VAR, BUILD_LEASES_VAR, DEFAULT_AGING, DEFAULT_BUILD_DISK_GB,
    DEFAULT_FLOOR_GB, Limits, MEMORY_GIB_VAR, Machine, SLOT_FREE_VAR, aging_from, free_disk,
};
pub use observe::{Observed, observe};
pub use resource::{NAMED, Resource, Shape};
pub use root::{ROOT_VAR, refuse_real_home, root_from, root_from_env};
pub use screen::{
    SCREENCAPTURE_SHIM, agent_marker, holds_screen, screen_refusal, screen_refusal_here,
};
pub use table::{Entry, PRIORITY_VAR, Priority, State, TABLE_SCHEMA};
pub use usage::{
    DiskUse, PathUse, SLOT_USE_SCHEMA, SessionUsage, SlotUse, process_running, session_live, usage,
};

use std::fmt;

/// Why a lease was not taken or released.
#[derive(Debug)]
pub enum Error {
    /// A file under the lease root could not be read or written.
    Io(std::io::Error),
    /// A file under the lease root is not what the broker wrote.
    Corrupt(String),
    /// The request can never be admitted as asked.
    Invalid(String),
    /// The request was not admitted at once, and it said not to wait.
    Busy(Blocked),
    /// The request waited as long as it said it would.
    TimedOut(Blocked),
    /// The resource needs an owner grant that does not admit this session.
    NoGrant(String),
    /// The request's own entry left the table while it waited.
    Lost(String),
    /// A `build` lease would leave less free space than the floor plus the
    /// disk budgets held and its own, even after reclaiming.
    DiskLow {
        /// Free bytes on the lease root's volume after reclaiming.
        free: u64,
        /// The free space it needs, in GB.
        need_gb: u64,
        /// The floor, in GB.
        floor_gb: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "the lease table could not be used: {error}"),
            Error::Corrupt(message) => write!(f, "the lease table is unreadable: {message}"),
            Error::Invalid(message) | Error::NoGrant(message) => f.write_str(message),
            Error::Busy(blocked) => write!(f, "not admitted: {}", blocked.reason),
            Error::TimedOut(blocked) => write!(f, "still waiting: {}", blocked.reason),
            Error::Lost(id) => write!(f, "lease {id} left the table while it waited"),
            Error::DiskLow {
                free,
                need_gb,
                floor_gb,
            } => write!(
                f,
                "{} GB is free after reclaiming; a build needs {need_gb} GB: the {floor_gb} GB floor and the disk budgets of this build and those held",
                free / 1_000_000_000
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

/// Now, in Unix milliseconds.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod usage_tests;
