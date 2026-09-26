//! Where commands run: a local directory, or a container reached with
//! `docker exec`.

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

impl Env for Local {
    async fn run(&self, command: &str, deadline: Duration) -> CommandResult {
        let mut child = Command::new("sh");
        child.arg("-c").arg(SHELL).current_dir(&self.dir);
        // The model's commands never see a key.
        for (name, _) in std::env::vars() {
            if name.ends_with("_API_KEY") || name.ends_with("_TOKEN") || name.ends_with("_SECRET") {
                child.env_remove(name);
            }
        }
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
                "if command -v timeout >/dev/null 2>&1; then exec timeout -k 5 {seconds} sh -c '{SHELL}'; else {SHELL}; fi"
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
    use super::*;

    #[tokio::test]
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
    async fn a_local_command_past_its_deadline_is_stopped() {
        let env = Local {
            dir: std::env::temp_dir(),
        };
        let result = env.run("sleep 5", Duration::from_millis(200)).await;
        assert!(result.timed_out);
        assert!(result.seconds < 4.0);
    }

    #[tokio::test]
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
