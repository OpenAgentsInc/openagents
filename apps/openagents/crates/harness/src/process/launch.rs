//! Host command wrapping without moving the host's borrowed policy into a task.

use std::io;

use tokio::sync::{mpsc, oneshot};

use super::Command;

/// Sends each prepared run command to the host before anything starts.
#[derive(Clone)]
pub struct LaunchBroker(mpsc::UnboundedSender<LaunchRequest>);

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
        (Self(tx), rx)
    }

    pub(crate) async fn prepare(&self, command: Command) -> io::Result<Command> {
        let (reply, response) = oneshot::channel();
        self.0
            .send(LaunchRequest {
                command: command.into_std(),
                reply,
            })
            .map_err(|_| unavailable())?;
        let command = response
            .await
            .map_err(|_| unavailable())?
            .map_err(|reason| io::Error::new(io::ErrorKind::PermissionDenied, reason))?;
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
