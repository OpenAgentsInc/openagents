//! The Unix process group one job runs in, and the tree handle the
//! supervisor ends it through.
//!
//! A job's direct child is spawned with `process_group(0)`, so the child
//! becomes the leader of a new group whose identifier is its own process
//! identifier. Everything the job starts inherits that group unless it
//! leaves on purpose, which makes the group the job's tree and makes
//! `killpg` the way to end it.
//!
//! # The one window this leaves
//!
//! A group identifier belongs to the job while the leader exists, and a
//! reaped leader no longer does. The supervisor therefore signals the group
//! immediately after it reaps the leader, and the host could in principle
//! recycle the identifier in between. The window is microseconds wide and
//! closing it needs `waitid(WNOWAIT)`, which is not on the asynchronous
//! runtime's wait path. This is the trade, written down rather than
//! implied.

/// Sends `signal` to every process in `group`.
///
/// Returns whether the group took it. `false` is the ordinary answer for a
/// group whose members have all exited, which is why nothing here treats it
/// as a failure.
#[cfg(unix)]
pub(crate) fn signal(group: i32, signal: i32) -> bool {
    if group <= 0 {
        return false;
    }
    // SAFETY: `killpg` reads two integers and returns one. The group is the
    // one this supervisor made for this job's direct child, so no process
    // outside the job is in it.
    unsafe { libc::killpg(group, signal) == 0 }
}

/// Asks the group to stop.
#[cfg(unix)]
pub(crate) fn ask(group: i32) -> bool {
    signal(group, libc::SIGTERM)
}

/// Ends the group, whatever it was doing.
#[cfg(unix)]
pub(crate) fn end(group: i32) -> bool {
    signal(group, libc::SIGKILL)
}

/// Whether any process is still in the group.
///
/// Signal zero performs the permission and existence checks and sends
/// nothing, which is the portable way to ask.
#[cfg(unix)]
#[must_use]
pub fn running(group: i32) -> bool {
    signal(group, 0)
}

/// Whether `pid` names a live process — not a group, one process.
///
/// Signal zero sends nothing; `EPERM` is a live process another user
/// owns, which this probe still counts as running. A recovered process
/// identifier can name a different live process than the one that wrote
/// it — liveness is the question this answers, not identity.
#[cfg(unix)]
#[must_use]
pub fn process_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: `kill` reads two integers and returns one; signal zero
    // performs the permission and existence checks and sends nothing.
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// The tree one job owns, as the supervisor ends it: the direct child's
/// process group on Unix, and a job object on Windows. Cloning shares it.
#[cfg(all(unix, feature = "job"))]
#[derive(Clone, Debug)]
pub(crate) struct Tree {
    group: Option<i32>,
}

#[cfg(all(unix, feature = "job"))]
impl Tree {
    /// The tree whose group leader is the direct child `pid`.
    pub(crate) fn led_by(pid: Option<u32>) -> Self {
        Tree {
            group: pid.and_then(|id| i32::try_from(id).ok()),
        }
    }

    /// The group's identifier, which is the direct child's process
    /// identifier.
    pub(crate) fn id(&self) -> Option<i32> {
        self.group
    }

    /// Asks the tree to stop: `SIGTERM` to the group.
    pub(crate) fn ask(&self) {
        if let Some(group) = self.group {
            ask(group);
        }
    }

    /// Ends the tree: `SIGKILL` to the group.
    pub(crate) fn end(&self) {
        if let Some(group) = self.group {
            end(group);
        }
    }

    /// Whether any process is still in the group.
    pub(crate) fn running(&self) -> bool {
        self.group.is_some_and(running)
    }
}

#[cfg(windows)]
pub use crate::windows::{process_running, running};

#[cfg(all(windows, feature = "job"))]
pub(crate) use crate::windows::Tree;
