//! Streaming and detached command execution.
//!
//! Commands are never retried: a failed request can leave the command running.
//! See [`crate::Error::may_be_running`].

use std::collections::VecDeque;

use reqwest::Method;

use crate::{
    Client, Error, Result, WaitOptions,
    client::{Op, decode_json, limited},
    models::*,
};

/// The default cap on one NDJSON line of a command stream.
pub const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;

/// One frame of a streamed command. `Debug` omits output and error text.
#[derive(Clone)]
pub enum CommandFrame {
    /// The command started on the sandbox.
    Started,
    /// A UTF-8 chunk the command wrote to stdout.
    Stdout(String),
    /// A UTF-8 chunk the command wrote to stderr.
    Stderr(String),
    /// The command finished. This is the synchronous result without output.
    Exit {
        exit_code: Option<i64>,
        success: bool,
        timed_out: bool,
    },
    /// The command could not run or the stream failed. `retryable` is true
    /// only when the command provably never ran.
    Error {
        error: Option<String>,
        message: Option<String>,
        retryable: bool,
    },
    /// A frame type this SDK does not know yet, kept whole.
    Unknown(CommandStreamFrame),
}

impl std::fmt::Debug for CommandFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Started => f.write_str("Started"),
            Self::Stdout(data) => write!(f, "Stdout({} bytes)", data.len()),
            Self::Stderr(data) => write!(f, "Stderr({} bytes)", data.len()),
            Self::Exit {
                exit_code,
                success,
                timed_out,
            } => f
                .debug_struct("Exit")
                .field("exit_code", exit_code)
                .field("success", success)
                .field("timed_out", timed_out)
                .finish(),
            Self::Error { retryable, .. } => f
                .debug_struct("Error")
                .field("retryable", retryable)
                .finish_non_exhaustive(),
            Self::Unknown(_) => f.write_str("Unknown(..)"),
        }
    }
}

impl From<CommandStreamFrame> for CommandFrame {
    fn from(frame: CommandStreamFrame) -> Self {
        match frame.type_.as_str() {
            "started" => Self::Started,
            "stdout" => Self::Stdout(frame.data.unwrap_or_default()),
            "stderr" => Self::Stderr(frame.data.unwrap_or_default()),
            "exit" => Self::Exit {
                exit_code: frame.exit_code.as_ref().copied(),
                success: frame.success.unwrap_or(false),
                timed_out: frame.timed_out.unwrap_or(false),
            },
            "error" => Self::Error {
                error: frame.error,
                message: frame.message,
                retryable: frame.retryable.unwrap_or(false),
            },
            _ => Self::Unknown(frame),
        }
    }
}

/// A command's output as it arrives. Call [`CommandStream::next`] until it
/// returns `None`; the last frame is `Exit` or `Error`.
pub struct CommandStream {
    source: Source,
    buffer: Vec<u8>,
    pending: VecDeque<CommandFrame>,
    max_line_bytes: usize,
    finished: bool,
}

enum Source {
    Live(reqwest::Response),
    Done,
}

impl std::fmt::Debug for CommandStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandStream").finish_non_exhaustive()
    }
}

impl CommandStream {
    pub async fn next(&mut self) -> Result<Option<CommandFrame>> {
        loop {
            if let Some(frame) = self.pending.pop_front() {
                return Ok(Some(frame));
            }
            if self.finished {
                return Ok(None);
            }
            let Source::Live(response) = &mut self.source else {
                self.finished = true;
                continue;
            };
            match response.chunk().await.map_err(|_| Error::Transport)? {
                Some(chunk) => {
                    self.buffer.extend_from_slice(&chunk);
                    self.drain_lines(false)?;
                }
                None => {
                    self.drain_lines(true)?;
                    self.source = Source::Done;
                    self.finished = true;
                }
            }
        }
    }

    /// Collect every frame, returning stdout, stderr and the final frame.
    pub async fn collect(mut self) -> Result<CommandOutput> {
        let mut output = CommandOutput::default();
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

    fn drain_lines(&mut self, end: bool) -> Result<()> {
        while let Some(at) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=at).collect();
            self.push_line(&line[..line.len() - 1])?;
        }
        if self.buffer.len() > self.max_line_bytes {
            return Err(Error::StreamLineTooLong);
        }
        if end && !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            self.push_line(&line)?;
        }
        Ok(())
    }

    fn push_line(&mut self, line: &[u8]) -> Result<()> {
        if line.len() > self.max_line_bytes {
            return Err(Error::StreamLineTooLong);
        }
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.iter().all(u8::is_ascii_whitespace) {
            return Ok(());
        }
        let frame: CommandStreamFrame = serde_json::from_slice(line).map_err(|_| Error::Decode)?;
        self.pending.push_back(frame.into());
        Ok(())
    }

    fn from_frames(frames: Vec<CommandFrame>) -> Self {
        Self {
            source: Source::Done,
            buffer: Vec::new(),
            pending: frames.into(),
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            finished: true,
        }
    }
}

/// Everything a finished stream produced. `Debug` omits the output.
#[derive(Clone, Default)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    /// The `Exit` or `Error` frame, or `None` if the stream ended without one.
    pub last: Option<CommandFrame>,
}

impl CommandOutput {
    /// The exit code when the command finished.
    pub fn exit_code(&self) -> Option<i64> {
        match self.last {
            Some(CommandFrame::Exit { exit_code, .. }) => exit_code,
            _ => None,
        }
    }
}

impl std::fmt::Debug for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandOutput")
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("last", &self.last)
            .finish()
    }
}

impl Client {
    /// Run a command with `stream: true` and read its NDJSON frames as they
    /// arrive. The request is sent once and never retried.
    pub async fn exec_stream(
        &self,
        sandbox_id: &str,
        request: CommandRequest,
    ) -> Result<CommandStream> {
        self.exec_stream_with_limit(sandbox_id, request, DEFAULT_MAX_LINE_BYTES)
            .await
    }

    /// [`Client::exec_stream`] with a custom cap on one NDJSON line.
    pub async fn exec_stream_with_limit(
        &self,
        sandbox_id: &str,
        mut request: CommandRequest,
        max_line_bytes: usize,
    ) -> Result<CommandStream> {
        if max_line_bytes == 0 {
            return Err(Error::Configuration(
                "The stream line limit must be greater than zero.",
            ));
        }
        request.stream = Some(true);
        request.detached = None;
        let path = self.path(&["sandboxes", sandbox_id, "commands"])?;
        let body = serde_json::to_value(&request).map_err(Error::encode)?;
        let headers = [("Accept", "application/x-ndjson".to_string())];
        let response = self
            .send(Op::NEVER, Method::POST, path, &[], &headers, Some(body))
            .await?;
        let ndjson = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/x-ndjson"));
        if ndjson {
            return Ok(CommandStream {
                source: Source::Live(response),
                buffer: Vec::new(),
                pending: VecDeque::new(),
                max_line_bytes,
                finished: false,
            });
        }
        // A server that ignored `stream` answered with the synchronous result.
        let status = response.status();
        let bytes = limited(response, self.max_json_bytes()).await?;
        match decode_json::<CommandResponseBody>(status, &bytes)? {
            CommandResponseBody::Finished(done) => {
                let mut frames = vec![CommandFrame::Started];
                if !done.stdout.is_empty() {
                    frames.push(CommandFrame::Stdout(done.stdout));
                }
                if !done.stderr.is_empty() {
                    frames.push(CommandFrame::Stderr(done.stderr));
                }
                frames.push(CommandFrame::Exit {
                    exit_code: done.exit_code,
                    success: done.success,
                    timed_out: done.timed_out,
                });
                Ok(CommandStream::from_frames(frames))
            }
            CommandResponseBody::Started(_) => Err(Error::Decode),
        }
    }

    /// Start a command in the background and return its process record.
    /// Poll it with [`Client::wait_command`]. Never retried.
    pub async fn exec_detached(
        &self,
        sandbox_id: &str,
        mut request: CommandRequest,
    ) -> Result<CommandStartedResponse> {
        request.detached = Some(true);
        request.stream = None;
        match self
            .command(&CommandParams {
                sandbox_id: sandbox_id.into(),
                body: request,
                ..Default::default()
            })
            .await?
        {
            CommandResponseBody::Started(started) => Ok(started),
            CommandResponseBody::Finished(_) => Err(Error::Decode),
        }
    }

    /// Poll a detached command until it stops running.
    pub async fn wait_command(
        &self,
        sandbox_id: &str,
        process_id: i64,
        options: &WaitOptions,
    ) -> Result<CommandStatusResponse> {
        self.wait_for_command(
            &CommandStatusParams {
                sandbox_id: sandbox_id.into(),
                process_id,
                ..Default::default()
            },
            options,
        )
        .await
    }
}
