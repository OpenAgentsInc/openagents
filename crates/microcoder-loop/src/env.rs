//! Where commands run: a local directory, a local directory inside a
//! `coder-boundary` boundary, or a container reached with `docker exec`.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use crate::state::{CommandResult, OUTPUT_HEAD, OUTPUT_TAIL, cut};

/// Bytes of a command's output read, at most. The rest is drained unread.
pub const READ_MAX: usize = 1024 * 1024;

/// Characters of one file a [`Env::read`] returns, at most.
pub const FILE_MAX: usize = 40_000;

/// A place commands run.
pub trait Env {
    /// Runs `command` as a bash script (sh when there's no bash), fed on
    /// standard input so it needs no quoting, stopping it at `deadline`.
    fn run(
        &self,
        command: &str,
        deadline: Duration,
    ) -> impl std::future::Future<Output = CommandResult>;

    /// A file's contents, up to [`FILE_MAX`] characters, or `None` when
    /// there's no such file. A relative path is from the working
    /// directory.
    fn read(&self, path: &str) -> impl std::future::Future<Output = Option<String>>;

    /// Whether the run was stopped from outside the loop: its task was
    /// cancelled, reached its host's deadline, or its host refused to go
    /// on. Once it is true the loop makes no further model call or command
    /// and ends with [`crate::run::Ending::Stopped`]. A place nothing stops
    /// from outside never is.
    fn stopped(&self) -> bool {
        false
    }

    /// Messages the person sent the running turn since the last call, in
    /// order: the loop reads them at the start of each step, and before it
    /// lets a step finish. Taking them consumes them.
    fn steering(&self) -> Vec<String> {
        Vec::new()
    }
}

/// A local working directory.
#[derive(Clone, Debug)]
pub struct Local {
    pub dir: PathBuf,
}

/// A running container.
#[derive(Clone, Debug)]
pub struct Docker {
    pub container: String,
    pub workdir: String,
    /// The user commands run as; `None` is the image's user.
    pub user: Option<String>,
}

/// The shell a script runs under: bash when there is one.
const SHELL: &str = "if command -v bash >/dev/null 2>&1; then exec bash -s; else exec sh -s; fi";

/// Sets the file-creation mask every command in a task container starts
/// with. `docker exec` inherits the daemon's umask, which is 0000 on some
/// hosts, such as coderos-4080, so files a command creates would be
/// world-writable, and a task that checks permissions fails: sshd refuses
/// a world-writable `/run/sshd` in Terminal-Bench 2.1's `git-multibranch`.
/// Harbor's Beam environment sets `umask 022` the same way, and its Docker
/// environment gets 022 from a stock daemon.
pub const UMASK: &str = "umask 022";

/// The names among `vars` that hold a credential, by the workspace's one
/// policy. Names that are not valid Unicode are judged lossily, so a
/// non-UTF-8 variable can neither crash a step nor slip a key through.
fn credential_vars(
    vars: impl Iterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) -> Vec<std::ffi::OsString> {
    vars.map(|(name, _)| name)
        .filter(|name| acp_client::process::is_credential_name(&name.to_string_lossy()))
        .collect()
}

/// Remove every credential in this process's environment from `child`'s.
fn scrub_credentials(child: &mut Command) {
    for name in credential_vars(std::env::vars_os()) {
        child.env_remove(name);
    }
}

impl Env for Local {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        let mut child = Command::new("sh");
        child.arg("-c").arg(SHELL).current_dir(&self.dir);
        // Heavy `cargo` commands take a build lease through the shim first
        // on `PATH`, once this process turned the shims on.
        child.envs(coder_lease::shim::delegate_vars_here());
        // The model's commands never see a key.
        scrub_credentials(&mut child);
        execute(child, command, deadline).await
    }

    async fn read(&self, path: &str) -> Option<String> {
        let text = std::fs::read(self.dir.join(path)).ok()?;
        Some(cut(&String::from_utf8_lossy(&text), FILE_MAX, 0))
    }
}

/// A local working directory whose commands run inside a
/// `coder-boundary` boundary: read-only, or writing only the directory,
/// with a private scratch directory as `TMPDIR` either way. Coder's
/// delegate door runs a turn's commands here, under the turn's permit.
#[derive(Debug)]
pub struct Bounded {
    pub dir: PathBuf,
    boundary: coder_boundary::Boundary,
}

impl Bounded {
    /// Commands in `dir`, which they may write only when `writes` is set.
    ///
    /// # Errors
    ///
    /// The boundary's refusal, in a sentence, when this host cannot
    /// enforce one.
    pub fn new(dir: PathBuf, writes: bool) -> Result<Self, String> {
        let spec = if writes {
            coder_boundary::Boundary::writing(&dir)
        } else {
            coder_boundary::Boundary::readonly().protecting(&dir)
        };
        let boundary = spec
            .owned_scratch_under(std::env::temp_dir())
            .build()
            .map_err(|error| format!("cannot bound the commands: {error}"))?;
        Ok(Bounded { dir, boundary })
    }

    /// Whether the commands may write the directory.
    #[must_use]
    pub fn writes(&self) -> bool {
        self.boundary.checkout().is_some()
    }
}

impl Env for Bounded {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        let wrapped = match self.boundary.command("/bin/sh", ["-c", SHELL]) {
            Ok(wrapped) => wrapped,
            Err(error) => {
                return CommandResult {
                    command: command.to_string(),
                    exit: None,
                    timed_out: false,
                    seconds: 0.0,
                    output: format!("the boundary refused the command: {error}"),
                };
            }
        };
        let mut child = Command::from(wrapped);
        child.current_dir(&self.dir);
        if let Some(scratch) = self.boundary.scratch() {
            child.env("TMPDIR", scratch);
        }
        scrub_credentials(&mut child);
        execute(child, command, deadline).await
    }

    async fn read(&self, path: &str) -> Option<String> {
        let text = std::fs::read(self.dir.join(path)).ok()?;
        Some(cut(&String::from_utf8_lossy(&text), FILE_MAX, 0))
    }
}

impl Env for Docker {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        // `timeout` inside the container ends the script itself; killing
        // `docker exec` alone would leave it running.
        let seconds = deadline.as_secs().max(1);
        let mut child = Command::new("docker");
        child.arg("exec");
        if let Some(user) = &self.user {
            child.args(["-u", user]);
        }
        child.args([
            "-i",
            "-w",
            &self.workdir,
            &self.container,
            "sh",
            "-c",
            &format!(
                "{UMASK}; if command -v timeout >/dev/null 2>&1; then exec timeout -k 5 {seconds} sh -c '{SHELL}'; else {SHELL}; fi"
            ),
        ]);
        execute(child, command, deadline + Duration::from_secs(10)).await
    }

    async fn read(&self, path: &str) -> Option<String> {
        let mut command = Command::new("docker");
        command.arg("exec");
        if let Some(user) = &self.user {
            command.args(["-u", user]);
        }
        let output = command
            .args(["-w", &self.workdir, &self.container, "cat", "--", path])
            .stdin(Stdio::null())
            .output()
            .await
            .ok()?;
        output
            .status
            .success()
            .then(|| cut(&String::from_utf8_lossy(&output.stdout), FILE_MAX, 0))
    }
}

async fn execute(mut child: Command, command: &str, deadline: Duration) -> CommandResult {
    let started = Instant::now();
    child
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut process = match child.spawn() {
        Ok(process) => process,
        Err(error) => {
            return CommandResult {
                command: command.to_string(),
                exit: None,
                timed_out: false,
                seconds: 0.0,
                output: format!("couldn't start: {error}"),
            };
        }
    };
    let stdin = process.stdin.take();
    let script = format!("{command}\n");
    let feed = async move {
        if let Some(mut stdin) = stdin {
            let _ = stdin.write_all(script.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }
    };
    let stdout = process.stdout.take();
    let stderr = process.stderr.take();
    let read = async {
        let ((), out, err) = tokio::join!(feed, read_capped(stdout), read_capped(stderr));
        let status = process.wait().await.ok();
        (out, err, status)
    };
    let (output, exit, timed_out) = match tokio::time::timeout(deadline, read).await {
        Ok((out, err, status)) => {
            let mut text = out;
            if !err.is_empty() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            let exit = status.and_then(|s| s.code());
            // `timeout` exits 124 when it ends the script.
            let timed_out = exit == Some(124);
            (text, exit, timed_out)
        }
        Err(_) => (
            String::from("[the host stopped the command at its deadline]"),
            None,
            true,
        ),
    };
    CommandResult {
        command: command.to_string(),
        exit,
        timed_out,
        seconds: started.elapsed().as_secs_f64(),
        output: cut(&output, OUTPUT_HEAD, OUTPUT_TAIL),
    }
}

async fn read_capped<R: tokio::io::AsyncRead + Unpin>(source: Option<R>) -> String {
    let Some(mut source) = source else {
        return String::new();
    };
    let mut kept = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match source.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if kept.len() < READ_MAX {
                    let room = READ_MAX - kept.len();
                    kept.extend_from_slice(&chunk[..n.min(room)]);
                }
            }
        }
    }
    String::from_utf8_lossy(&kept).to_string()
}

#[cfg(test)]
mod tests {

    #[test]
    #[cfg(unix)]
    fn credential_scrubbing_survives_non_utf8_names_and_catches_more_than_suffixes() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let vars = [
            (
                OsString::from_vec(vec![b'X', 0xff, b'Y']),
                OsString::from("v"),
            ),
            (
                OsString::from_vec(b"BAD\xff_TOKEN".to_vec()),
                OsString::from("v"),
            ),
            (OsString::from("PATH"), OsString::from_vec(vec![0xfe])),
            (OsString::from("aws_secret_access_key"), OsString::from("v")),
            (OsString::from("DB_PASSWORD"), OsString::from("v")),
            (OsString::from("OPENAI_API_KEY"), OsString::from("v")),
            (OsString::from("HOME"), OsString::from("v")),
        ];
        let removed = super::credential_vars(vars.into_iter());
        assert_eq!(
            removed,
            [
                OsString::from_vec(b"BAD\xff_TOKEN".to_vec()),
                OsString::from("aws_secret_access_key"),
                OsString::from("DB_PASSWORD"),
                OsString::from("OPENAI_API_KEY"),
            ]
        );
    }
    use super::*;

    #[tokio::test]
    // The script runs under `sh`, which Windows does not ship.
    #[cfg(unix)]
    async fn a_local_command_reports_its_output_and_exit() {
        let dir = std::env::temp_dir();
        let env = Local { dir };
        let ok = env
            .run("echo hi; echo oops >&2", Duration::from_secs(5))
            .await;
        assert!(ok.ok());
        assert!(ok.output.contains("hi") && ok.output.contains("oops"));
        let failed = env.run("exit 3", Duration::from_secs(5)).await;
        assert_eq!(failed.exit, Some(3));
        assert!(!failed.ok());
    }

    #[tokio::test]
    // The script runs under `sh`, which Windows does not ship.
    #[cfg(unix)]
    async fn a_script_with_quotes_and_a_heredoc_runs_as_written() {
        let dir = std::env::temp_dir().join(format!("microcoder-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let env = Local { dir: dir.clone() };
        let script = "cat > note.py <<'PY'\nprint(\"it's here\")\nPY\npython3 note.py";
        let result = env.run(script, Duration::from_secs(10)).await;
        assert!(result.ok(), "{}", result.output);
        assert!(result.output.contains("it's here"));
        assert_eq!(
            env.read("note.py").await.as_deref(),
            Some("print(\"it's here\")\n")
        );
        assert_eq!(env.read("missing.txt").await, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    // The script runs under `sh`, which Windows does not ship.
    #[cfg(unix)]
    async fn a_local_command_past_its_deadline_is_stopped() {
        let env = Local {
            dir: std::env::temp_dir(),
        };
        let result = env.run("sleep 5", Duration::from_millis(200)).await;
        assert!(result.timed_out);
        assert!(result.seconds < 4.0);
    }

    #[tokio::test]
    async fn a_bounded_directory_is_written_only_when_the_boundary_writes() {
        let dir = tempfile::tempdir().unwrap();
        let reading = match Bounded::new(dir.path().to_path_buf(), false) {
            Ok(env) => env,
            Err(why) => {
                eprintln!("skipped: {why}");
                return;
            }
        };
        assert!(!reading.writes());
        let refused = reading
            .run("printf x > marker", Duration::from_secs(10))
            .await;
        assert!(!refused.ok(), "{}", refused.output);
        assert!(!dir.path().join("marker").exists());
        // The private scratch directory is writable either way.
        let scratch = reading
            .run(
                "printf y > \"$TMPDIR/note\" && cat \"$TMPDIR/note\"",
                Duration::from_secs(10),
            )
            .await;
        assert!(scratch.ok(), "{}", scratch.output);
        let writing = Bounded::new(dir.path().to_path_buf(), true).unwrap();
        assert!(writing.writes());
        let wrote = writing
            .run("printf x > marker", Duration::from_secs(10))
            .await;
        assert!(wrote.ok(), "{}", wrote.output);
        assert_eq!(writing.read("marker").await.as_deref(), Some("x"));
    }

    #[tokio::test]
    // The script runs under `sh`, which Windows does not ship.
    #[cfg(unix)]
    async fn the_models_commands_never_see_a_key() {
        // SAFETY: tests in this module don't read this variable concurrently.
        unsafe { std::env::set_var("MICROCODER_TEST_API_KEY", "secret-value") };
        let env = Local {
            dir: std::env::temp_dir(),
        };
        let result = env
            .run(
                "echo \"[$MICROCODER_TEST_API_KEY]\"",
                Duration::from_secs(5),
            )
            .await;
        assert!(result.output.contains("[]"), "{}", result.output);
    }
}
