//! Native protocol I/O with optional host process supervision.

use std::io;
use std::process::{ExitStatus, Output};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use super::{Child, Command, Stdio};

pub(crate) type RuntimeInput = Box<dyn AsyncWrite + Send + Sync + Unpin>;
pub(crate) type RuntimeOutput = Box<dyn AsyncRead + Send + Sync + Unpin>;

#[derive(Clone, Copy)]
pub(crate) enum RuntimePipes {
    Interactive,
    Server,
    Probe,
}

pub(crate) struct RuntimeChild {
    native: Option<Child>,
    pub stdin: Option<RuntimeInput>,
    pub stdout: Option<RuntimeOutput>,
    pub stderr: Option<RuntimeOutput>,
    #[cfg(all(unix, feature = "supervised"))]
    supervised: Option<SupervisedChild>,
}

impl RuntimeChild {
    pub(crate) async fn launch(
        mut command: Command,
        #[cfg(unix)] broker: Option<&super::LaunchBroker>,
        pipes: RuntimePipes,
    ) -> io::Result<Self> {
        #[cfg(unix)]
        if let Some(broker) = broker {
            command = broker.prepare(command).await?;
            #[cfg(feature = "supervised")]
            if let Some(supervision) = &broker.supervision {
                return Self::supervised(command, pipes, supervision.clone(), broker.stop.clone());
            }
        }
        command
            .stdin(if matches!(pipes, RuntimePipes::Interactive) {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if matches!(pipes, RuntimePipes::Server) {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        Ok(Self {
            stdin: child.stdin.take().map(|io| Box::new(io) as RuntimeInput),
            stdout: child.stdout.take().map(|io| Box::new(io) as RuntimeOutput),
            stderr: child.stderr.take().map(|io| Box::new(io) as RuntimeOutput),
            native: Some(child),
            #[cfg(all(unix, feature = "supervised"))]
            supervised: None,
        })
    }

    pub(crate) fn id(&self) -> Option<u32> {
        if let Some(child) = &self.native {
            return child.id();
        }
        #[cfg(all(unix, feature = "supervised"))]
        if let Some(child) = &self.supervised {
            return child
                .ended
                .borrow()
                .is_none()
                .then_some(child.pid)
                .flatten();
        }
        None
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(child) = &mut self.native {
            return child.try_wait();
        }
        #[cfg(all(unix, feature = "supervised"))]
        if let Some(child) = &self.supervised {
            return Ok(child
                .ended
                .borrow()
                .as_ref()
                .map(|stopped| exit_status(&stopped.ending)));
        }
        Err(io::Error::other("agent process is unavailable"))
    }

    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
        if let Some(child) = &mut self.native {
            return child.wait().await;
        }
        #[cfg(all(unix, feature = "supervised"))]
        if let Some(child) = &mut self.supervised {
            loop {
                if let Some(stopped) = child.ended.borrow().as_ref() {
                    return Ok(exit_status(&stopped.ending));
                }
                child
                    .ended
                    .changed()
                    .await
                    .map_err(|_| io::Error::other("agent supervisor closed"))?;
            }
        }
        Err(io::Error::other("agent process is unavailable"))
    }

    pub(crate) async fn wait_with_output(mut self) -> io::Result<Output> {
        self.stdin.take();
        let stdout = self.stdout.take();
        let stderr = self.stderr.take();
        async fn read(io: Option<RuntimeOutput>) -> io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            if let Some(mut io) = io {
                let mut chunk = [0; 8192];
                loop {
                    let count = io.read(&mut chunk).await?;
                    if count == 0 {
                        break;
                    }
                    let keep = count.min((64 * 1024usize).saturating_sub(bytes.len()));
                    bytes.extend_from_slice(&chunk[..keep]);
                }
            }
            Ok(bytes)
        }
        let (status, stdout, stderr) = tokio::join!(self.wait(), read(stdout), read(stderr));
        Ok(Output {
            status: status?,
            stdout: stdout?,
            stderr: stderr?,
        })
    }

    #[cfg(unix)]
    pub(crate) fn signal_target(&self) -> Option<i32> {
        if let Some(child) = &self.native {
            return super::signal_target(child);
        }
        self.id().map(|pid| -(pid as i32))
    }

    #[cfg(windows)]
    pub(crate) fn signal_target(&self) -> Option<std::sync::Arc<crate::windows_process::Job>> {
        self.native.as_ref().and_then(super::signal_target)
    }

    pub(crate) async fn shutdown(&mut self, earlier: Vec<i32>, grace: Duration) {
        if let Some(child) = &mut self.native {
            crate::shutdown_agent(child, earlier.clone(), grace).await;
        }
        #[cfg(all(unix, feature = "supervised"))]
        if let Some(child) = &self.supervised {
            child.stop.cancel();
            super::terminate_tree(&earlier, grace).await;
        }
        let _ = self.wait().await;
    }
}

#[cfg(all(unix, feature = "supervised"))]
#[derive(Clone)]
pub struct Supervision {
    pub limits: supervise::Limits,
    /// Runs synchronously as output arrives and after process cleanup finishes.
    /// Observers must keep their own retained data bounded.
    pub observe: std::sync::Arc<dyn Fn(SupervisedEvent) + Send + Sync>,
}

#[cfg(all(unix, feature = "supervised"))]
pub enum SupervisedEvent {
    /// The process started; a matching stop follows after cleanup.
    Started,
    Output(supervise::Delivery),
    Stopped(supervise::Stopped),
}

#[cfg(all(unix, feature = "supervised"))]
struct SupervisedChild {
    pid: Option<u32>,
    stop: tokio_util::sync::CancellationToken,
    ended: tokio::sync::watch::Receiver<Option<std::sync::Arc<supervise::Stopped>>>,
}

#[cfg(all(unix, feature = "supervised"))]
impl Drop for SupervisedChild {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

#[cfg(all(unix, feature = "supervised"))]
fn exit_status(ending: &supervise::Ending) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(match ending {
        supervise::Ending::Exited(Some(code)) => code << 8,
        supervise::Ending::Exited(None) | supervise::Ending::TimedOut => libc::SIGKILL,
        supervise::Ending::Failed(_) => 1 << 8,
    })
}

#[cfg(all(unix, feature = "supervised"))]
impl RuntimeChild {
    fn supervised(
        command: Command,
        pipes: RuntimePipes,
        supervision: Supervision,
        host_stop: tokio_util::sync::CancellationToken,
    ) -> io::Result<Self> {
        let mut live = supervise::Job::from_command(command.into_std())
            .bounded(supervision.limits)
            .start(if matches!(pipes, RuntimePipes::Interactive) {
                supervise::Input::Piped
            } else {
                supervise::Input::Null
            })
            .map_err(io::Error::other)?;
        (supervision.observe)(SupervisedEvent::Started);
        let pid = live.pid();
        let stdin = live.take_input().map(|io| Box::new(io) as RuntimeInput);
        let (stdout, out) = tokio::io::duplex(64 * 1024);
        let (stderr, err) = tokio::io::duplex(64 * 1024);
        let stop = host_stop.child_token();
        let (ended_tx, ended) = tokio::sync::watch::channel(None);
        tokio::spawn(pump(
            live,
            supervision,
            stop.clone(),
            out,
            err,
            ended_tx,
            matches!(pipes, RuntimePipes::Server),
        ));
        Ok(Self {
            native: None,
            stdin,
            stdout: (!matches!(pipes, RuntimePipes::Server))
                .then(|| Box::new(stdout) as RuntimeOutput),
            stderr: Some(Box::new(stderr)),
            supervised: Some(SupervisedChild { pid, stop, ended }),
        })
    }
}

#[cfg(all(unix, feature = "supervised"))]
async fn pump(
    live: supervise::Live,
    supervision: Supervision,
    stop: tokio_util::sync::CancellationToken,
    mut stdout: tokio::io::DuplexStream,
    mut stderr: tokio::io::DuplexStream,
    ended: tokio::sync::watch::Sender<Option<std::sync::Arc<supervise::Stopped>>>,
    discard_stdout: bool,
) {
    use tokio::io::AsyncWriteExt;
    let mut interrupted = false;
    let mut lost_output = false;
    loop {
        let delivery = live.take();
        if !delivery.is_empty() {
            let gap = !delivery.gaps.is_empty();
            (supervision.observe)(SupervisedEvent::Output(delivery.clone()));
            // A protocol cannot continue across missing bytes.
            if gap {
                lost_output = true;
                interrupted = true;
                break;
            }
            if !discard_stdout {
                let write = stdout.write_all(&delivery.bytes);
                tokio::select! {
                    _ = stop.cancelled() => { interrupted = true; break; }
                    result = write => if result.is_err() { interrupted = true; break; },
                }
            }
        }
        if live.finished() {
            break;
        }
        tokio::select! {
            _ = stop.cancelled() => { interrupted = true; break; }
            _ = tokio::time::sleep(Duration::from_millis(5)) => {}
        }
    }
    let descendants = if interrupted {
        match live.pid() {
            Some(pid) => super::descendants(pid).await,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let mut stopped = if interrupted {
        live.stop().await
    } else {
        live.wait().await
    };
    super::terminate_tree(&descendants, supervise::GRACE).await;
    if !stopped.rest.is_empty() {
        (supervision.observe)(SupervisedEvent::Output(stopped.rest.clone()));
        if !interrupted && !discard_stdout && stopped.rest.gaps.is_empty() {
            tokio::select! {
                _ = stop.cancelled() => {}
                _ = stdout.write_all(&stopped.rest.bytes) => {}
            }
        }
    }
    if lost_output || !stopped.rest.gaps.is_empty() {
        stopped.ending = supervise::Ending::Failed(
            "The agent sent more output than this computer could read.".into(),
        );
    }
    // Diagnostics are bounded by the same supervisor cap as stdout.
    let _ = tokio::time::timeout(
        supervise::GRACE,
        stderr.write_all(stopped.stderr.marked().as_bytes()),
    )
    .await;
    (supervision.observe)(SupervisedEvent::Stopped(stopped.clone()));
    let _ = ended.send(Some(std::sync::Arc::new(stopped)));
}

#[cfg(all(test, unix, feature = "supervised"))]
mod tests {
    use std::sync::{Arc, Mutex};
    use tokio::io::AsyncWriteExt;

    use super::*;

    async fn launch(
        script: &str,
        limits: supervise::Limits,
    ) -> (RuntimeChild, Arc<Mutex<Option<supervise::Stopped>>>) {
        let stopped = Arc::new(Mutex::new(None));
        let observed = stopped.clone();
        let (broker, mut requests) = super::super::LaunchBroker::channel();
        let broker = broker.supervised(Supervision {
            limits,
            observe: Arc::new(move |event| {
                if let SupervisedEvent::Stopped(result) = event {
                    *observed.lock().unwrap() = Some(result);
                }
            }),
        });
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        let start = RuntimeChild::launch(command, Some(&broker), RuntimePipes::Interactive);
        let wrap = async { requests.recv().await.unwrap().apply(&Ok) };
        let (child, ()) = tokio::join!(start, wrap);
        (child.unwrap(), stopped)
    }

    #[tokio::test]
    async fn protocol_driver_owns_input_while_the_supervisor_holds_the_job() {
        let (mut child, stopped) = launch(
            "read message; printf '%s' \"$message\"; printf diagnostic >&2",
            supervise::Limits::within(Duration::from_secs(5)),
        )
        .await;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"hello\n")
            .await
            .unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"hello");
        assert_eq!(output.stderr, b"diagnostic");
        let stopped = stopped.lock().unwrap();
        let stopped = stopped.as_ref().unwrap();
        assert!(stopped.group_clear);
        assert!(stopped.memory.is_some(), "default memory cap stays active");
    }

    #[tokio::test]
    async fn deadline_cleans_up_before_reporting_and_no_descendant_writes_later() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("escaped");
        let script = format!(
            "(sleep 0.5; printf escaped > '{}') & printf ready; wait",
            marker.display()
        );
        let (child, stopped) = launch(
            &script,
            supervise::Limits::within(Duration::from_millis(60)),
        )
        .await;
        let output = child.wait_with_output().await.unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"ready");
        let stopped = stopped.lock().unwrap().clone().unwrap();
        assert!(matches!(stopped.ending, supervise::Ending::TimedOut));
        assert!(stopped.group_clear);
        tokio::time::sleep(Duration::from_millis(550)).await;
        assert!(!marker.exists());
    }

    #[tokio::test]
    async fn unread_output_cannot_block_an_acknowledged_stop() {
        let (mut child, stopped) = launch("while :; do printf '012345678901234567890123456789012345678901234567890123456789012345'; done", supervise::Limits::within(Duration::from_secs(5)).keeping(1024 * 1024)).await;
        // Leave stdout unread long enough to fill the protocol bridge's pipe.
        tokio::time::sleep(Duration::from_millis(100)).await;
        tokio::time::timeout(
            Duration::from_secs(2),
            child.shutdown(Vec::new(), supervise::GRACE),
        )
        .await
        .unwrap();
        assert!(stopped.lock().unwrap().as_ref().unwrap().group_clear);
    }

    #[tokio::test]
    async fn dropping_the_protocol_driver_ends_its_supervised_job() {
        let (child, stopped) = launch(
            "sleep 60",
            supervise::Limits::within(Duration::from_secs(10)),
        )
        .await;
        drop(child);
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if stopped
                    .lock()
                    .unwrap()
                    .as_ref()
                    .is_some_and(|result| result.group_clear)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn protocol_gaps_fail_the_job_instead_of_replaying_partial_frames() {
        let (child, stopped) = launch(
            "printf '%020000d' 1; sleep 0.1",
            supervise::Limits::within(Duration::from_secs(5)).keeping(1),
        )
        .await;
        let output = child.wait_with_output().await.unwrap();
        assert!(!output.status.success());
        assert!(matches!(
            stopped.lock().unwrap().as_ref().unwrap().ending,
            supervise::Ending::Failed(_)
        ));
    }

    #[tokio::test]
    async fn probe_capture_keeps_draining_after_its_output_cap() {
        let (child, _) = launch(
            "printf '%0200000d' 1",
            supervise::Limits::within(Duration::from_secs(5)).keeping(1024 * 1024),
        )
        .await;
        let output = child.wait_with_output().await.unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 64 * 1024);
    }
}
