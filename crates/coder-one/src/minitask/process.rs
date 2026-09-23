//! Bounded child processes for graders and check scenarios: run one to
//! its end, or run one, wait until it reports ready, interrupt it, and
//! watch it clean up.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How a bounded child ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ran {
    /// The exit code, or `None` when a signal ended it or the host killed
    /// it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    /// Whether the host killed it at its deadline.
    pub killed: bool,
    pub milliseconds: u64,
}

/// The first `python3` on `PATH`.
#[must_use]
pub fn python() -> Option<PathBuf> {
    which("python3")
}

/// The first `name` on `PATH`.
#[must_use]
pub fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Runs `command` to its end under `deadline` through the supervisor,
/// which ends the whole process group at the deadline.
pub async fn run(command: Command, deadline: Duration) -> Ran {
    let started = Instant::now();
    let ended = supervise::Job::from_command(command)
        .bounded(supervise::Limits::within(deadline).keeping(256 * 1024))
        .run()
        .await;
    Ran {
        code: match ended.ending {
            supervise::Ending::Exited(code) => code,
            _ => None,
        },
        killed: matches!(ended.ending, supervise::Ending::TimedOut),
        stdout: ended.stdout.marked(),
        stderr: ended.stderr.marked(),
        milliseconds: millis(started),
    }
}

/// What an interrupted child did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interrupted {
    /// Whether it reported ready before the readiness deadline.
    pub ready: bool,
    /// Milliseconds from spawn to the interrupt, when one was sent.
    pub interrupted_at_ms: Option<u64>,
    pub ran: Ran,
}

/// Spawns `command` with standard output and error to files under
/// `scratch`, waits until `ready` says the child is ready (checked every
/// 10 ms, up to `ready_within`), sends it SIGINT, and waits up to
/// `exit_within` for it to exit before killing it.
pub async fn interrupt_when(
    mut command: Command,
    scratch: &Path,
    ready: &dyn Fn() -> bool,
    ready_within: Duration,
    exit_within: Duration,
) -> Interrupted {
    let started = Instant::now();
    let out = scratch.join("child.stdout");
    let err = scratch.join("child.stderr");
    let (Ok(stdout), Ok(stderr)) = (std::fs::File::create(&out), std::fs::File::create(&err))
    else {
        return Interrupted {
            ready: false,
            interrupted_at_ms: None,
            ran: Ran {
                code: None,
                stdout: String::new(),
                stderr: format!("cannot create output files under {}", scratch.display()),
                killed: false,
                milliseconds: 0,
            },
        };
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Interrupted {
                ready: false,
                interrupted_at_ms: None,
                ran: Ran {
                    code: None,
                    stdout: String::new(),
                    stderr: format!("cannot start the child: {error}"),
                    killed: false,
                    milliseconds: 0,
                },
            };
        }
    };
    let mut is_ready = false;
    while started.elapsed() < ready_within {
        if ready() {
            is_ready = true;
            break;
        }
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut interrupted_at_ms = None;
    if matches!(child.try_wait(), Ok(None)) {
        interrupted_at_ms = Some(millis(started));
        let _ = Command::new("kill")
            .arg("-INT")
            .arg(child.id().to_string())
            .status();
    }
    let waiting = Instant::now();
    let mut killed = false;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if waiting.elapsed() < exit_within => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            _ => {
                killed = true;
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    Interrupted {
        ready: is_ready,
        interrupted_at_ms,
        ran: Ran {
            code,
            stdout: std::fs::read_to_string(&out).unwrap_or_default(),
            stderr: std::fs::read_to_string(&err).unwrap_or_default(),
            killed,
            milliseconds: millis(started),
        },
    }
}

/// Waits up to `within` for `path` to hold content `accept` takes,
/// checking every 20 ms. Returns the last content read.
pub async fn wait_for_file(
    path: &Path,
    within: Duration,
    accept: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let started = Instant::now();
    loop {
        let text = std::fs::read_to_string(path).ok();
        if text.as_deref().is_some_and(accept) || started.elapsed() >= within {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
