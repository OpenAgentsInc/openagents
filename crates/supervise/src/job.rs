//! The asynchronous supervisor: one job, spawned, drained, and ended.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::memory::{self, Placed};
use crate::{Captured, Ended, Ending, GRACE, Limits, Memory, Sink, group};

/// The bytes one read takes from a pipe. Two of these are the only capture
/// memory a job holds beyond its caps.
const CHUNK: usize = 8 * 1024;

/// One program to run under one set of limits.
///
/// ```no_run
/// # async fn example() {
/// use std::time::Duration;
/// use supervise::{Ending, Job, Limits};
///
/// let ended = Job::new("sh")
///     .arg("-c")
///     .arg("echo hello")
///     .bounded(Limits::within(Duration::from_secs(5)))
///     .run()
///     .await;
/// assert_eq!(ended.ending, Ending::Exited(Some(0)));
/// assert_eq!(ended.stdout.text.trim(), "hello");
/// # }
/// ```
#[derive(Debug)]
pub struct Job {
    pub(crate) command: std::process::Command,
    pub(crate) limits: Limits,
}

impl Job {
    /// A job that runs `program`, bounded at [`crate::STREAM_MAX`] bytes a
    /// stream and fifteen seconds of wall time until a caller says
    /// otherwise.
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Self::from_command(std::process::Command::new(program.into()))
    }

    /// Supervises a prepared command, preserving its arguments, working
    /// directory, and environment policy, including `env_clear`.
    ///
    /// The supervisor replaces standard input with null, captures both
    /// output streams, and creates its own process group. A filesystem
    /// boundary can prepare its wrapper before handing ownership here.
    #[must_use]
    pub fn from_command(command: std::process::Command) -> Self {
        Job {
            command,
            limits: Limits::within(Duration::from_secs(15)),
        }
    }

    /// Adds one argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.command.arg(arg.into());
        self
    }

    /// Adds several arguments, in order.
    #[must_use]
    pub fn args<I, A>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<OsString>,
    {
        self.command.args(args.into_iter().map(Into::into));
        self
    }

    /// Runs the job somewhere other than this process's working directory.
    #[must_use]
    pub fn in_directory(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.command.current_dir(workdir.into());
        self
    }

    /// The time and output this job may take.
    #[must_use]
    pub fn bounded(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Runs the job and waits for it, cleanup included.
    ///
    /// The future this returns owns the job. Awaiting it to the end gives
    /// the result; dropping it first cancels the job, and the supervisor
    /// still terminates the tree and reaps the direct child — the caller
    /// just never learns what it said.
    pub async fn run(self) -> Ended {
        self.run_holding(()).await
    }

    /// Keeps a resource alive until the child is reaped and output cleanup
    /// ends, even if the caller cancels the returned future. A worktree
    /// guard uses this to outlive every process that can write into it.
    pub async fn run_holding<T: Send + 'static>(self, resource: T) -> Ended {
        let started = Instant::now();
        // The supervisor runs in a task of its own so that cleanup survives
        // a caller that walks away. `held` is the only live end of the
        // channel: dropping this future closes it, which is the same signal
        // to the supervisor as an expired deadline.
        let (held, dropped) = oneshot::channel::<()>();
        let supervisor = tokio::spawn(async move {
            let ended = supervise(self, dropped).await;
            drop(resource);
            ended
        });
        let ended = match supervisor.await {
            Ok(ended) => ended,
            Err(error) => Ended {
                ending: Ending::Failed(format!("the supervisor stopped: {error}")),
                stdout: Captured::default(),
                stderr: Captured::default(),
                elapsed: started.elapsed(),
                memory: None,
            },
        };
        drop(held);
        ended
    }
}

/// Spawns the job, drains it, ends it, and reaps it.
async fn supervise(job: Job, dropped: oneshot::Receiver<()>) -> Ended {
    let started = Instant::now();
    let mut prepared = job.command;
    let handshake = match job.limits.memory_max {
        Some(max) => match memory::arm(&mut prepared, max) {
            Ok(handshake) => Some(handshake),
            Err(why) => return unspawned(why, started),
        },
        None => None,
    };
    let mut command = Command::from(prepared);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The group is what this supervisor terminates. `kill_on_drop` is
        // kept underneath it as the last resort for the direct child if the
        // runtime itself goes away mid-cleanup.
        .process_group(0)
        .kill_on_drop(true);
    // With a cap, the child waits between `fork` and `exec` until the
    // helper has placed it, so the spawn returns once it is placed.
    let serving = handshake.map(memory::Handshake::serve);
    let spawned = command.spawn();
    let placed = serving.map(memory::Serving::finish);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            // A child that failed to execute may still have been placed,
            // and its scope is cleared away like any other.
            settle(placed).await;
            return unspawned(error.to_string(), started);
        }
    };
    // `process_group(0)` makes the child the leader of a new group, so the
    // group's identifier is the child's own.
    let group = child.id().and_then(|id| i32::try_from(id).ok());
    let out = drain(child.stdout.take(), job.limits.stream_max);
    let err = drain(child.stderr.take(), job.limits.stream_max);

    let ending = tokio::select! {
        waited = child.wait() => match waited {
            Ok(status) => Ending::Exited(status.code()),
            Err(error) => Ending::Failed(error.to_string()),
        },
        () = tokio::time::sleep(job.limits.wall) => Ending::TimedOut,
        _ = dropped => Ending::TimedOut,
    };

    match &ending {
        // The child is already reaped, so anything left in the group is a
        // descendant that outlived the job. There is nothing to negotiate
        // with it about: the job has its result.
        Ending::Exited(_) => {
            if let Some(group) = group {
                group::end(group);
            }
        }
        _ => stop(group, &mut child).await,
    }

    let stdout = finish(out).await;
    let stderr = finish(err).await;
    let memory = settle(placed).await;
    Ended {
        ending,
        stdout,
        stderr,
        elapsed: started.elapsed(),
        memory,
    }
}

/// A job that never started.
fn unspawned(why: String, started: Instant) -> Ended {
    Ended {
        ending: Ending::Failed(why),
        stdout: Captured::default(),
        stderr: Captured::default(),
        elapsed: started.elapsed(),
        memory: None,
    }
}

/// Settles a job's memory cap once its tree is gone, off the runtime's
/// worker threads, since it waits on systemd.
pub(crate) async fn settle(placed: Option<Placed>) -> Option<Memory> {
    let placed = placed?;
    let fallback = placed.clone();
    match tokio::task::spawn_blocking(move || placed.settle()).await {
        Ok(memory) => Some(memory),
        Err(_) => Some(fallback.unsettled()),
    }
}

/// Ends a job that is still running and reaps its direct child.
async fn stop(group: Option<i32>, child: &mut Child) {
    if let Some(group) = group {
        group::ask(group);
    }
    let exited = timeout(GRACE, child.wait()).await.is_ok();
    if let Some(group) = group {
        group::end(group);
    }
    if !exited {
        // The group is killed, so this is the reap rather than a wait.
        let _ = child.wait().await;
    }
}

/// One stream being read into a capped buffer.
struct Drain {
    sink: Arc<Mutex<Sink>>,
    reading: JoinHandle<()>,
}

/// Starts reading a stream, capped at `max` bytes and counted in full.
fn drain<R>(reader: Option<R>, max: usize) -> Drain
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let sink = Arc::new(Mutex::new(Sink::new(max)));
    let held = Arc::clone(&sink);
    let reading = tokio::spawn(async move {
        let Some(mut reader) = reader else {
            return;
        };
        let mut chunk = [0u8; CHUNK];
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                // Reading past the cap is deliberate: the bytes are
                // counted and dropped, and the pipe keeps moving. A pipe
                // nobody drains fills, and a writer the kernel has stopped
                // cannot answer its own deadline.
                Ok(read) => sank(&held, &chunk[..read]),
            }
        }
    });
    Drain { sink, reading }
}

/// Adds a chunk to a sink another task is sharing.
fn sank(sink: &Arc<Mutex<Sink>>, chunk: &[u8]) {
    let mut sink = sink.lock().unwrap_or_else(|poison| poison.into_inner());
    sink.push(chunk);
}

/// Takes what a stream captured, waiting a bounded time for the end of it.
///
/// The wait is bounded because the far end of a pipe can outlive the group:
/// a descendant that left the group keeps the write end open, and a drain
/// that waited on it would hold the job open forever. What was read by then
/// is the answer, which is why the buffer is shared rather than returned.
async fn finish(mut drain: Drain) -> Captured {
    if timeout(GRACE, &mut drain.reading).await.is_err() {
        drain.reading.abort();
    }
    let sink = drain
        .sink
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    sink.captured()
}
