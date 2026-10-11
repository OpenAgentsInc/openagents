//! One way to run the local command behind a paid route.
//!
//! The command runs under `supervise`: its own process group, a wall-clock
//! deadline, and a cap on what it may print. A command that never exits,
//! or one that prints past the cap, ends as an error instead of holding the
//! paid path open, and its whole group is cleaned up either way.

use std::time::Duration;

use supervise::{Ending, Input, Job, Limits};

/// How long a paid command may run.
pub(crate) const PAID_DEADLINE: Duration = Duration::from_secs(120);

/// The bytes a paid command may print on stdout (and, separately, stderr).
pub(crate) const PAID_OUTPUT_MAX: usize = 8 * 1024 * 1024;

/// The stderr characters an error message carries.
const STDERR_TAIL: usize = 512;

/// Why a paid command gave no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PaidError {
    /// The command could not be started.
    Spawn(String),
    /// The command ran past its deadline and was stopped.
    TimedOut,
    /// The command printed more than the cap.
    TooLarge,
    /// The command ended with a failure; the text is what it said on stderr.
    Exited { ending: Ending, stderr: String },
}

impl PaidError {
    /// The message for a command named `program`.
    pub(crate) fn describe(&self, program: &str) -> String {
        match self {
            PaidError::Spawn(why) => format!("spawn {program}: {why}"),
            PaidError::TimedOut => format!("{program} timed out"),
            PaidError::TooLarge => format!("{program} output exceeded the limit"),
            PaidError::Exited { ending, stderr } if stderr.is_empty() => {
                format!("{program} exited with {ending}")
            }
            PaidError::Exited { ending, stderr } => {
                format!("{program} exited with {ending}: {stderr}")
            }
        }
    }
}

/// Run `program` with `args` and `env`, `stdin` on its standard input, and
/// return its stdout when it exits zero within the default limits.
pub(crate) fn paid_exec(
    program: &str,
    args: &[String],
    env: &[(String, String)],
    stdin: &[u8],
) -> Result<Vec<u8>, PaidError> {
    paid_exec_within(program, args, env, stdin, PAID_DEADLINE, PAID_OUTPUT_MAX)
}

/// [`paid_exec`] with an explicit deadline and output cap.
pub(crate) fn paid_exec_within(
    program: &str,
    args: &[String],
    env: &[(String, String)],
    stdin: &[u8],
    deadline: Duration,
    output_max: usize,
) -> Result<Vec<u8>, PaidError> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    for (name, value) in env {
        command.env(name, value);
    }
    let job = Job::from_command(command).bounded(Limits::within(deadline).keeping(output_max));
    // A runtime of its own on a thread of its own, so a caller inside an
    // async server and a plain synchronous caller both work.
    std::thread::scope(|scope| {
        scope
            .spawn(|| crate::runtime().block_on(supervised(job, stdin)))
            .join()
            .unwrap_or_else(|_| Err(PaidError::Spawn("the supervisor stopped".to_string())))
    })
}

async fn supervised(job: Job, stdin: &[u8]) -> Result<Vec<u8>, PaidError> {
    let mut live = job.start(Input::Piped).map_err(PaidError::Spawn)?;
    // A command that exits without reading its input still gets judged on
    // how it exited, so a failed write is not itself the answer. The
    // deadline bounds a write to a command that never reads.
    let _ = live.send(stdin).await;
    live.close_input();
    let stopped = live.wait().await;
    match stopped.ending {
        Ending::TimedOut => return Err(PaidError::TimedOut),
        Ending::Failed(why) => return Err(PaidError::Spawn(why)),
        Ending::Exited(Some(0)) => {}
        ending => {
            return Err(PaidError::Exited {
                ending,
                stderr: tail(stopped.stderr.text.trim()),
            });
        }
    }
    let rest = stopped.rest;
    if !rest.gaps.is_empty() || stopped.stdout_bytes > rest.bytes.len() as u64 {
        return Err(PaidError::TooLarge);
    }
    Ok(rest.bytes)
}

fn tail(text: &str) -> String {
    let count = text.chars().count();
    if count <= STDERR_TAIL {
        return text.to_string();
    }
    let kept: String = text.chars().skip(count - STDERR_TAIL).collect();
    format!("…{kept}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn the_body_goes_in_and_stdout_comes_back() {
        let out = paid_exec("cat", &[], &[], b"paid body").unwrap();
        assert_eq!(out, b"paid body");
    }

    #[test]
    fn the_environment_reaches_the_command() {
        let env = vec![("PAID_EXEC_PROBE".to_string(), "yes".to_string())];
        let out = paid_exec(
            "sh",
            &words(&["-c", "printf %s \"$PAID_EXEC_PROBE\""]),
            &env,
            b"",
        )
        .unwrap();
        assert_eq!(out, b"yes");
    }

    #[test]
    fn a_command_past_its_deadline_is_stopped() {
        let started = Instant::now();
        let error = paid_exec_within(
            "sleep",
            &words(&["30"]),
            &[],
            b"",
            Duration::from_millis(200),
            PAID_OUTPUT_MAX,
        )
        .unwrap_err();
        assert_eq!(error, PaidError::TimedOut);
        assert!(error.describe("sleep").contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn output_past_the_cap_is_an_error_not_a_short_answer() {
        let error = paid_exec_within(
            "head",
            &words(&["-c", "2000000", "/dev/zero"]),
            &[],
            b"",
            Duration::from_secs(30),
            8 * 1024,
        )
        .unwrap_err();
        assert_eq!(error, PaidError::TooLarge);
    }

    #[test]
    fn a_failed_exit_is_an_error_with_what_it_said() {
        let error =
            paid_exec("sh", &words(&["-c", "echo nope >&2; exit 3"]), &[], b"").unwrap_err();
        let text = error.describe("sh");
        assert!(text.contains("exit 3"), "{text}");
        assert!(text.contains("nope"), "{text}");
    }

    #[test]
    fn a_missing_program_does_not_start() {
        let error = paid_exec("/nonexistent/paid-exec-probe", &[], &[], b"").unwrap_err();
        assert!(matches!(error, PaidError::Spawn(_)));
    }
}
