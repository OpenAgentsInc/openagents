//! Stopping a child and everything the child started.
//!
//! A coding agent shells out. Killing only the agent leaves its build, its
//! test run, or its `sleep` behind with nothing left to stop them, and with a
//! fan-out that is one orphan per child every time a run is cancelled. So a
//! child is spawned into a process group of its own and the group is what gets
//! signalled.
//!
//! `SIGTERM` first, then `SIGKILL` after a grace period, so an agent that
//! writes a transcript on the way out gets to write it.

use std::time::Duration;

/// How long a stopped child has to leave on its own before it is killed.
pub const KILL_GRACE: Duration = Duration::from_secs(3);

/// Signal a process group. `pid` is the group leader, which is the child
/// itself because it was spawned with `process_group(0)`.
///
/// Returns whether the signal was delivered. A group that has already exited
/// reports `false`, which is the outcome that was wanted.
#[cfg(unix)]
pub fn signal_group(pid: u32, signal: i32) -> bool {
    // Negative pid means "the group led by pid" — the whole point of the
    // exercise. Safe because the only inputs are an integer we were handed by
    // the spawn and a constant from libc.
    unsafe { libc::kill(-(pid as i32), signal) == 0 }
}

#[cfg(not(unix))]
pub fn signal_group(_pid: u32, _signal: i32) -> bool {
    false
}

#[cfg(unix)]
pub const SIGTERM: i32 = libc::SIGTERM;
#[cfg(unix)]
pub const SIGKILL: i32 = libc::SIGKILL;

#[cfg(not(unix))]
pub const SIGTERM: i32 = 15;
#[cfg(not(unix))]
pub const SIGKILL: i32 = 9;

/// Ask a child's whole group to stop, then insist.
///
/// The direct child is killed as well as the group: on a platform where the
/// group signal does not land, the agent itself still goes.
pub async fn stop_tree(child: &mut tokio::process::Child) {
    let Some(pid) = child.id() else {
        return;
    };
    signal_group(pid, SIGTERM);
    tokio::select! {
        _ = child.wait() => return,
        _ = tokio::time::sleep(KILL_GRACE) => {}
    }
    signal_group(pid, SIGKILL);
    let _ = child.start_kill();
}
