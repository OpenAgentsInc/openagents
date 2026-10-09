//! Coder V1 as the engine of the coding agents in Verse (#10753, #10754).
//!
//! Every agent that codes, Alice the workshop agent and the Agent Studio's
//! seats, runs its work as programmatic turns of Coder V1
//! (`crates/coder-new`): `openagents coder chat --json`, one durable
//! session per agent, in the agent's working directory. This module is
//! the host's side of that: where the two programs are
//! ([`cli_binary`], [`tui_binary`]), one turn as a supervised child
//! ([`Cli`]), and Coder's NDJSON events as typed [`Event`]s.
//!
//! A turn may be gated (`--approvals stdin`): Coder asks before any command
//! that is not read-only, and the host's answer goes back on the child's
//! standard input. The agent's pane runs Coder's own terminal on the same
//! session in follow mode ([`follow_command`]), so the owner watches the
//! work and takes it over with a key.
//!
//! This crate cannot link `coder-new`, which depends on it, so the turn is
//! a child process.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use serde_json::Value;

/// The `openagents` program whose `coder` command runs a turn, when it is
/// not beside this program or on `PATH`.
pub const CLI_VAR: &str = "OPENAGENTS_CODER_CLI";
/// The Coder V1 terminal an agent's pane runs, when it is not beside this
/// program or the CLI.
pub const TUI_VAR: &str = "OPENAGENTS_CODER_TUI";
/// How long a cancelled turn has to save its session and exit after
/// `SIGINT` before it is killed.
const CANCEL_GRACE: Duration = Duration::from_secs(10);
/// The most bytes of a failed turn's standard error the host keeps.
const STDERR_TAIL: usize = 4096;

fn session_name(agent: &str) -> String {
    agent
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(100)
        .collect()
}

/// The session an agent's turns shared before she steered Coder:
/// `agent-NAME`, whose saved instructions carry her persona. It stays
/// readable, and `/resume` still lists it; no new turn goes there.
#[must_use]
pub fn session_for(agent: &str) -> String {
    format!("agent-{}", session_name(agent))
}

/// The plain Coder session an agent steers: `NAME-coder`. Her prompts are
/// its user turns, and it carries no instructions, so nothing in it says
/// who she is.
#[must_use]
pub fn coder_session_for(agent: &str) -> String {
    format!("{}-coder", session_name(agent))
}

/// The Coder store the host's turns use: `~/.openagents/coder-new`, the
/// same store the owner's own Coder reads, so `/resume` lists the agents'
/// sessions too.
#[must_use]
pub fn default_state() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents/coder-new"))
}

fn executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|path| executable(path))
    })
}

fn beside_this(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let path = dir.join(name);
    executable(&path).then_some(path)
}

/// The `openagents` program for `openagents coder`: [`CLI_VAR`], this
/// program when it is `openagents`, the one beside this program, or the
/// first on `PATH`.
///
/// # Errors
/// None is found.
pub fn cli_binary() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os(CLI_VAR).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        return if executable(&path) {
            Ok(path)
        } else {
            Err(format!(
                "{CLI_VAR} names {}, which isn't a program",
                path.display()
            ))
        };
    }
    if let Ok(exe) = std::env::current_exe()
        && exe.file_stem().is_some_and(|stem| stem == "openagents")
    {
        return Ok(exe);
    }
    beside_this("openagents")
        .or_else(|| on_path("openagents"))
        .ok_or_else(|| {
            format!(
                "Coder V1 is not installed: no openagents program beside this one or on PATH; \
                 install Coder with `curl -fsSL https://openagents.com/cli/install.sh | bash` \
                 (Windows PowerShell: `irm https://openagents.com/cli/install.ps1 | iex`) or set {CLI_VAR}"
            )
        })
}

/// Whether `path` is Coder V1's terminal, which has a follow mode, rather
/// than another program named `coder`.
fn follows(path: &Path) -> bool {
    Command::new(path)
        .arg("--help")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains("--follow"))
}

/// Coder V1's terminal for an agent's pane: [`TUI_VAR`], `coder-new` beside
/// this program or the CLI (a development build), `coder` beside them (an
/// installed release), or the first `coder-new` or `coder` on `PATH` that
/// has a follow mode.
#[must_use]
pub fn tui_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(TUI_VAR).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    let cli_dir = cli_binary()
        .ok()
        .and_then(|cli| cli.parent().map(Path::to_path_buf));
    let mut candidates = Vec::new();
    for name in ["coder-new", "coder"] {
        candidates.extend(beside_this(name));
        if let Some(dir) = &cli_dir {
            let path = dir.join(name);
            if executable(&path) {
                candidates.push(path);
            }
        }
        candidates.extend(on_path(name));
    }
    candidates.into_iter().find(|path| follows(path))
}

/// The command line an agent's pane runs: Coder V1's terminal following
/// `session` in `state`, working in `cwd`.
#[must_use]
pub fn follow_command(tui: &Path, session: &str, state: &Path, cwd: &str) -> Vec<String> {
    vec![
        tui.display().to_string(),
        "--follow".into(),
        session.into(),
        "--state".into(),
        state.display().to_string(),
        "--in".into(),
        cwd.into(),
    ]
}

/// Where Coder keeps `session`'s lock in `state`; Coder V1 holds it with
/// an exclusive file lock while a process uses the session.
#[must_use]
pub fn lock_path(state: &Path, session: &str) -> PathBuf {
    state.join("sessions").join(format!("{session}.atif.lock"))
}

/// Where an agent asks the process holding her session to hand it back.
/// Coder V1's terminal reads it while it holds the session: idle, it saves,
/// lets go, and follows again; mid-turn, it writes [`RECLAIM_BUSY`] and
/// lets go when its turn ends.
#[must_use]
pub fn reclaim_path(state: &Path, session: &str) -> PathBuf {
    state
        .join("sessions")
        .join(format!("{session}.atif.reclaim"))
}

/// What a holder mid-turn writes into [`reclaim_path`].
pub const RECLAIM_BUSY: &str = "busy";

/// Whether another process holds `session`'s lock right now. A process
/// that crashed or quit holds nothing: the system drops its lock.
#[must_use]
pub fn session_held(state: &Path, session: &str) -> bool {
    let Ok(file) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(lock_path(state, session))
    else {
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// Why an agent could not take her session back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unreclaimed {
    /// The kill switch or the owner stopped her while she waited.
    Stopped,
    /// The holder kept it past the limit.
    Busy,
}

/// How long a holder has to answer a reclaim before the agent treats it
/// as a pane from before the reclaim protocol.
const RECLAIM_ANSWER: Duration = Duration::from_secs(3);

/// Takes `session` back from whatever holds it: Coder V1's terminal in the
/// agent's pane hands it back when idle, or when its turn ends; a pane
/// from before this protocol that sits idle is ended. `queued` runs once
/// when the holder is mid-turn, so the agent waits for that turn.
///
/// # Errors
/// `stop` was set, or the holder kept the session past `limit`.
pub fn reclaim(
    state: &Path,
    session: &str,
    stop: &AtomicBool,
    limit: Duration,
    mut queued: impl FnMut(),
) -> Result<(), Unreclaimed> {
    if !session_held(state, session) {
        return Ok(());
    }
    let marker = reclaim_path(state, session);
    // Whole or absent, so a holder's answer never interleaves with it.
    let staged = marker.with_extension("reclaim.tmp");
    if std::fs::write(&staged, "reclaim\n").is_ok() {
        let _ = std::fs::rename(&staged, &marker);
    }
    let started = Instant::now();
    let mut told = false;
    let mut ended_pane = false;
    let mut tried: Option<Instant> = None;
    let result = loop {
        if !session_held(state, session) {
            if ended_pane {
                // Let the window see the old pane exit before her new turn.
                std::thread::sleep(Duration::from_millis(500));
            }
            break Ok(());
        }
        if stop.load(Ordering::SeqCst) {
            break Err(Unreclaimed::Stopped);
        }
        if started.elapsed() > limit {
            break Err(Unreclaimed::Busy);
        }
        let busy = std::fs::read_to_string(&marker).is_ok_and(|text| text.trim() == RECLAIM_BUSY);
        if busy && !told {
            told = true;
            queued();
        }
        // A holder that never answers is a pane from before this protocol:
        // end it once its session has gone quiet.
        if !busy
            && !ended_pane
            && started.elapsed() > RECLAIM_ANSWER
            && tried.is_none_or(|at: Instant| at.elapsed() > Duration::from_secs(2))
        {
            tried = Some(Instant::now());
            ended_pane = end_idle_pane(state, session);
            if !ended_pane && !told {
                told = true;
                queued();
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = std::fs::remove_file(&marker);
    result
}

/// Ends a pane that follows `session` and holds it without answering a
/// reclaim, a terminal from before the protocol, when the session has
/// been quiet long enough that no turn is running in it.
#[cfg(unix)]
fn end_idle_pane(state: &Path, session: &str) -> bool {
    let document = state.join("sessions").join(format!("{session}.atif.json"));
    let quiet = std::fs::metadata(&document)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_none_or(|age| age > Duration::from_secs(10));
    if !quiet {
        return false;
    }
    let lock = lock_path(state, session);
    let Ok(out) = Command::new("lsof").arg("-t").arg("--").arg(&lock).output() else {
        return false;
    };
    let mut ended = false;
    for pid in String::from_utf8_lossy(&out.stdout).split_whitespace() {
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if pid == std::process::id() {
            continue;
        }
        let Ok(ps) = Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
        else {
            continue;
        };
        let command = String::from_utf8_lossy(&ps.stdout);
        let words: Vec<&str> = command.split_whitespace().collect();
        let follows = words
            .windows(2)
            .any(|pair| pair[0] == "--follow" && pair[1] == session);
        if follows
            && Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .is_ok_and(|s| s.success())
        {
            ended = true;
        }
    }
    ended
}

#[cfg(not(unix))]
fn end_idle_pane(_state: &Path, _session: &str) -> bool {
    false
}

/// One thing a Coder turn reported. A recorded turn ([`Scripted`]) is a
/// JSON list of these, each tagged by `event`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// Reply text as it streams.
    Delta { text: String },
    /// A tool call started (`running`) or ended, such as Microcoder's
    /// `Run` with the command as `input`. `delegation` names the child
    /// chat it ran in, if any.
    Tool {
        name: String,
        #[serde(default)]
        input: Value,
        #[serde(default)]
        output: Value,
        #[serde(default)]
        running: bool,
        #[serde(default)]
        delegation: Option<String>,
    },
    /// The model that answers.
    Model { model: String },
    /// Coder asks before running `command`, which is not read-only.
    Approval {
        id: u64,
        command: String,
        #[serde(default)]
        why: String,
    },
    /// An approval was answered.
    Answered { id: u64, confirm: bool },
    /// Coder delegated work to `agent`, such as Codex, in the child chat
    /// `id`: running, or ended with `output` (its reply, model, and usage,
    /// or `error`).
    Delegation {
        id: String,
        agent: String,
        #[serde(default)]
        running: bool,
        #[serde(default)]
        output: Value,
    },
}

/// How a turn ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ended {
    /// Coder answered: its reply and the tokens the turn used.
    Finished { reply: String, tokens: u64 },
    /// It stopped with this error.
    Failed(String),
    /// The host cancelled it; the session keeps what it did.
    Cancelled,
}

/// What a workshop agent on Codex adds to each prompt: Coder, on its own
/// model, hands the coding to Codex and checks what it did. A turn whose
/// prompt carries it runs with [`Turn::codex_writes`].
pub const CODEX_DIRECTIVE: &str = "Delegate the file edits in this task to the codex agent \
     with acp_subagent: give it the task, the files involved, and how to check the result. \
     Then read its changes and run the checks yourself before you answer. Answer questions \
     and do read-only lookups yourself with read-only commands; never delegate them. If Codex \
     is unavailable or out of capacity, do the edits yourself and say so.";

/// What a workshop agent on Devin adds to each prompt: Coder, on its own
/// model, hands the coding to the Devin CLI through the `devin-cli`
/// subagent and checks what it did, falling back to Codex, then to its own
/// model (#10929).
pub const DEVIN_DIRECTIVE: &str = "Delegate the file edits in this task to the devin-cli agent \
     with acp_subagent: give it the task, the files involved, and how to check the result. \
     Then read its changes and run the checks yourself before you answer. Answer questions \
     and do read-only lookups yourself with read-only commands; never delegate them. If Devin \
     is unavailable or out of capacity, delegate the edits to the codex agent instead; if Codex \
     is also unavailable or out of capacity, do the edits yourself and say so.";

/// The [`DEVIN_DIRECTIVE`] plus the sentence naming the model `devin:MODEL`
/// asks for, so Coder passes `model` on its `acp_subagent` call.
#[must_use]
pub fn devin_directive(model: Option<&str>) -> String {
    match model {
        Some(model) => format!(
            "{DEVIN_DIRECTIVE} Call acp_subagent with agent `devin-cli` and model `{model}`."
        ),
        None => DEVIN_DIRECTIVE.to_owned(),
    }
}

/// One turn to run.
#[derive(Clone, Debug)]
pub struct Turn {
    /// The working directory.
    pub cwd: PathBuf,
    /// The Coder store.
    pub state: PathBuf,
    /// The session the turn continues or starts.
    pub session: String,
    /// The owner's words, exactly as the session and its follower show them.
    pub prompt: String,
    /// Standing instructions, such as a workshop agent's charter: the model
    /// reads them as system instructions, and no transcript shows them.
    pub instructions: Option<String>,
    /// Ask before any command that is not read-only.
    pub approvals: bool,
    /// Let the turn's Codex delegations edit the working directory under
    /// Codex's workspace-write sandbox (`--codex-writes`); a gated turn
    /// keeps Codex read-only.
    pub codex_writes: bool,
    /// Disable every model tool, including read-only commands and delegation.
    pub tool_free: bool,
}

/// What runs a turn: Coder V1 ([`Cli`]) or, in tests, a stand-in.
pub trait Engine: Send {
    /// Runs `turn`. `hear` gets each event; for an [`Event::Approval`] it
    /// returns the answer, `Some(true)` to confirm. `cancel` stops it.
    fn turn(
        &mut self,
        turn: &Turn,
        cancel: &AtomicBool,
        hear: &mut dyn FnMut(&Event) -> Option<bool>,
    ) -> Ended;
}

/// One NDJSON line of `openagents coder chat --json`, as an event, or
/// `Err` with the result document when it is the last line.
fn parse(line: &str) -> Option<Result<Event, Value>> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let tool = |entry: &Value, delegation: Option<String>| {
        (entry.get("source").and_then(Value::as_str) == Some("tool")).then(|| Event::Tool {
            name: entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            input: entry.get("input").cloned().unwrap_or(Value::Null),
            output: entry.get("output").cloned().unwrap_or(Value::Null),
            running: entry.get("running").and_then(Value::as_bool) == Some(true),
            delegation,
        })
    };
    if value.get("error").is_some() || value.get("reply").is_some() {
        return Some(Err(value));
    }
    let event = match value.get("event").and_then(Value::as_str)? {
        "delta" => Event::Delta { text: text("text") },
        "model" => Event::Model {
            model: text("model"),
        },
        "entry" if value.pointer("/entry/source").and_then(Value::as_str) == Some("delegation") => {
            let entry = value.get("entry")?;
            let field = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            Event::Delegation {
                id: field("id"),
                agent: field("name"),
                running: entry.get("running").and_then(Value::as_bool) == Some(true),
                output: entry.get("output").cloned().unwrap_or(Value::Null),
            }
        }
        "entry" => tool(value.get("entry")?, None)?,
        "delegation_entry" => tool(value.get("entry")?, Some(text("delegation")))?,
        "approval" => Event::Approval {
            id: value.get("id").and_then(Value::as_u64)?,
            command: text("command"),
            why: text("why"),
        },
        "approval_answered" => Event::Answered {
            id: value.get("id").and_then(Value::as_u64)?,
            confirm: value.get("decision").and_then(Value::as_str) == Some("confirm"),
        },
        _ => return None,
    };
    Some(Ok(event))
}

/// Coder V1 itself: `openagents coder chat --json` as a child process.
#[derive(Clone, Debug)]
pub struct Cli {
    pub program: PathBuf,
    /// Variables to set for it, such as the owner's login environment.
    pub env: Vec<(String, String)>,
}

impl Cli {
    /// Coder V1 on this computer.
    ///
    /// # Errors
    /// It is not installed.
    pub fn found() -> Result<Self, String> {
        cli_binary().map(|program| Self {
            program,
            env: Vec::new(),
        })
    }

    fn command(&self, turn: &Turn) -> Command {
        let mut command = Command::new(&self.program);
        command
            .arg("coder")
            .arg("chat")
            .arg("--json")
            .arg("--session")
            .arg(&turn.session)
            .arg("--state")
            .arg(&turn.state)
            .arg("--in")
            .arg(&turn.cwd)
            .arg("-p")
            .arg(&turn.prompt);
        if let Some(instructions) = turn
            .instructions
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            command.arg("--instructions").arg(instructions);
        }
        if turn.tool_free {
            command.arg("--approvals").arg("tool-free");
        } else if turn.approvals {
            command.arg("--approvals").arg("stdin");
        }
        if turn.codex_writes && !turn.tool_free {
            command.arg("--codex-writes");
        }
        command.envs(self.env.iter().map(|(key, value)| (key, value)));
        command
            .current_dir(&turn.cwd)
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .stdin(if turn.approvals {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

fn interrupt(child: &Child) {
    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(child.id()) {
        // SAFETY: a signal to the child this host started and still owns.
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }
}

fn tail(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.trim().to_owned();
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].trim().to_owned()
}

impl Engine for Cli {
    fn turn(
        &mut self,
        turn: &Turn,
        cancel: &AtomicBool,
        hear: &mut dyn FnMut(&Event) -> Option<bool>,
    ) -> Ended {
        if let Err(why) = std::fs::create_dir_all(&turn.state) {
            return Ended::Failed(format!("cannot make the Coder store: {why}"));
        }
        let mut child = match self.command(turn).spawn() {
            Ok(child) => child,
            Err(why) => {
                return Ended::Failed(format!(
                    "Coder V1 did not start ({}): {why}",
                    self.program.display()
                ));
            }
        };
        let mut stdin = child.stdin.take();
        let (lines, from_child) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if lines.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let stderr = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let mut text = String::new();
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    text.push_str(&line);
                    text.push('\n');
                    if text.len() > 4 * STDERR_TAIL {
                        text = tail(&text, STDERR_TAIL);
                    }
                }
                text
            })
        });
        let mut result: Option<Value> = None;
        let mut interrupted: Option<Instant> = None;
        loop {
            if cancel.load(Ordering::SeqCst) && interrupted.is_none() {
                interrupt(&child);
                // Closing its input rejects a waiting approval.
                stdin = None;
                interrupted = Some(Instant::now());
            }
            if interrupted.is_some_and(|at| at.elapsed() > CANCEL_GRACE) {
                let _ = child.kill();
            }
            match from_child.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => match parse(&line) {
                    Some(Ok(event)) => {
                        let answer = hear(&event);
                        if let (Event::Approval { id, .. }, Some(input)) = (&event, stdin.as_mut())
                        {
                            let word = if answer == Some(true) {
                                "confirm"
                            } else {
                                "reject"
                            };
                            let _ = writeln!(input, "{word} {id}");
                            let _ = input.flush();
                        }
                    }
                    Some(Err(document)) => result = Some(document),
                    None => {}
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        drop(stdin);
        let status = child.wait();
        let stderr = stderr
            .and_then(|reader| reader.join().ok())
            .unwrap_or_default();
        if interrupted.is_some() {
            return Ended::Cancelled;
        }
        match result {
            Some(document) if document.get("error").is_none() => Ended::Finished {
                reply: document
                    .get("reply")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                tokens: document
                    .get("tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
            },
            Some(document) => Ended::Failed(
                document
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Coder stopped")
                    .to_owned(),
            ),
            None => Ended::Failed(match status {
                Ok(status) if !stderr.trim().is_empty() => {
                    format!("Coder V1 exited ({status}): {}", tail(&stderr, 600))
                }
                Ok(status) => format!("Coder V1 exited ({status}) without a result"),
                Err(why) => format!("Coder V1 did not report: {why}"),
            }),
        }
    }
}

/// A stand-in that plays recorded events, for tests and offline captures:
/// each [`Event::Approval`] is answered by the host, and a rejected one
/// skips the events up to the next approval or the end, as Coder would
/// not run the command.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    pub events: Vec<Event>,
    pub ended: Option<Ended>,
    /// The turns it was given.
    pub turns: Vec<Turn>,
    /// Keep working after the events until cancelled, as a long command
    /// does.
    pub hold: bool,
}

impl Engine for Scripted {
    fn turn(
        &mut self,
        turn: &Turn,
        cancel: &AtomicBool,
        hear: &mut dyn FnMut(&Event) -> Option<bool>,
    ) -> Ended {
        self.turns.push(turn.clone());
        let mut skipping = false;
        for event in &self.events {
            if cancel.load(Ordering::SeqCst) {
                return Ended::Cancelled;
            }
            match event {
                Event::Approval { id, .. } => {
                    let confirm = turn.approvals && hear(event) == Some(true);
                    let _ = hear(&Event::Answered { id: *id, confirm });
                    skipping = !confirm;
                }
                Event::Tool { running: false, .. } if skipping => skipping = false,
                _ if skipping => {}
                _ => {
                    let _ = hear(event);
                }
            }
        }
        while self.hold && !cancel.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if cancel.load(Ordering::SeqCst) {
            return Ended::Cancelled;
        }
        self.ended.clone().unwrap_or(Ended::Finished {
            reply: "Done.".into(),
            tokens: 0,
        })
    }
}

/// A stand-in that plays one recorded turn per prompt, in order, and
/// keeps every turn it was given where a test can read them. After the
/// last recorded turn it answers `Done.` with nothing run.
#[derive(Clone, Debug, Default)]
pub struct Sequence {
    pub turns: std::collections::VecDeque<Scripted>,
    pub given: std::sync::Arc<std::sync::Mutex<Vec<Turn>>>,
}

impl Sequence {
    #[must_use]
    pub fn new(turns: Vec<Scripted>) -> Self {
        Self {
            turns: turns.into(),
            given: std::sync::Arc::default(),
        }
    }
}

impl Engine for Sequence {
    fn turn(
        &mut self,
        turn: &Turn,
        cancel: &AtomicBool,
        hear: &mut dyn FnMut(&Event) -> Option<bool>,
    ) -> Ended {
        if let Ok(mut given) = self.given.lock() {
            given.push(turn.clone());
        }
        let mut next = self.turns.pop_front().unwrap_or_default();
        next.turn(turn, cancel, hear)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coder_lines_become_events_and_the_last_line_is_the_result() {
        let run = json!({"event":"entry","session":"s","index":2,
            "entry":{"source":"tool","name":"Run","input":"git status","output":null,"running":true}});
        assert_eq!(
            parse(&run.to_string()),
            Some(Ok(Event::Tool {
                name: "Run".into(),
                input: json!("git status"),
                output: Value::Null,
                running: true,
                delegation: None,
            }))
        );
        let ask = json!({"event":"approval","id":3,"command":"touch a","why":"it writes"});
        assert!(matches!(
            parse(&ask.to_string()),
            Some(Ok(Event::Approval { id: 3, .. }))
        ));
        let delegated = json!({"event":"entry","entry":{"source":"delegation","id":"s-delegate-1",
            "name":"Codex","task":"fix it","running":false,
            "output":{"reply":"done","tokens":18,"usage":{"input_tokens":11,"output_tokens":7}}}});
        assert!(matches!(
            parse(&delegated.to_string()),
            Some(Ok(Event::Delegation { ref agent, running: false, ref output, .. }))
                if agent == "Codex" && output["tokens"] == 18
        ));
        let user = json!({"event":"entry","entry":{"source":"user","text":"hi"}});
        assert_eq!(parse(&user.to_string()), None);
        let done = json!({"event":"finished","session":"s","reply":"ok","tokens":7});
        assert!(matches!(parse(&done.to_string()), Some(Err(_))));
        assert!(matches!(
            parse(&json!({"error":"no"}).to_string()),
            Some(Err(_))
        ));
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn an_agents_session_is_a_valid_coder_session_id() {
        assert_eq!(session_for("alice"), "agent-alice");
        assert_eq!(session_for("a b/c"), "agent-a-b-c");
        assert_eq!(coder_session_for("alice"), "alice-coder");
        assert_eq!(coder_session_for("a b/c"), "a-b-c-coder");
    }

    #[cfg(unix)]
    #[test]
    fn crew_turn_passes_the_machine_tool_free_boundary_to_the_native_child() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("openagents");
        let argv = dir.path().join("argv");
        std::fs::write(&program, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho '{{\"event\":\"finished\",\"reply\":\"draft\",\"tokens\":0}}'\n", argv.display())).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let turn = Turn {
            cwd: dir.path().into(),
            state: dir.path().join("state"),
            session: "paul-coder".into(),
            prompt: "Draft a recommendation.".into(),
            instructions: None,
            approvals: true,
            tool_free: true,
            codex_writes: true,
        };
        assert!(matches!(
            Cli {
                program,
                env: vec![]
            }
            .turn(&turn, &AtomicBool::new(false), &mut |_| None),
            Ended::Finished { .. }
        ));
        let arguments = std::fs::read_to_string(argv).unwrap();
        assert!(arguments.contains("--approvals\ntool-free\n"));
        assert!(!arguments.contains("--approvals\nstdin\n"));
        assert!(!arguments.contains("--codex-writes"));
    }

    #[cfg(unix)]
    #[test]
    fn the_cli_turn_streams_answers_approvals_and_reads_the_result() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("openagents");
        // A stand-in for `openagents coder chat --json --approvals stdin`.
        std::fs::write(
            &program,
            "#!/bin/sh\n\
             echo '{\"event\":\"entry\",\"entry\":{\"source\":\"tool\",\"name\":\"Run\",\"input\":\"ls\",\"output\":null,\"running\":true}}'\n\
             echo '{\"event\":\"approval\",\"id\":1,\"command\":\"touch a\",\"why\":\"it writes\"}'\n\
             read answer\n\
             echo \"{\\\"event\\\":\\\"finished\\\",\\\"reply\\\":\\\"$answer\\\",\\\"tokens\\\":3}\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let turn = Turn {
            cwd: dir.path().to_path_buf(),
            state: dir.path().join("state"),
            session: "agent-alice".into(),
            prompt: "hello".into(),
            instructions: None,
            approvals: true,
            codex_writes: false,
            tool_free: false,
        };
        let mut heard = Vec::new();
        let ended = Cli {
            program,
            env: Vec::new(),
        }
        .turn(&turn, &AtomicBool::new(false), &mut |event| {
            heard.push(event.clone());
            Some(true)
        });
        assert_eq!(
            ended,
            Ended::Finished {
                reply: "confirm 1".into(),
                tokens: 3
            }
        );
        assert_eq!(heard.len(), 2);
    }

    #[test]
    fn the_stand_in_skips_what_a_rejection_stops() {
        let mut scripted = Scripted {
            events: vec![
                Event::Approval {
                    id: 1,
                    command: "touch a".into(),
                    why: "w".into(),
                },
                Event::Tool {
                    name: "Run".into(),
                    input: json!("touch a"),
                    output: json!({"exit":0}),
                    running: false,
                    delegation: None,
                },
            ],
            ..Scripted::default()
        };
        let turn = Turn {
            cwd: PathBuf::from("/"),
            state: PathBuf::from("/"),
            session: "s".into(),
            prompt: "p".into(),
            instructions: None,
            approvals: true,
            codex_writes: false,
            tool_free: false,
        };
        let mut tools = 0;
        scripted.turn(&turn, &AtomicBool::new(false), &mut |event| {
            if matches!(event, Event::Tool { .. }) {
                tools += 1;
            }
            Some(false)
        });
        assert_eq!(tools, 0);
    }
}
