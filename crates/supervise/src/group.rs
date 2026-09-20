//! The Unix process group one job runs in.
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
pub(crate) fn ask(group: i32) -> bool {
    signal(group, libc::SIGTERM)
}

/// Ends the group, whatever it was doing.
pub(crate) fn end(group: i32) -> bool {
    signal(group, libc::SIGKILL)
}

/// Whether any process is still in the group.
///
/// Signal zero performs the permission and existence checks and sends
/// nothing, which is the portable way to ask.
#[must_use]
pub fn running(group: i32) -> bool {
    signal(group, 0)
}
