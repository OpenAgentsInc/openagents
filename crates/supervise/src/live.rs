//! A job the caller watches while it runs.
//!
//! [`Job::run`] hands back one result once cleanup ends. An executor
//! session needs more than that: its host reads the event stream while the
//! session runs, writes messages into its standard input, and stops it
//! when a rule fires. [`Job::start`] gives the caller a [`Live`] handle for
//! that, under the same ownership contract:
//!
//! - The direct child is the leader of a process group of its own.
//! - The deadline, [`Live::stop`], and dropping the handle all end the
//!   group the same way: `SIGTERM`, up to [`GRACE`], then `SIGKILL`, and
//!   the direct child is reaped.
//! - A child that exits on its own takes whatever is left in its group
//!   with it.
//!
//! Standard output is delivered in order as it arrives. The bytes the
//! caller hasn't taken yet are held to [`Limits::stream_max`]; past that,
//! arriving bytes are counted and dropped, and the next [`Live::take`]
//! reports the dropped span as a [`Gap`] at its stream offset. Draining
//! never stops, so a slow reader can't stall the writer past its deadline.
//! Standard error is captured under the same cap as a [`Job::run`].

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::{Captured, Ending, GRACE, Job, Sink, group};

/// The bytes one read takes from a pipe.
const CHUNK: usize = 8 * 1024;

/// Whether the job gets a standard input the caller writes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Standard input is null, as in [`Job::run`].
    Null,
    /// Standard input is a pipe the caller writes with [`Live::send`].
    Piped,
}

/// A span of standard output the caller never received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// Where the span starts in the stream, in bytes from its start.
    pub offset: u64,
    pub bytes: u64,
}

/// What standard output delivered since the last take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delivery {
    /// The stream offset of the first byte of `bytes`.
    pub offset: u64,
    pub bytes: Vec<u8>,
    /// Spans dropped before `bytes`, in stream order.
    pub gaps: Vec<Gap>,
}

impl Delivery {
    /// Whether nothing arrived.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty() && self.gaps.is_empty()
    }
}

/// How a watched job ended.
#[derive(Clone, Debug)]
pub struct Stopped {
    pub ending: Ending,
    /// Whether the caller asked for the stop, rather than the job exiting
    /// or its deadline passing.
    pub requested: bool,
    /// Whether the direct child exited within [`GRACE`] of `SIGTERM`. Only
    /// meaningful when the supervisor had to end the job.
    pub graceful: bool,
    /// Whether the job's process group was empty once cleanup ended. This
    /// is the cleanup acknowledgement a host records.
    pub group_clear: bool,
    /// The process group the job ran in.
    pub group: Option<i32>,
    /// Standard output not yet taken when the job ended.
    pub rest: Delivery,
    /// Every byte standard output produced, taken or dropped.
    pub stdout_bytes: u64,
    pub stderr: Captured,
    pub elapsed: Duration,
}

/// Standard output waiting for the caller.
#[derive(Debug, Default)]
struct Pending {
    /// The stream offset of `held[0]`.
    offset: u64,
    held: Vec<u8>,
    gaps: Vec<Gap>,
    /// Every byte the stream produced.
    total: u64,
    max: usize,
}

impl Pending {
    fn push(&mut self, chunk: &[u8]) {
        let at = self.total;
        self.total += chunk.len() as u64;
        let room = self.max.saturating_sub(self.held.len());
        let kept = room.min(chunk.len());
        self.held.extend_from_slice(&chunk[..kept]);
        let dropped = (chunk.len() - kept) as u64;
        if dropped == 0 {
            return;
        }
        let from = at + kept as u64;
        match self.gaps.last_mut() {
            Some(gap) if gap.offset + gap.bytes == from => gap.bytes += dropped,
            _ => self.gaps.push(Gap {
                offset: from,
                bytes: dropped,
            }),
        }
    }

    fn take(&mut self) -> Delivery {
        let bytes = std::mem::take(&mut self.held);
        let gaps = std::mem::take(&mut self.gaps);
        // Bytes kept before a gap and bytes after it can't share one
        // delivery with one offset, so a gap always ends a delivery: the
        // held bytes precede every gap, because nothing is held once the
        // cap is reached until the caller takes.
        let delivery = Delivery {
            offset: self.offset,
            bytes,
            gaps,
        };
        self.offset = self.total;
        delivery
    }
}

/// A running job the caller watches, writes to, and stops.
///
/// Dropping the handle ends the job: the supervisor task that owns the
/// child sees its channel close and terminates the group, exactly as a
/// deadline does.
#[derive(Debug)]
pub struct Live {
    pid: Option<u32>,
    group: Option<i32>,
    stdout: Arc<Mutex<Pending>>,
    stdin: Option<ChildStdin>,
    stop: Option<oneshot::Sender<()>>,
    supervisor: JoinHandle<(Ending, Captured, bool)>,
    started: Instant,
}

impl Job {
    /// Starts the job and returns a handle the caller watches it through.
    /// Must be called inside a Tokio runtime.
    ///
    /// # Errors
    ///
    /// Returns a message when the program can't be spawned.
    pub fn start(self, input: Input) -> Result<Live, String> {
        let started = Instant::now();
        let limits = self.limits;
        let mut command = Command::from(self.command);
        command
            .stdin(match input {
                Input::Null => Stdio::null(),
                Input::Piped => Stdio::piped(),
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        let pid = child.id();
        let group = pid.and_then(|id| i32::try_from(id).ok());
        let stdin = child.stdin.take();
        let stdout = Arc::new(Mutex::new(Pending {
            max: limits.stream_max,
            ..Pending::default()
        }));
        let out_reader = stream(child.stdout.take(), Arc::clone(&stdout));
        let err_sink = Arc::new(Mutex::new(Sink::new(limits.stream_max)));
        let err_reader = drain(child.stderr.take(), Arc::clone(&err_sink));
        let (stop, stopped) = oneshot::channel::<()>();
        let supervisor = tokio::spawn(async move {
            let mut requested = false;
            let ending = tokio::select! {
                waited = child.wait() => match waited {
                    Ok(status) => Ending::Exited(status.code()),
                    Err(error) => Ending::Failed(error.to_string()),
                },
                () = tokio::time::sleep(limits.wall) => Ending::TimedOut,
                asked = stopped => {
                    // A sent stop is the caller's request; a closed channel
                    // is a dropped handle, which ends the job the same way.
                    requested = asked.is_ok();
                    Ending::TimedOut
                }
            };
            let mut graceful = true;
            let mut stopped_with = None;
            match &ending {
                Ending::Exited(_) => {
                    if let Some(group) = group {
                        group::end(group);
                    }
                }
                _ => {
                    if let Some(group) = group {
                        group::ask(group);
                    }
                    match timeout(GRACE, child.wait()).await {
                        Ok(Ok(status)) => stopped_with = Some(status.code()),
                        _ => graceful = false,
                    }
                    if let Some(group) = group {
                        group::end(group);
                    }
                    if !graceful {
                        stopped_with = child.wait().await.ok().map(|status| status.code());
                    }
                }
            }
            for reader in [out_reader, err_reader] {
                let mut reader = reader;
                if timeout(GRACE, &mut reader).await.is_err() {
                    reader.abort();
                }
            }
            let stderr = err_sink
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .captured();
            // A stop the caller asked for ends the job as the signal left
            // it; only the deadline is a timeout.
            let ending = match (requested, ending) {
                (true, Ending::TimedOut) => Ending::Exited(stopped_with.flatten()),
                (_, ending) => ending,
            };
            (ending, stderr, graceful)
        });
        Ok(Live {
            pid,
            group,
            stdout,
            stdin,
            stop: Some(stop),
            supervisor,
            started,
        })
    }
}

impl Live {
    /// The direct child's process identifier.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Standard output since the last take, and any span dropped since.
    #[must_use]
    pub fn take(&self) -> Delivery {
        self.stdout
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }

    /// Writes `bytes` to the job's standard input and flushes it.
    ///
    /// # Errors
    ///
    /// Returns a message when the job has no piped input, the input is
    /// closed, or the write fails because the job stopped reading.
    pub async fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| "the job's standard input is closed".to_string())?;
        stdin
            .write_all(bytes)
            .await
            .map_err(|error| error.to_string())?;
        stdin.flush().await.map_err(|error| error.to_string())
    }

    /// Closes standard input, which a program that reads to the end takes
    /// as the end of its work.
    pub fn close_input(&mut self) {
        self.stdin = None;
    }

    /// Whether standard input is still open.
    #[must_use]
    pub fn input_open(&self) -> bool {
        self.stdin.is_some()
    }

    /// Whether the job has ended and its cleanup finished.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.supervisor.is_finished()
    }

    /// Asks the supervisor to end the job now, and waits for its cleanup.
    pub async fn stop(mut self) -> Stopped {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.end(true).await
    }

    /// Waits for the job to end on its own or at its deadline.
    pub async fn wait(mut self) -> Stopped {
        self.end(false).await
    }

    async fn end(&mut self, requested: bool) -> Stopped {
        self.stdin = None;
        let (ending, stderr, graceful) = match (&mut self.supervisor).await {
            Ok(result) => result,
            Err(error) => (
                Ending::Failed(format!("the supervisor stopped: {error}")),
                Captured::default(),
                false,
            ),
        };
        // A killed descendant stays in the group until its parent, or
        // init for an orphan, reaps it, so emptiness is waited for, within
        // the grace period, rather than read once.
        let mut group_clear = self.group.is_none_or(|group| !group::running(group));
        let settle = Instant::now();
        while !group_clear && settle.elapsed() < GRACE {
            tokio::time::sleep(Duration::from_millis(5)).await;
            group_clear = self.group.is_none_or(|group| !group::running(group));
        }
        let mut pending = self
            .stdout
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let stdout_bytes = pending.total;
        let rest = pending.take();
        drop(pending);
        // The supervisor has ended; dropping the sender now changes nothing.
        self.stop = None;
        Stopped {
            ending,
            requested,
            graceful,
            group_clear,
            group: self.group,
            rest,
            stdout_bytes,
            stderr,
            elapsed: self.started.elapsed(),
        }
    }
}

/// Reads standard output into the pending buffer as it arrives.
fn stream<R>(reader: Option<R>, pending: Arc<Mutex<Pending>>) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return;
        };
        let mut chunk = [0u8; CHUNK];
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => pending
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push(&chunk[..read]),
            }
        }
    })
}

/// Reads a stream into a capped sink.
fn drain<R>(reader: Option<R>, sink: Arc<Mutex<Sink>>) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return;
        };
        let mut chunk = [0u8; CHUNK];
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => sink
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .push(&chunk[..read]),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_output_past_its_cap_becomes_one_gap_at_its_offset() {
        let mut pending = Pending {
            max: 4,
            ..Pending::default()
        };
        pending.push(b"abc");
        pending.push(b"defgh");
        pending.push(b"ij");
        let first = pending.take();
        assert_eq!(first.offset, 0);
        assert_eq!(first.bytes, b"abcd");
        assert_eq!(
            first.gaps,
            vec![Gap {
                offset: 4,
                bytes: 6
            }]
        );
        pending.push(b"kl");
        let second = pending.take();
        assert_eq!(second.offset, 10);
        assert_eq!(second.bytes, b"kl");
        assert!(second.gaps.is_empty());
    }
}
