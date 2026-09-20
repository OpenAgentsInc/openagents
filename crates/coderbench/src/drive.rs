//! Running Coder from a script and reading back what it left.
//!
//! Headless mode is what makes this possible: `coder -p` runs one turn with
//! no terminal, `--trace` names the file the trace lands in, and the exit
//! code carries the outcome. This module is the caller those choices were
//! made for. It names the trace rather than globbing for it, and it reads
//! the exit code rather than treating anything non-zero as a failure —
//! declining is a result, and a harness that could not tell it from a dead
//! door would score the two the same.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How the turn ended, as the exit code reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The turn finished and the agent answered.
    Answered,
    /// The turn did not finish. The door failed, or the trace could not be
    /// opened.
    Failed,
    /// The turn finished and the router declined it. Nothing went wrong.
    Declined,
    /// The command line was wrong, which is the harness's fault rather than
    /// the agent's.
    Usage,
    /// The turn ran past the task's timeout and was stopped.
    TimedOut,
    /// An exit code headless mode does not document.
    Unknown(i32),
    /// A signal, so there is no exit code.
    Signal,
}

impl Outcome {
    /// Reads an exit code as headless mode documents it.
    #[must_use]
    pub fn of(code: Option<i32>) -> Self {
        match code {
            Some(0) => Self::Answered,
            Some(1) => Self::Failed,
            Some(2) => Self::Declined,
            Some(64) => Self::Usage,
            Some(other) => Self::Unknown(other),
            None => Self::Signal,
        }
    }

    /// Whether the turn ran to its own conclusion, either way.
    #[must_use]
    pub fn finished(self) -> bool {
        matches!(self, Self::Answered | Self::Declined)
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Answered => write!(f, "answered"),
            Self::Failed => write!(f, "did not finish"),
            Self::Declined => write!(f, "declined"),
            Self::Usage => write!(f, "refused the command line"),
            Self::TimedOut => write!(f, "ran past the timeout"),
            Self::Unknown(code) => write!(f, "exited {code}"),
            Self::Signal => write!(f, "died on a signal"),
        }
    }
}

/// One headless run of Coder.
#[derive(Clone, Debug)]
pub struct Run {
    pub outcome: Outcome,
    /// The trace the run was told to write. It exists when the turn got far
    /// enough to record anything.
    pub trace: PathBuf,
    /// Where standard output and standard error were kept, so a reader can
    /// see what the agent said as well as what it did.
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub seconds: f64,
}

/// Where the `coder` binary is.
///
/// `--coder` names one. Otherwise `CODERBENCH_CODER` does, then the binary
/// beside this one, which is what a `cargo run` in this workspace wants,
/// and then `PATH`.
///
/// # Errors
///
/// Returns an error when none of those resolve, because a run that cannot
/// find the agent has nothing to report about it.
pub fn find_coder(named: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(named) = named {
        return here(named);
    }
    if let Ok(named) = std::env::var("CODERBENCH_CODER") {
        return here(Path::new(&named)).map_err(|error| format!("CODERBENCH_CODER names {error}"));
    }
    if let Ok(mine) = std::env::current_exe()
        && let Some(beside) = mine.parent().map(|directory| directory.join("coder"))
        && beside.exists()
    {
        return Ok(beside);
    }
    crate::preflight::resolve("coder")
        .ok_or_else(|| "no coder binary — build one, or name it with --coder".to_string())
}

/// Reads a named binary as a path from here.
///
/// The run happens in the repository being measured rather than in this
/// directory, so a relative path would resolve somewhere the caller did not
/// mean. Every path this hands on is absolute.
fn here(named: &Path) -> Result<PathBuf, String> {
    let named = absolute(named);
    if named.exists() {
        Ok(named)
    } else {
        Err(format!("{}, which is not there", named.display()))
    }
}

/// A path as it reads from this directory.
#[must_use]
pub fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Runs one turn and hands back where it landed.
///
/// The trace is named rather than searched for, and a path that already
/// exists is refused here rather than by Coder: a named trace it cannot
/// open ends the run before the turn, so passing one would spend a run to
/// learn what this can say first.
///
/// # Errors
///
/// Returns an error when the trace path is taken, the output files cannot
/// be opened, or the binary will not start.
pub fn coder(
    binary: &Path,
    repository: &Path,
    request: &str,
    trace: &Path,
    timeout: Duration,
) -> Result<Run, String> {
    if trace.exists() {
        return Err(format!(
            "{} already exists, and a session never writes over another session's record",
            trace.display()
        ));
    }
    if let Some(directory) = trace.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
    }
    let stdout = with_suffix(trace, "stdout");
    let stderr = with_suffix(trace, "stderr");
    let mut command = Command::new(binary);
    command
        .current_dir(repository)
        .arg("--print")
        .arg("--json")
        .arg("--trace")
        .arg(trace)
        .arg("--")
        .arg(request)
        .stdin(Stdio::null())
        .stdout(Stdio::from(create(&stdout)?))
        .stderr(Stdio::from(create(&stderr)?));

    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("{} will not start — {error}", binary.display()))?;
    let outcome = match wait(&mut child, timeout) {
        Some(code) => Outcome::of(code),
        None => Outcome::TimedOut,
    };
    Ok(Run {
        outcome,
        trace: trace.to_path_buf(),
        stdout,
        stderr,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// What a short command said.
#[derive(Clone, Debug)]
pub struct Said {
    pub code: Option<i32>,
    pub out: String,
    pub err: String,
}

/// Runs a command that is expected to answer quickly and reads its output.
///
/// # Errors
///
/// Returns an error when the command will not start, or does not answer
/// inside `timeout`.
pub fn output(mut command: Command, timeout: Duration) -> Result<Said, String> {
    // Output goes to temporary files rather than pipes, because a pipe
    // nobody is reading fills and stops the child, and this waits rather
    // than reads.
    let directory = std::env::temp_dir().join(format!("coderbench-{}", std::process::id()));
    std::fs::create_dir_all(&directory).map_err(|error| format!("{error}"))?;
    let stem = format!("{:x}-{}", now_nanos(), next());
    let out = directory.join(format!("{stem}.out"));
    let err = directory.join(format!("{stem}.err"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(create(&out)?))
        .stderr(Stdio::from(create(&err)?));
    let mut child = command.spawn().map_err(|error| format!("{error}"))?;
    let code = wait(&mut child, timeout).ok_or_else(|| format!("no answer in {timeout:?}"))?;
    let said = Said {
        code,
        out: std::fs::read_to_string(&out).unwrap_or_default(),
        err: std::fs::read_to_string(&err).unwrap_or_default(),
    };
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&err);
    Ok(said)
}

/// Waits for a child, stopping it when `timeout` passes. `None` means it
/// was stopped.
fn wait(child: &mut std::process::Child, timeout: Duration) -> Option<Option<i32>> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.code()),
            Ok(None) => {}
            Err(_) => return Some(None),
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Opens a file for a child to write to.
fn create(path: &Path) -> Result<std::fs::File, String> {
    std::fs::File::create(path).map_err(|error| format!("{}: {error}", path.display()))
}

/// `one.atif.jsonl` and `stdout` make `one.atif.jsonl.stdout`, so the three
/// files a run leaves sort together.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

/// Nanoseconds since the epoch, for a temporary name no concurrent run
/// takes.
fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

/// A number no other call in this process takes.
///
/// The clock is not enough on its own. Two calls from different threads can
/// read the same nanosecond, and then one writes over the other's output
/// file and deletes it — which reads as a command that answered with
/// nothing rather than as two callers sharing a name.
fn next() -> u64 {
    static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    TAKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented exit codes read back as what they mean, and
    /// declining is not failing.
    #[test]
    fn an_exit_code_is_an_outcome() {
        assert_eq!(Outcome::of(Some(0)), Outcome::Answered);
        assert_eq!(Outcome::of(Some(1)), Outcome::Failed);
        assert_eq!(Outcome::of(Some(2)), Outcome::Declined);
        assert_eq!(Outcome::of(Some(64)), Outcome::Usage);
        assert!(Outcome::Declined.finished());
        assert!(!Outcome::Failed.finished());
    }

    /// A short command answers, and one that never would is stopped.
    #[test]
    fn a_command_answers_or_is_stopped() {
        let mut echo = Command::new("sh");
        echo.args(["-c", "echo hello"]);
        let said = output(echo, Duration::from_secs(5)).unwrap();
        assert_eq!(said.code, Some(0));
        assert_eq!(said.out.trim(), "hello");

        let mut forever = Command::new("sh");
        forever.args(["-c", "sleep 30"]);
        assert!(output(forever, Duration::from_millis(200)).is_err());
    }

    /// A trace path that is taken is refused before anything runs, because
    /// Coder would refuse it too and this can say so without spending a
    /// run.
    #[test]
    fn a_taken_trace_is_refused_before_the_run() {
        let directory = tempfile::tempdir().unwrap();
        let trace = directory.path().join("taken.atif.jsonl");
        std::fs::write(&trace, "").unwrap();
        let error = coder(
            Path::new("/bin/echo"),
            directory.path(),
            "hello",
            &trace,
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("already exists"), "{error}");
    }
}
