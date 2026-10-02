//! Following, stopping and streaming commands beyond what one request does.
//!
//! Boat has no endpoint for a detached command's output from an offset, for
//! signalling a process, or for stdin. These helpers build the first two from
//! the public operations:
//!
//! - [`Client::follow_command`] reads a detached command's log files from a
//!   byte [`OutputCursor`] with a short read-only command, so output arrives
//!   once, in order, and a follower that was dropped (a lost connection, a
//!   restarted caller) picks up from the cursor it persisted.
//! - [`Client::kill_command`] signals a detached command and every process it
//!   started.
//! - [`Client::run_streaming`] streams a synchronous command into an async
//!   sink, reading the next chunk only after the sink has taken the last one.
//!
//! For stdin, write the input with [`Client::write_bytes`] and redirect it
//! (`cmd < /tmp/input`). None of these resend the caller's command.

use std::{collections::VecDeque, future::Future, time::Duration};

use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::{Client, CommandFrame, Error, Result, WaitOptions, models::*};

/// The most log bytes one follower read takes from each stream. Two of them,
/// base64-encoded, stay well under Boat's synchronous output cap.
pub const FOLLOW_CHUNK_BYTES: u64 = 128 * 1024;
/// Consecutive transient failures (transport, 429, 5xx, a sandbox still
/// starting) a follower rides out before it returns the error.
pub const FOLLOW_MAX_FAILURES: u32 = 5;

/// Bytes of a detached command's stdout and stderr logs already delivered.
/// Persist it after handling each frame to resume with
/// [`Client::follow_command_from`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputCursor {
    pub stdout: u64,
    pub stderr: u64,
}

/// The signals [`Client::kill_command`] sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Term,
    Int,
    Hup,
    Kill,
}

impl Signal {
    fn name(self) -> &'static str {
        match self {
            Self::Term => "TERM",
            Self::Int => "INT",
            Self::Hup => "HUP",
            Self::Kill => "KILL",
        }
    }
}

/// Quote `text` for a POSIX shell. A leading `~/` stays expandable.
pub fn shell_quote(text: &str) -> String {
    if let Some(rest) = text.strip_prefix("~/") {
        return format!("\"$HOME\"/{}", shell_quote(rest));
    }
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn is_transient(error: &Error) -> bool {
    match error {
        Error::Transport => true,
        Error::Api(api) => {
            api.is_transient() || matches!(api.code(), Some("boat_starting" | "box_starting"))
        }
        _ => false,
    }
}

/// The valid UTF-8 prefix of `bytes`, leaving a split character at the end
/// for the next read. Invalid bytes elsewhere are replaced.
fn utf8_prefix(bytes: &[u8], end: bool) -> (String, usize) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), bytes.len()),
        Err(error) if !end && error.error_len().is_none() => {
            let valid = error.valid_up_to();
            (String::from_utf8_lossy(&bytes[..valid]).into_owned(), valid)
        }
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), bytes.len()),
    }
}

/// A detached command's output and end, read through a byte cursor.
/// Call [`CommandFollower::next`] until it returns `None`; the last frame is
/// `Exit`, or `Error` with `error: "lost"` when the sandbox agent forgot the
/// process.
pub struct CommandFollower {
    client: Client,
    sandbox_id: String,
    process_id: i64,
    logs: Option<(String, String)>,
    cursor: OutputCursor,
    options: WaitOptions,
    deadline: Instant,
    chunk_bytes: u64,
    pending: VecDeque<CommandFrame>,
    failures: u32,
    finished: bool,
}

impl std::fmt::Debug for CommandFollower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandFollower")
            .field("process_id", &self.process_id)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

struct Reads {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl CommandFollower {
    /// Where the delivered output ends. It covers every frame already
    /// returned by `next`, and nothing after.
    pub fn cursor(&self) -> OutputCursor {
        self.cursor
    }

    /// The next frame. Output frames come in log order; the end comes last.
    pub async fn next(&mut self) -> Result<Option<CommandFrame>> {
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(Some(frame));
            }
            if self.finished {
                return Ok(None);
            }
            let cancellation = self.options.cancellation.clone();
            let deadline = self.deadline;
            let step = cancellation.run(deadline, self.step()).await;
            match step {
                Ok(true) => {}
                Ok(false) => {
                    let interval = self.options.interval;
                    cancellation
                        .run(deadline, async {
                            tokio::time::sleep(interval).await;
                            Ok(())
                        })
                        .await?;
                }
                Err(error) if is_transient(&error) && self.failures < FOLLOW_MAX_FAILURES => {
                    self.failures += 1;
                    let interval = self.options.interval;
                    cancellation
                        .run(deadline, async {
                            tokio::time::sleep(interval).await;
                            Ok(())
                        })
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Drain the follower into stdout, stderr and the final frame.
    pub async fn collect(mut self) -> Result<crate::CommandOutput> {
        let mut output = crate::CommandOutput::default();
        while let Some(frame) = self.next().await? {
            match frame {
                CommandFrame::Stdout(data) => output.stdout.push_str(&data),
                CommandFrame::Stderr(data) => output.stderr.push_str(&data),
                CommandFrame::Started | CommandFrame::Unknown(_) => {}
                last => output.last = Some(last),
            }
        }
        Ok(output)
    }

    /// One poll. `Ok(true)` when it produced frames or more output is ready.
    async fn step(&mut self) -> Result<bool> {
        // Status first: output read after an exit is then complete.
        let status = self
            .client
            .command_status(&CommandStatusParams {
                sandbox_id: self.sandbox_id.clone(),
                process_id: self.process_id,
                tail_bytes: Some(1),
                ..Default::default()
            })
            .await?;
        if self.logs.is_none()
            && let (Some(out), Some(err)) = (&status.log_path, &status.err_log_path)
        {
            self.logs = Some((out.clone(), err.clone()));
        }
        let ended = !status.running;
        let mut full = false;
        if self.logs.is_some() {
            let reads = self.read_logs().await?;
            full = reads.stdout.len() as u64 >= self.chunk_bytes
                || reads.stderr.len() as u64 >= self.chunk_bytes;
            let last = ended && !full;
            let (text, used) = utf8_prefix(&reads.stdout, last);
            self.cursor.stdout += used as u64;
            if !text.is_empty() {
                self.pending.push_back(CommandFrame::Stdout(text));
            }
            let (text, used) = utf8_prefix(&reads.stderr, last);
            self.cursor.stderr += used as u64;
            if !text.is_empty() {
                self.pending.push_back(CommandFrame::Stderr(text));
            }
        } else if ended {
            // No log paths: the status tails are all there is.
            let status = self
                .client
                .command_status(&CommandStatusParams {
                    sandbox_id: self.sandbox_id.clone(),
                    process_id: self.process_id,
                    ..Default::default()
                })
                .await?;
            for (text, seen, stdout) in [
                (status.stdout, self.cursor.stdout, true),
                (status.stderr, self.cursor.stderr, false),
            ] {
                let rest = text
                    .get(usize::try_from(seen).unwrap_or(usize::MAX)..)
                    .unwrap_or_default()
                    .to_owned();
                if !rest.is_empty() {
                    self.pending.push_back(if stdout {
                        CommandFrame::Stdout(rest)
                    } else {
                        CommandFrame::Stderr(rest)
                    });
                }
            }
        }
        self.failures = 0;
        if ended && !full {
            self.pending.push_back(if status.status == "lost" {
                CommandFrame::Error {
                    error: Some("lost".into()),
                    message: None,
                    retryable: false,
                }
            } else {
                CommandFrame::Exit {
                    exit_code: status.exit_code,
                    success: status.exit_code == Some(0),
                    timed_out: false,
                }
            });
            self.finished = true;
        }
        Ok(!self.pending.is_empty() || full)
    }

    /// Read up to `chunk_bytes` of each log from the cursor, base64 over a
    /// short synchronous command so any byte survives the JSON response.
    async fn read_logs(&self) -> Result<Reads> {
        let Some((out, err)) = &self.logs else {
            return Ok(Reads {
                stdout: Vec::new(),
                stderr: Vec::new(),
            });
        };
        let script = format!(
            "r() {{ tail -c +\"$2\" \"$1\" 2>/dev/null | head -c {n} | base64 -w0; echo; }}; r {} {}; r {} {}",
            shell_quote(out),
            self.cursor.stdout + 1,
            shell_quote(err),
            self.cursor.stderr + 1,
            n = self.chunk_bytes,
        );
        let answer = self
            .client
            .command(&CommandParams {
                sandbox_id: self.sandbox_id.clone(),
                body: CommandRequest {
                    command: script,
                    timeout_seconds: Some(30),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;
        let CommandResponseBody::Finished(done) = answer else {
            return Err(Error::Decode);
        };
        if done.exit_code != Some(0) {
            return Err(Error::Decode);
        }
        let mut lines = done.stdout.lines();
        let mut decode = || -> Result<Vec<u8>> {
            base64::engine::general_purpose::STANDARD
                .decode(lines.next().unwrap_or_default().trim())
                .map_err(|_| Error::Decode)
        };
        Ok(Reads {
            stdout: decode()?,
            stderr: decode()?,
        })
    }
}

impl Client {
    /// Follow a detached command from the start of its output. The polling
    /// interval, overall timeout and cancellation come from `options`; when
    /// they end the follower, the command keeps running.
    pub fn follow_command(
        &self,
        sandbox_id: &str,
        process_id: i64,
        options: WaitOptions,
    ) -> Result<CommandFollower> {
        self.follow_command_from(sandbox_id, process_id, OutputCursor::default(), options)
    }

    /// Follow a detached command from a cursor a previous follower returned.
    pub fn follow_command_from(
        &self,
        sandbox_id: &str,
        process_id: i64,
        cursor: OutputCursor,
        options: WaitOptions,
    ) -> Result<CommandFollower> {
        if options.timeout.is_zero() || options.interval.is_zero() {
            return Err(Error::Configuration(
                "Polling limits must be greater than zero.",
            ));
        }
        let deadline = Instant::now()
            .checked_add(options.timeout)
            .ok_or(Error::Configuration("The polling deadline is too large."))?;
        Ok(CommandFollower {
            client: self.clone(),
            sandbox_id: sandbox_id.into(),
            process_id,
            logs: None,
            cursor,
            options,
            deadline,
            chunk_bytes: FOLLOW_CHUNK_BYTES,
            pending: VecDeque::new(),
            failures: 0,
            finished: false,
        })
    }

    /// Send `signal` to a detached command's process and every process it
    /// started (children first collected, then all signalled at once).
    /// Returns `false` when the process had already gone. The kill runs as
    /// one short synchronous command and is never retried.
    pub async fn kill_command(&self, sandbox_id: &str, pid: i64, signal: Signal) -> Result<bool> {
        if pid <= 1 {
            return Err(Error::Configuration("The process id must be above 1."));
        }
        let script = format!(
            "t() {{ echo \"$1\"; for c in $(pgrep -P \"$1\"); do t \"$c\"; done; }}; \
             if kill -0 {pid} 2>/dev/null; then kill -s {sig} $(t {pid}) 2>/dev/null; echo killed; else echo gone; fi",
            sig = signal.name(),
        );
        let answer = self
            .command(&CommandParams {
                sandbox_id: sandbox_id.into(),
                body: CommandRequest {
                    command: script,
                    timeout_seconds: Some(30),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;
        match answer {
            CommandResponseBody::Finished(done) => Ok(done.stdout.trim_end().ends_with("killed")),
            CommandResponseBody::Started(_) => Err(Error::Decode),
        }
    }

    /// Run a command with `stream: true` and hand each frame to `sink`,
    /// reading the next chunk only once `sink` has finished with the last,
    /// so a slow consumer slows the stream instead of buffering it. Returns
    /// the `Exit` or `Error` frame. `options` bounds the whole run; when it
    /// ends early the command may still be running. Never retried.
    pub async fn run_streaming<F, Fut>(
        &self,
        sandbox_id: &str,
        request: CommandRequest,
        options: &WaitOptions,
        mut sink: F,
    ) -> Result<CommandFrame>
    where
        F: FnMut(CommandFrame) -> Fut,
        Fut: Future<Output = ()>,
    {
        let deadline = Instant::now()
            .checked_add(options.timeout)
            .ok_or(Error::Configuration("The run deadline is too large."))?;
        options
            .cancellation
            .run(deadline, async {
                let mut stream = self.exec_stream(sandbox_id, request).await?;
                while let Some(frame) = stream.next().await? {
                    match frame {
                        CommandFrame::Exit { .. } | CommandFrame::Error { .. } => {
                            return Ok(frame);
                        }
                        other => sink(other).await,
                    }
                }
                // The stream closed without its last frame: the command may
                // still be running.
                Err(Error::Transport)
            })
            .await
    }
}

/// How long [`Client::follow_command`] waits by default: the four-hour
/// sandbox lifetime `openagents boat run` asks for.
pub const DEFAULT_FOLLOW_TIMEOUT: Duration = Duration::from_secs(4 * 3600);
