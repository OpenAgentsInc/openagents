//! A bounded subprocess run: the one supervision contract probes and
//! preflight both use, behind a synchronous door.
//!
//! The command runs through [`supervise::Job::from_command`], which keeps
//! the command's arguments, environment, and working directory, puts the
//! process in a group of its own, and drains both output streams to a
//! cap while they arrive. A probe that answers slowly is stopped — the
//! supervisor terminates the group and reaps the child — and a probe
//! that answers voluminously is truncated and marked, so output an
//! executor controls cannot grow a host's memory and cannot look like a
//! clean answer it did not give.
//!
//! `run` is synchronous because the callers — a survey, a preflight —
//! are. The job runs on a thread of its own with a current-thread
//! runtime, so calling it from inside an asynchronous host does not nest
//! one runtime inside another.

use std::process::Command;
use std::time::Duration;

use supervise::{Job, Limits};

/// The most a bounded run keeps from either stream. Well past any
/// reasonable version line; the point is the bound, not the size.
pub const OUTPUT_MAX: usize = 64 * 1024;

/// What a command answered, read back within [`OUTPUT_MAX`].
#[derive(Debug)]
pub struct Said {
    /// The exit code, or `None` when the process died on a signal.
    pub code: Option<i32>,
    /// What the command printed to stdout.
    pub out: String,
    /// What the command printed to stderr.
    pub err: String,
    /// Whether either stream overran the cap. A truncated answer is
    /// evidence the probe could not read — a caller treats it as unknown
    /// rather than reading the half it kept.
    pub truncated: bool,
}

/// Why a bounded run stopped without an answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// The wall clock ran out and the supervisor terminated the process
    /// group.
    TimedOut,
    /// The run itself failed — the command would not spawn, the
    /// supervisor could not read the ending, or the probe's own thread
    /// or runtime failed.
    Failed(String),
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::TimedOut => write!(f, "timed out"),
            Stop::Failed(why) => write!(f, "{why}"),
        }
    }
}

/// Runs `command` under `wall`, in a process group of its own, with the
/// caller's permissions.
///
/// An exit — any exit — is `Ok` with the code inside [`Said`], because
/// "exited 1" is an answer a caller classifies. `Err` is only the run
/// that produced no answer: the deadline expired or the process would
/// not run at all.
///
/// # Errors
///
/// [`Stop::TimedOut`] when the wall clock expired, [`Stop::Failed`] when
/// the command, the supervisor, or the probe's own thread could not run.
pub fn run(command: Command, wall: Duration) -> Result<Said, Stop> {
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                Stop::Failed(format!(
                    "couldn't start the runtime that runs the probe: {error}"
                ))
            })?;
        Ok(runtime.block_on(
            Job::from_command(command)
                .bounded(Limits::within(wall).keeping(OUTPUT_MAX))
                .run(),
        ))
    });
    let ended = worker
        .join()
        .map_err(|_| Stop::Failed("the probe's supervising thread panicked".to_string()))??;
    let truncated = ended.truncated();
    match ended.ending {
        supervise::Ending::Exited(code) => Ok(Said {
            code,
            out: ended.stdout.text,
            err: ended.stderr.text,
            truncated,
        }),
        supervise::Ending::TimedOut => Err(Stop::TimedOut),
        supervise::Ending::Failed(why) => Err(Stop::Failed(why)),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn command(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        command
    }

    #[test]
    fn a_bounded_run_reads_what_the_command_said() {
        let said = run(
            command("echo stdout-word; echo stderr-word >&2"),
            Duration::from_secs(5),
        )
        .expect("the run answers");
        assert_eq!(said.code, Some(0));
        assert!(said.out.contains("stdout-word"));
        assert!(said.err.contains("stderr-word"));
        assert!(!said.truncated);
    }

    #[test]
    fn a_hung_command_is_stopped_at_the_wall() {
        let started = Instant::now();
        let stop = run(command("sleep 30"), Duration::from_millis(200))
            .expect_err("a command that outlives its wall is a stop, not an answer");
        assert_eq!(stop, Stop::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_hung_commands_children_do_not_outlive_it() {
        // The process group, not just the child: a probe that spawns a
        // sleeper and waits is stopped at the wall, and the sleeper is
        // terminated with it.
        let marker = tempfile::tempdir().unwrap().path().join("still-running");
        let script = format!("sleep 30 & sleep 30; touch {}", marker.display());
        let stop =
            run(command(&script), Duration::from_millis(300)).expect_err("a hung probe is a stop");
        assert_eq!(stop, Stop::TimedOut);
    }

    #[test]
    fn a_command_that_fails_to_spawn_fails_the_run() {
        let stop = run(
            Command::new("no-such-binary-openagents-definitely"),
            Duration::from_secs(5),
        )
        .expect_err("a binary that does not exist cannot answer");
        assert!(matches!(stop, Stop::Failed(_)));
    }

    #[test]
    fn voluminous_output_is_kept_to_the_cap_and_marked() {
        // Well past OUTPUT_MAX: the bound is what keeps this from being
        // a memory question.
        let said = run(
            command("head -c 200000 /dev/zero | tr '\\0' 'x'"),
            Duration::from_secs(10),
        )
        .expect("the run answers");
        assert!(said.out.len() <= OUTPUT_MAX);
        assert!(said.truncated, "the cap marks the answer it cut");
    }

    #[test]
    fn a_nonzero_exit_is_an_answer_not_a_stop() {
        let said = run(command("exit 3"), Duration::from_secs(5)).expect("exiting is answering");
        assert_eq!(said.code, Some(3));
    }

    /// The door has to stay synchronous without panicking inside a host
    /// that already runs a runtime — the survey's own context.
    #[tokio::test]
    async fn a_probe_runs_from_inside_an_async_host() {
        let said = run(command("echo nested"), Duration::from_secs(5))
            .expect("the probe answers from inside a runtime");
        assert!(said.out.contains("nested"));
    }
}
