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

use crate::{Captured, Ended, Ending, GRACE, Limits, Sink, group};

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
#[derive(Clone, Debug)]
pub struct Job {
    program: OsString,
    args: Vec<OsString>,
    workdir: Option<PathBuf>,
    limits: Limits,
}

impl Job {
    /// A job that runs `program`, bounded at [`crate::STREAM_MAX`] bytes a
    /// stream and fifteen seconds of wall time until a caller says
    /// otherwise.
    #[must_use]
    pub fn new(program: impl Into<OsString>) -> Self {
        Job {
            program: program.into(),
            args: Vec::new(),
            workdir: None,
            limits: Limits::within(Duration::from_secs(15)),
        }
    }

    /// Adds one argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds several arguments, in order.
    #[must_use]
    pub fn args<I, A>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Runs the job somewhere other than this process's working directory.
    #[must_use]
    pub fn in_directory(mut self, workdir: impl Into<PathBuf>) -> Self {
        self.workdir = Some(workdir.into());
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
        let started = Instant::now();
        // The supervisor runs in a task of its own so that cleanup survives
        // a caller that walks away. `held` is the only live end of the
        // channel: dropping this future closes it, which is the same signal
        // to the supervisor as an expired deadline.
        let (held, dropped) = oneshot::channel::<()>();
        let supervisor = tokio::spawn(supervise(self, dropped));
        let ended = match supervisor.await {
            Ok(ended) => ended,
            Err(error) => Ended {
                ending: Ending::Failed(format!("the supervisor stopped: {error}")),
                stdout: Captured::default(),
                stderr: Captured::default(),
                elapsed: started.elapsed(),
            },
        };
        drop(held);
        ended
    }
}

/// Spawns the job, drains it, ends it, and reaps it.
async fn supervise(job: Job, dropped: oneshot::Receiver<()>) -> Ended {
    let started = Instant::now();
    let mut command = Command::new(&job.program);
    command
        .args(&job.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // The group is what this supervisor terminates. `kill_on_drop` is
        // kept underneath it as the last resort for the direct child if the
        // runtime itself goes away mid-cleanup.
        .process_group(0)
        .kill_on_drop(true);
    if let Some(workdir) = &job.workdir {
        command.current_dir(workdir);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ended {
                ending: Ending::Failed(error.to_string()),
                stdout: Captured::default(),
                stderr: Captured::default(),
                elapsed: started.elapsed(),
            };
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
    Ended {
        ending,
        stdout,
        stderr,
        elapsed: started.elapsed(),
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
