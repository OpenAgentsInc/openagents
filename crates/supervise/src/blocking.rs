//! The blocking half of the same contract, for a caller that keeps its own
//! output.
//!
//! A benchmark run writes its output to files on purpose — the files are
//! part of the record a reader opens afterwards — so it needs the process
//! tree half of the supervisor and not the capture half. The ownership
//! rules are the contract's: the job runs in a process group of its own,
//! the deadline terminates the group, and the direct child is reaped before
//! the wait returns.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use crate::{Ending, GRACE, group};

/// How often the wait asks whether the child is done.
const POLL: Duration = Duration::from_millis(25);

/// Puts the command's job in a process group of its own, which is what
/// makes [`wait`] able to end the whole tree.
///
/// Call this before spawning. A child spawned without it shares this
/// process's group, and terminating that group would terminate the caller.
pub fn own_group(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

/// Waits for a child, terminating its process group when `wall` passes.
///
/// The sequence is the one [`crate::Job`] uses: `SIGTERM` to the group, up
/// to [`GRACE`] for the direct child to exit, `SIGKILL` to the group, and a
/// reap. A child that exits on its own still takes its group with it, so a
/// descendant cannot outlive the job that started it.
///
/// The child must have been spawned through [`own_group`]. Waiting on one
/// that was not would signal this process's own group, so this returns
/// [`Ending::Failed`] rather than doing that.
pub fn wait(child: &mut Child, wall: Duration) -> Ending {
    let group = match i32::try_from(child.id()) {
        Ok(group) if leads(group) => group,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return Ending::Failed(
                "the child process was not started in a process group of its own; start it through own_group"
                    .to_string(),
            );
        }
    };
    let deadline = Instant::now() + wall;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                group::end(group);
                return Ending::Exited(status.code());
            }
            Ok(None) => {}
            Err(error) => return Ending::Failed(error.to_string()),
        }
        if Instant::now() >= deadline {
            return terminate(child, group);
        }
        std::thread::sleep(POLL);
    }
}

/// Ends a child that ran past its deadline, and everything it started.
fn terminate(child: &mut Child, group: i32) -> Ending {
    group::ask(group);
    let grace = Instant::now() + GRACE;
    while Instant::now() < grace {
        if matches!(child.try_wait(), Ok(Some(_)) | Err(_)) {
            break;
        }
        std::thread::sleep(POLL);
    }
    group::end(group);
    let _ = child.wait();
    Ending::TimedOut
}

/// Whether the process leads a group of its own, which is what
/// [`own_group`] arranges and what makes the process identifier a group
/// identifier this job owns.
fn leads(pid: i32) -> bool {
    // SAFETY: `getpgid` reads a process identifier and returns the group
    // it is in, or -1.
    unsafe { libc::getpgid(pid) == pid }
}
