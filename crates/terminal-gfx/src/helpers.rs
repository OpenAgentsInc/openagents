//! Bounded native helper processes behind the application's typed bridge.
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use terminal_core::bridge::{Connection, Message, Process, Request};
use terminal_core::smart::scrub;
pub struct Worker {
    pub events: Receiver<Message>,
    child: Child,
    started: std::time::Instant,
}

impl Worker {
    pub fn start(request: Request, home: Option<PathBuf>) -> Result<Self, String> {
        let program = crate::pty::candidates("openagents")
            .into_iter()
            .next()
            .ok_or("openagents helper not found")?;
        let mut command = Command::new(program);
        if let Some(home) = &home {
            command.env("HOME", home);
        }
        let mut child = command
            .args(["--json", "chat", "shell-request", "-"])
            .current_dir(&request.binding.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "terminal request helper did not start")?;
        let body =
            serde_json::to_vec(&request).map_err(|_| "terminal request could not be encoded")?;
        let mut input = child
            .stdin
            .take()
            .ok_or("terminal request helper has no input")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("terminal request helper has no output")?;
        let (sender, events) = mpsc::channel();
        std::thread::spawn(move || {
            if input.write_all(&body).is_err() {
                return;
            }
            drop(input);
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                // The shared helper emits bounded NDJSON; a malformed line fails closed.
                if (&mut reader)
                    .take(256 * 1024 + 1)
                    .read_line(&mut line)
                    .ok()
                    .is_none_or(|size| size == 0)
                {
                    break;
                }
                if line.len() > 256 * 1024 {
                    break;
                }
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                let message = match value["event"].as_str() {
                    Some("attached") => value["thread"].as_str().map(|thread| Message::Attached(thread.into())),
                    Some("shell-proposal") => serde_json::from_value(value["proposal"].clone()).ok().map(|proposal| Message::Proposal(proposal, if value["effect"] == "read_only" { terminal_core::proposals::Effect::Ordinary } else { terminal_core::proposals::Effect::Destructive("This command may change files or this computer, or publish data. Press Enter again to approve it.".into()) })),
                    _ => None,
                };
                if let Some(message) = message {
                    let _ = sender.send(message);
                }
            }
        });
        Ok(Self {
            events,
            child,
            started: std::time::Instant::now(),
        })
    }

    pub fn ended(&mut self) -> Option<bool> {
        if self.started.elapsed() > std::time::Duration::from_secs(150) {
            let _ = self.child.kill();
        }
        self.child
            .try_wait()
            .ok()
            .flatten()
            .map(|status| status.success())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Process for Worker {
    fn ended(&mut self) -> Option<bool> {
        Worker::ended(self)
    }
}
pub fn request(request: &Request, home: Option<&Path>) -> Result<Connection, String> {
    let mut worker = Worker::start(request.clone(), home.map(Path::to_path_buf))?;
    let (_, empty) = mpsc::channel();
    let events = std::mem::replace(&mut worker.events, empty);
    Ok(Connection {
        events,
        process: Box::new(worker),
    })
}

pub fn git_summary(
    pane_id: u64,
    directory: String,
    home: Option<&Path>,
) -> Receiver<(u64, String, String)> {
    let (sender, receiver) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let mut command = Command::new("git");
        if let Some(home) = &home {
            command.env("HOME", home);
        }
        let Ok(mut child) = command
            .args(["status", "--short", "--branch"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            return;
        };
        let (output_sender, output_receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout.take(8193).read_to_end(&mut bytes);
            let _ = output_sender.send((result, bytes));
        });
        if let Ok((Ok(_), bytes)) = output_receiver.recv_timeout(std::time::Duration::from_secs(2))
            && bytes.len() <= 8192
            && {
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
                loop {
                    if let Ok(Some(status)) = child.try_wait() {
                        break status.success();
                    }
                    if std::time::Instant::now() >= deadline {
                        break false;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        {
            let summary = scrub(&String::from_utf8_lossy(&bytes));
            let _ = sender.send((pane_id, directory, summary));
        }
        let _ = child.kill();
        let _ = child.wait();
    });
    receiver
}
