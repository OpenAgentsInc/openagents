//! Class 10, agent scratch: the per-session directories under
//! `~/.openagents/scratch/` (`coder_lease::scratch`). A session's scratch
//! is a candidate only once the session has ended and nothing in it changed
//! for the rule's `scratch_days`.
//!
//! A session has ended when no lease in the machine's lease table
//! (`~/.openagents/leases`) names it and, for a session whose identity
//! names a process (`codex:4242`, `process:77`), that process is gone. A
//! lease table that can't be read keeps every scratch directory. The
//! in-use check every candidate passes also keeps one a process has open
//! or works inside.

use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use crate::paths::Layout;

/// Why the scratch directory at `path` belongs to a live session, or
/// `None` when its session ended.
#[must_use]
pub fn live(layout: &Layout, path: &Path) -> Option<String> {
    let session = coder_lease::scratch::session_of(path);
    match coder_lease::scratch::live_sessions(&layout.leases()) {
        Ok(live) if live.contains(&session) => {
            return Some(format!("session {session} holds a lease"));
        }
        Ok(_) => {}
        Err(error) => return Some(format!("the lease table can't be read: {error}")),
    }
    if let Some(pid) = coder_lease::scratch::session_pid(&session)
        && running(pid)
    {
        return Some(format!("session {session} is still running"));
    }
    None
}

/// Whether process `pid` exists. A process this user may not signal still
/// exists.
fn running(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return true;
    };
    // SAFETY: signal 0 checks only that the process exists.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// The most entries [`touched`] reads before it gives up and calls the
/// directory in use now.
const MAX_ENTRIES: usize = 100_000;

/// When anything in a scratch directory last changed: the newest
/// modification time in the tree, never through a link and never on
/// another volume. A tree larger than 100,000 entries counts as changed
/// `now`, so it is kept rather than judged from part of it.
#[must_use]
pub fn touched(path: &Path, now: u64) -> u64 {
    let Ok(top) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    let device = top.dev();
    let mut newest = stamp(&top);
    let mut seen = 0usize;
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > MAX_ENTRIES {
                return now;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            newest = newest.max(stamp(&meta));
            if meta.is_dir() && meta.dev() == device {
                stack.push(entry.path());
            }
        }
    }
    newest
}

fn stamp(meta: &std::fs::Metadata) -> u64 {
    u64::try_from(meta.mtime()).unwrap_or(0)
}
