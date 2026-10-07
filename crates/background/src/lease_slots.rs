//! Class 1, build slots whose lease ended with its session: when a `build`
//! lease that took a target slot ends, `openagents lease build` writes
//! `<slot>.lease.json` ([`coder_lease::SlotUse`]) naming the lease's
//! session. Once that session has ended (no lease in its table names it,
//! and neither the process its identity names nor its agent process runs)
//! and nothing has used the slot since, the slot is a class 1 candidate at
//! once, with no idle time to wait out. A live session's slot stays in
//! class 2, where it waits for the idle rule.
//!
//! [`reclaim`] runs class 1 alone, for `build` lease admission
//! ([`coder_lease::Broker::with_reclaim`]).

use std::path::Path;

use crate::plan::Env;
use crate::rule::{Action, Class, Level};
use crate::run::{Cause, Report};

/// Why the slot at `slot` may go now, or why it stays: no lease record,
/// a use since its lease ended, or a session that still lives.
///
/// # Errors
/// Why the slot stays.
pub fn ended(slot: &Path) -> Result<String, String> {
    let Some(record) = coder_lease::SlotUse::read(slot) else {
        return Err("no ended build lease".into());
    };
    // Every slot lease writes its lock when it starts and touches it when
    // it ends, so a lock newer than the record means a later use.
    let mut lock = slot.as_os_str().to_owned();
    lock.push(".lock");
    let used_ms = std::fs::symlink_metadata(Path::new(&lock))
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
    if used_ms > record.released_at_ms {
        return Err("used again since its build lease ended".into());
    }
    match coder_lease::session_live(&record.lease_root, &record.session, record.agent_pid) {
        Ok(Some(why)) => Err(why),
        Ok(None) => Ok(format!(
            "its build lease ended and session {} ended",
            record.session
        )),
        Err(error) => Err(format!("the lease table can't be read: {error}")),
    }
}

/// Runs class 1 alone until the target slots' volume has `shortfall`
/// more bytes free than now: what a `build` lease's admission does before
/// it refuses for want of disk. Every class 1 check runs, and the run is
/// recorded under the disk rule.
///
/// # Errors
/// Another cleanup holds the run lock.
pub fn reclaim(env: &Env<'_>, shortfall: u64) -> Result<Report, String> {
    let layout = env.layout;
    let free = env
        .volumes
        .space(&layout.targets())
        .or_else(|_| env.volumes.space(&layout.openagents))
        .map_err(|error| error.to_string())?
        .free;
    let level = Level {
        bytes: free.saturating_add(shortfall),
        percent: 0,
    };
    let mut rule = crate::rule::disk();
    rule.goal.start = level;
    rule.goal.stop = level;
    rule.goal.emergency = Level {
        bytes: 0,
        percent: 0,
    };
    rule.goal.max_freed = u64::MAX;
    rule.actions = vec![Action::DeleteCaches {
        classes: vec![Class::EndedTargets],
    }];
    crate::run::run(env, &rule, Cause::Threshold, false, true)
}
