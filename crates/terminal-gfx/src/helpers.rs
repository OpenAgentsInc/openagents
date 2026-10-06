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
                    Some("answer") => value["text"].as_str().map(|text| Message::Answer(text.into())),
                    Some("door") => value["door"].as_str().map(|door| Message::Door(door.into())),
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

/// Reads thread `thread` through the shared chat client's read command,
/// which only reads. The answer is bounded in size and time; a client that
/// is missing, slow, or unreadable is unavailable, never retried here.
pub fn read_thread(thread: &str, home: Option<&Path>) -> Receiver<terminal_core::thread::Read> {
    use terminal_core::thread::{READ_MAX, Unread, decode};
    let (sender, receiver) = mpsc::channel();
    let thread = thread.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let Some(program) = crate::pty::candidates("openagents").into_iter().next() else {
            let _ = sender.send(Err(Unread::Unavailable(
                "the openagents command is not installed".into(),
            )));
            return;
        };
        let mut command = Command::new(program);
        if let Some(home) = &home {
            command.env("HOME", home);
        }
        let Ok(mut child) = command
            .args(["--json", "chat", "read", "--thread", &thread])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            let _ = sender.send(Err(Unread::Unavailable(
                "the chat client did not start".into(),
            )));
            return;
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return;
        };
        let (output_sender, output_receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.take(READ_MAX as u64 + 1).read_to_end(&mut bytes);
            let _ = output_sender.send(bytes);
        });
        let read = match output_receiver.recv_timeout(std::time::Duration::from_secs(20)) {
            Ok(bytes) => decode(&bytes, &thread),
            Err(_) => Err(Unread::Unavailable(
                "the chat client did not answer in time".into(),
            )),
        };
        let _ = child.kill();
        let _ = child.wait();
        let _ = sender.send(read);
    });
    receiver
}

/// Runs `openagents` with `args`, writing `input` to it, and answers its
/// standard output and error, each bounded to `max` bytes, or why it did
/// not answer within `deadline`.
fn helper(
    args: &[&str],
    input: Option<Vec<u8>>,
    home: Option<&Path>,
    max: usize,
    deadline: std::time::Duration,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let program = crate::pty::candidates("openagents")
        .into_iter()
        .next()
        .ok_or("the openagents command is not installed")?;
    let mut command = Command::new(program);
    if let Some(home) = home {
        command.env("HOME", home);
    }
    let mut child = command
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "the openagents command did not start")?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(pipe) = pipe {
                let _ = pipe.take(max as u64 + 1).read_to_end(&mut bytes);
            }
            let _ = sender.send(bytes);
        });
        receiver
    };
    let stdout = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let answer = stdout
        .recv_timeout(deadline)
        .ok()
        .zip(stderr.recv_timeout(std::time::Duration::from_secs(1)).ok());
    let _ = child.kill();
    let _ = child.wait();
    answer.ok_or_else(|| "the openagents command did not answer in time".into())
}

/// Reads Coder run `task` from the task owner's view, which only reads.
pub fn read_run(task: &str, home: Option<&Path>) -> Receiver<terminal_core::run::Read> {
    use terminal_core::run::{READ_MAX, STEPS, Unread, decode};
    let (sender, receiver) = mpsc::channel();
    let task = task.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let limit = STEPS.to_string();
        let read = match helper(
            &["--json", "task", "view", &task, "--limit", &limit],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        ) {
            Ok((stdout, stderr)) => decode(&stdout, &stderr, &task),
            Err(why) => Err(Unread::Unavailable(why)),
        };
        let _ = sender.send(read);
    });
    receiver
}

/// Reads retained file `path` of run `task` through the task owner, which
/// reads only by manifest path, and checks it against `digest`.
pub fn read_artifact(
    task: &str,
    path: &str,
    digest: &str,
    home: Option<&Path>,
) -> Receiver<terminal_core::files::Read> {
    use terminal_core::files::{READ_MAX, Unread, decode};
    let (sender, receiver) = mpsc::channel();
    let (task, path, digest) = (task.to_owned(), path.to_owned(), digest.to_owned());
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = match helper(
            &["--json", "task", "artifact", &task, "--path", &path],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        ) {
            Ok((stdout, stderr)) => decode(&stdout, &stderr, &path, &digest),
            Err(why) => Err(Unread::Unavailable(why)),
        };
        let _ = sender.send(read);
    });
    receiver
}

/// Reads this computer's background rules from the host's own store.
pub fn read_rules(home: Option<&Path>) -> Receiver<terminal_core::rules::Read> {
    use terminal_core::rules::{READ_MAX, decode};
    let (sender, receiver) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "background", "list"],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| decode(&stdout, &stderr));
        let _ = sender.send(read);
    });
    receiver
}

/// Lists the plugin test results under `root`; the listing reads each
/// report without checking it.
pub fn read_studies(root: &str, home: Option<&Path>) -> Receiver<terminal_core::gym::ListRead> {
    use terminal_core::gym::{READ_MAX, decode_list};
    let (sender, receiver) = mpsc::channel();
    let root = root.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "plugin", "test", "studies", &root],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| decode_list(&stdout, &stderr));
        let _ = sender.send(read);
    });
    receiver
}

/// Reads the installed plugins by exact release, with the test results
/// under `root` for each; it runs and probes nothing.
pub fn read_components(
    root: &str,
    home: Option<&Path>,
) -> Receiver<terminal_core::gym::ComponentsRead> {
    use terminal_core::gym::{READ_MAX, decode_components};
    let (sender, receiver) = mpsc::channel();
    let root = root.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "plugin", "inspect", "--results", &root],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| decode_components(&stdout, &stderr));
        let _ = sender.send(read);
    });
    receiver
}

/// Uses an installed plugin once through `openagents plugin use`: `id`,
/// `version`, `digest`, the request, and the workspace, in that order. The
/// thread is the terminal's, so the same request again follows the first
/// run instead of running twice.
pub fn plugin_use(terms: [&str; 5], home: Option<&Path>) -> Receiver<terminal_core::gym::UseRead> {
    use terminal_core::gym::decode_use;
    let (sender, receiver) = mpsc::channel();
    let terms = terms.map(str::to_owned);
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let [id, version, digest, request, workspace] = &terms;
        let answer = helper(
            &[
                "--json",
                "plugin",
                "use",
                id,
                "--version",
                version,
                "--digest",
                digest,
                "--request",
                request,
                "--in",
                workspace,
                "--thread",
                "terminal",
            ],
            None,
            home.as_deref(),
            256 * 1024,
            std::time::Duration::from_secs(120),
        )
        .and_then(|(stdout, stderr)| decode_use(&stdout, &stderr));
        let _ = sender.send(answer);
    });
    receiver
}

/// Searches local and trusted knowledge entries, lexically: no model runs
/// and nothing is spent.
pub fn search_knowledge(
    query: &str,
    home: Option<&Path>,
) -> Receiver<terminal_core::knowledge::HitsRead> {
    use terminal_core::knowledge::{READ_MAX, decode_hits};
    let (sender, receiver) = mpsc::channel();
    let query = query.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &[
                "--json",
                "kb",
                "search",
                &query,
                "--lexical",
                "--limit",
                "20",
            ],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(30),
        )
        .and_then(|(stdout, stderr)| decode_hits(&stdout, &stderr));
        let _ = sender.send(read);
    });
    receiver
}

/// Reads knowledge entry `id` at its current version.
pub fn read_entry(id: &str, home: Option<&Path>) -> Receiver<terminal_core::knowledge::ShownRead> {
    use terminal_core::knowledge::{READ_MAX, decode_shown};
    let (sender, receiver) = mpsc::channel();
    let id = id.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "kb", "show", &id],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| decode_shown(&stdout, &stderr, &id));
        let _ = sender.send(read);
    });
    receiver
}

/// Reads the studio's goals and their plans.
pub fn read_goals(home: Option<&Path>) -> Receiver<terminal_core::knowledge::GoalsRead> {
    use terminal_core::knowledge::{READ_MAX, decode_goals};
    let (sender, receiver) = mpsc::channel();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "studio", "goal", "list"],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| decode_goals(&stdout, &stderr));
        let _ = sender.send(read);
    });
    receiver
}

/// Recomputes the retained plugin test result in `dir` from its attempts.
/// It runs nothing and publishes nothing.
pub fn read_study(dir: &str, home: Option<&Path>) -> Receiver<terminal_core::gym::Read> {
    use terminal_core::gym::{READ_MAX, decode};
    let (sender, receiver) = mpsc::channel();
    let dir = dir.to_owned();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let read = helper(
            &["--json", "plugin", "test", "show", &dir],
            None,
            home.as_deref(),
            READ_MAX,
            std::time::Duration::from_secs(60),
        )
        .and_then(|(stdout, stderr)| decode(&stdout, &stderr, &dir));
        let _ = sender.send(read);
    });
    receiver
}

/// Pauses or resumes background rule `id` through the host's existing
/// command; `verb` is `pause` or `resume` and nothing else.
pub fn rule_command(verb: &str, id: &str, home: Option<&Path>) -> Receiver<Result<(), String>> {
    let (sender, receiver) = mpsc::channel();
    let (verb, id) = (verb.to_owned(), id.to_owned());
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        if verb != "pause" && verb != "resume" {
            let _ = sender.send(Err("only pause and resume are sent".into()));
            return;
        }
        let answer = helper(
            &["--json", "background", &verb, &id],
            None,
            home.as_deref(),
            64 * 1024,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| terminal_core::rules::decode_change(&stdout, &stderr, &id));
        let _ = sender.send(answer);
    });
    receiver
}

/// Hands `text` to workshop agent `agent` with `openagents agent ask`,
/// which sends NIP-HOST `studio.agent.ask` to the host.
pub fn ask_agent(
    agent: &str,
    text: &str,
    directory: Option<&str>,
    home: Option<&Path>,
) -> Receiver<Result<String, String>> {
    let (sender, receiver) = mpsc::channel();
    let (agent, text) = (agent.to_owned(), text.to_owned());
    let directory = directory.map(str::to_owned);
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let mut args = vec!["--json", "agent", "ask", agent.as_str(), text.as_str()];
        if let Some(directory) = &directory {
            args.extend(["--from", directory.as_str()]);
        }
        let answer = helper(
            &args,
            None,
            home.as_deref(),
            64 * 1024,
            std::time::Duration::from_secs(20),
        )
        .and_then(|(stdout, stderr)| {
            let value: serde_json::Value = serde_json::from_slice(&stdout).map_err(|_| {
                let said = String::from_utf8_lossy(&stderr).trim().to_string();
                if said.is_empty() {
                    "the host did not answer".to_string()
                } else {
                    said
                }
            })?;
            match value["error"].as_str() {
                Some(error) => Err(error.to_string()),
                None => Ok(format!(
                    "Asked {agent}. She reports at her desk and in her thread."
                )),
            }
        });
        let _ = sender.send(answer);
    });
    receiver
}

/// Sends task command `bytes` to the task owner's `verb`. The command
/// keeps its ID, so an unknown outcome may be retried with the same bytes.
pub fn task_command(
    verb: &str,
    bytes: &[u8],
    home: Option<&Path>,
) -> Receiver<terminal_core::run::Sent> {
    use terminal_core::run::{Sent, decode_receipt};
    let (sender, receiver) = mpsc::channel();
    let verb = verb.to_owned();
    let bytes = bytes.to_vec();
    let home = home.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let id = serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .and_then(|value| value["command_id"].as_str().map(str::to_owned))
            .unwrap_or_default();
        let sent = match helper(
            &["--json", "task", &verb, "--file", "-"],
            Some(bytes),
            home.as_deref(),
            64 * 1024,
            std::time::Duration::from_secs(20),
        ) {
            Ok((stdout, stderr)) => decode_receipt(&stdout, &stderr, &id),
            Err(why) => Sent::Unknown(why),
        };
        let _ = sender.send(sent);
    });
    receiver
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
