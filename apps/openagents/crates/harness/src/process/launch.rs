//! Host command wrapping without moving the host's borrowed policy into a task.

use std::io;

use tokio::sync::{mpsc, oneshot};

use super::Command;

/// Sends each prepared run command to the host before anything starts.
#[derive(Clone)]
pub struct LaunchBroker {
    sender: mpsc::UnboundedSender<LaunchRequest>,
    pub(crate) stop: tokio_util::sync::CancellationToken,
    pub(crate) options: crate::host::NativeOptions,
    accounting: std::sync::Arc<std::sync::Mutex<crate::host::AccountingReader>>,
    #[cfg(feature = "supervised")]
    pub(crate) supervision: Option<super::Supervision>,
}

/// The host receives launch requests while polling the harness's run future.
pub type LaunchRequests = mpsc::UnboundedReceiver<LaunchRequest>;

/// One command awaiting the host's boundary wrapper.
pub struct LaunchRequest {
    command: std::process::Command,
    reply: oneshot::Sender<Result<std::process::Command, String>>,
}

impl LaunchBroker {
    /// The receiver stays with the host; only the sender enters the harness.
    pub fn channel() -> (Self, LaunchRequests) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                sender: tx,
                stop: tokio_util::sync::CancellationToken::new(),
                options: Default::default(),
                accounting: Default::default(),
                #[cfg(feature = "supervised")]
                supervision: None,
            },
            rx,
        )
    }

    pub fn with_host_options(mut self, options: crate::host::NativeOptions) -> Self {
        self.options = options;
        self
    }

    pub fn accounting(&self) -> crate::host::Accounting {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot
            .clone()
    }

    /// Stop every process this run started through the host supervisor.
    pub fn stop_processes(&self) {
        self.stop.cancel();
    }

    /// Return a command's native exit status once its tool result arrives.
    pub fn take_exit_code(&self, id: &str) -> Option<i64> {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take_exit_code(id)
    }

    /// Full file paths behind a completed native patch tool.
    pub fn take_changes(&self, id: &str) -> Result<Vec<(String, String)>, String> {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take_changes(id)
    }

    pub(crate) fn record_claude(&self, line: &str) {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .claude(line);
    }
    pub(crate) fn record_codex(&self, line: &str) {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .codex(line);
    }
    pub(crate) fn record_opencode(&self, event: &serde_json::Value, session: &str) {
        self.accounting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .opencode(event, session);
    }

    /// Keep the host's process supervisor around the native protocol driver.
    #[cfg(feature = "supervised")]
    pub fn supervised(mut self, supervision: super::Supervision) -> Self {
        self.supervision = Some(supervision);
        self
    }

    pub(crate) async fn prepare(&self, command: Command) -> io::Result<Command> {
        if self.stop.is_cancelled() {
            return Err(unavailable());
        }
        let (reply, response) = oneshot::channel();
        self.sender
            .send(LaunchRequest {
                command: command.into_std(),
                reply,
            })
            .map_err(|_| unavailable())?;
        let command = response
            .await
            .map_err(|_| unavailable())?
            .map_err(|reason| io::Error::new(io::ErrorKind::PermissionDenied, reason))?;
        if self.stop.is_cancelled() {
            return Err(unavailable());
        }
        let mut command = Command::from(command);
        // Tokio's kill-on-drop flag is not part of std::process::Command.
        command.kill_on_drop(true).process_group(0);
        Ok(command)
    }
}

impl LaunchRequest {
    /// Apply the policy on the host's task. It may borrow non-Send state.
    pub fn apply(
        self,
        wrap: &dyn Fn(std::process::Command) -> Result<std::process::Command, String>,
    ) {
        // A cancelled launch cannot start later when its request is drained.
        if !self.reply.is_closed() {
            let _ = self.reply.send(wrap(self.command));
        }
    }
}

fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "the computer stopped accepting agent launches",
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::process::Stdio;
    use std::rc::Rc;

    use super::*;

    #[tokio::test]
    async fn borrowed_wrapper_preserves_directory_environment_and_pipes() {
        let dir = tempfile::tempdir().unwrap();
        let called = Rc::new(Cell::new(0));
        let wrap = |mut command: std::process::Command| {
            called.set(called.get() + 1);
            command.env_clear().env("WRAPPED", "yes");
            Ok(command)
        };
        let (broker, mut requests) = LaunchBroker::channel();
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "printf '%s\\n' \"$WRAPPED\"; /bin/pwd; read text; printf '%s' \"$text\"",
            ])
            .current_dir(dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let prepare = broker.prepare(command);
        let serve = async { requests.recv().await.unwrap().apply(&wrap) };
        let (command, ()) = tokio::join!(prepare, serve);
        let mut child = command.unwrap().spawn().unwrap();
        let pid = child.id().unwrap() as i32;
        // SAFETY: inspect the live fixture child before writing its input.
        assert_eq!(unsafe { libc::getpgid(pid) }, pid);
        use tokio::io::AsyncWriteExt;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"hello\n")
            .await
            .unwrap();
        let output = child.wait_with_output().await.unwrap();
        assert!(output.status.success());
        assert_eq!(called.get(), 1);
        let expected = format!(
            "yes\n{}\nhello",
            dir.path().canonicalize().unwrap().display()
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }

    #[tokio::test]
    async fn refusal_and_broker_loss_never_return_an_unwrapped_command() {
        let (broker, mut requests) = LaunchBroker::channel();
        let prepare = broker.prepare(Command::new("/bin/sh"));
        let serve = async {
            requests
                .recv()
                .await
                .unwrap()
                .apply(&|_| Err("blocked by host".into()));
        };
        let (result, ()) = tokio::join!(prepare, serve);
        assert_eq!(result.unwrap_err().to_string(), "blocked by host");
        drop(requests);
        assert_eq!(
            broker
                .prepare(Command::new("/bin/sh"))
                .await
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[tokio::test]
    async fn a_stopped_host_refuses_future_launches_before_requesting_a_wrapper() {
        let (broker, mut requests) = LaunchBroker::channel();
        broker.stop_processes();
        assert!(broker.prepare(Command::new("/bin/sh")).await.is_err());
        assert!(requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn unanswered_request_and_cancelled_launch_fail_closed() {
        let (broker, mut requests) = LaunchBroker::channel();
        let prepare = broker.prepare(Command::new("/bin/sh"));
        let serve = async {
            drop(requests.recv().await.unwrap());
        };
        let (result, ()) = tokio::join!(prepare, serve);
        assert!(result.is_err());

        let request = {
            let prepare = broker.prepare(Command::new("/bin/sh"));
            tokio::pin!(prepare);
            tokio::select! {
                _ = &mut prepare => panic!("launch must wait for the host"),
                request = requests.recv() => request.unwrap(),
            }
        };
        request.apply(&|_| panic!("cancelled launch must not invoke the wrapper"));
    }
}
