//! Spawning one job's direct child so the supervisor owns its tree from the
//! child's first instruction, under its memory cap.
//!
//! [`crate::Job::run`] and [`crate::Job::start`] share this; they differ only
//! in what they do with the child afterwards.

use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::group::Tree;
#[cfg(unix)]
use crate::memory;
use crate::memory::Placed;

/// A child that started, the tree it leads, and where its cap ended up.
pub(crate) struct Spawned {
    pub(crate) child: Child,
    pub(crate) tree: Tree,
    pub(crate) placed: Option<Placed>,
}

/// A child that never started, and a cap that may still need clearing
/// away.
pub(crate) struct Unspawned {
    pub(crate) why: String,
    pub(crate) placed: Option<Placed>,
}

/// Spawns `prepared` with standard input `stdin` and both output streams
/// piped, in a tree of its own, under `memory_max` bytes when that is set.
///
/// On Unix the child leads a new process group. With a cap, it waits
/// between `fork` and `exec` until the helper has placed it, so the spawn
/// returns once it is placed.
#[cfg(unix)]
pub(crate) fn spawn(
    mut prepared: std::process::Command,
    memory_max: Option<u64>,
    stdin: Stdio,
) -> Result<Spawned, Unspawned> {
    let handshake = memory_max
        .map(|max| memory::arm(&mut prepared, max))
        .transpose()
        .map_err(|why| Unspawned { why, placed: None })?;
    let mut command = Command::from(prepared);
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The group is what this supervisor terminates. `kill_on_drop` is
        // kept underneath it as the last resort for the direct child if the
        // runtime itself goes away mid-cleanup.
        .process_group(0)
        .kill_on_drop(true);
    let serving = handshake.map(memory::Handshake::serve);
    let spawned = command.spawn();
    let placed = serving.map(memory::Serving::finish);
    match spawned {
        // `process_group(0)` makes the child the leader of a new group, so
        // the group's identifier is the child's own.
        Ok(child) => Ok(Spawned {
            tree: Tree::led_by(child.id()),
            child,
            placed,
        }),
        // A child that failed to execute may still have been placed, and
        // its scope is cleared away like any other.
        Err(error) => Err(Unspawned {
            why: placed
                .as_ref()
                .map_or_else(|| error.to_string(), |placed| placed.refusal(&error)),
            placed,
        }),
    }
}

/// Spawns `prepared` with standard input `stdin` and both output streams
/// piped, in a job object of its own, under `memory_max` bytes when that is
/// set.
///
/// On Windows the child starts suspended, joins a new job object, and only
/// then runs, so everything it starts is in the job. The job kills what is
/// left in it when its last handle closes, and holds the tree's committed
/// memory to the cap.
#[cfg(windows)]
pub(crate) fn spawn(
    prepared: std::process::Command,
    memory_max: Option<u64>,
    stdin: Stdio,
) -> Result<Spawned, Unspawned> {
    let unspawned = |why: String| Unspawned { why, placed: None };
    let job = crate::windows::JobObject::new(memory_max)
        .map_err(|error| unspawned(format!("couldn't create the job's job object: {error}")))?;
    let mut command = Command::from(prepared);
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(crate::windows::SUSPENDED)
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| unspawned(error.to_string()))?;
    let (Some(pid), Some(process)) = (child.id(), child.raw_handle()) else {
        return Err(unspawned("the job exited before it started".to_string()));
    };
    let tree = match job.adopt(process, pid) {
        Ok(tree) => tree,
        Err(error) => {
            // The child never ran a single instruction; it goes before it
            // can start anything outside a job.
            let _ = child.start_kill();
            return Err(unspawned(format!(
                "couldn't put the job in its job object: {error}"
            )));
        }
    };
    let placed = memory_max.map(|max| Placed::new(max, &tree));
    Ok(Spawned {
        child,
        tree,
        placed,
    })
}
