//! Background shells and monitors (#11177).
//!
//! `Run` with `background: true` starts a command and returns at once with
//! its id and output file. When the command ends, its notice (exit status,
//! time, and the last lines) joins the chat as the next input, the same
//! way a background agent's report does. A build, a test suite or a server
//! no longer holds the turn or meets `Run`'s two-minute limit.
//!
//! The `monitor` tool starts a command whose every stdout line is an
//! event: the lines that arrive between frames join the chat as one
//! notice. It ends when the command ends, when it is stopped, or at its
//! timeout. `background_shell` lists, reads and stops both kinds.
//!
//! Commands run with the same access as `Run`, without its time limit, in
//! their own process group so a stop ends the whole tree. Credentials are
//! taken out of their environment and out of everything the chat reads.
//! A host that gates commands behind approval ([`crate::approval`]) gets no
//! background commands, since nobody would be there to approve them.
//!
//! Jobs belong to the chat that started them: each chat's agent list
//! ([`crate::fleet::Fleet`]) has a key, and only that chat sees and
//! receives its jobs.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use model_access::ApiKey;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::plugin_tools::ExecutionSettings;
use crate::{App, Mode, live};

/// The tool that lists, reads and stops background commands.
pub const SHELL_TOOL: &str = "background_shell";
/// The tool that turns a command's output lines into chat events.
pub const MONITOR_TOOL: &str = "monitor";
/// A monitor's timeout when the call names none.
const DEFAULT_MONITOR_SECS: u64 = 30 * 60;
/// The longest a monitor runs.
const MAX_MONITOR_SECS: u64 = 24 * 60 * 60;
/// One notice's share of a monitor's lines; the rest are in its file.
const NOTICE_LINES: usize = 200;
/// The end of the output a finished command's notice shows.
const TAIL_BYTES: usize = 4 * 1024;
/// The most `background_shell output` returns.
const OUTPUT_BYTES: usize = 64 * 1024;

/// What the model reads about background commands.
pub const INSTRUCTIONS: &str = "Background commands: Run with background true starts a long command (a build, a test suite, a server) and returns at once with its id; its exit status and last lines arrive later as a message that starts with \"Background command\". Keep working or end your turn meanwhile; never sleep or poll in a loop for it. The monitor tool runs a command whose every output line is an event, such as `tail -f build.log | grep --line-buffered -E 'error|warning'`; matching lines arrive as messages that start with \"Monitor\". Use background_shell to list, read the output of, or stop them.\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Shell,
    Monitor,
}

#[derive(Clone)]
struct Job {
    id: String,
    owner: usize,
    kind: Kind,
    /// The command, redacted.
    command: String,
    output: PathBuf,
    pid: u32,
    started_ms: u64,
    ended_ms: Option<u64>,
    exit: Option<i32>,
    /// Why it ended when not by itself: `stopped` or `timed out`.
    ended_by: Option<&'static str>,
    lines: u64,
    keys: Vec<ApiKey>,
}

impl Job {
    fn status(&self) -> String {
        match (self.ended_ms, self.ended_by, self.exit) {
            (None, Some(by), _) => format!("{by}, ending"),
            (None, None, _) => "running".into(),
            (Some(_), Some(by), _) => by.into(),
            (Some(_), None, Some(0)) => "finished (exit 0)".into(),
            (Some(_), None, Some(code)) => format!("failed (exit {code})"),
            (Some(_), None, None) => "ended by a signal".into(),
        }
    }

    fn seconds(&self) -> u64 {
        self.ended_ms
            .unwrap_or_else(atif::now_ms)
            .saturating_sub(self.started_ms)
            / 1000
    }

    fn row(&self) -> Value {
        json!({
            "id": self.id,
            "kind": match self.kind { Kind::Shell => "command", Kind::Monitor => "monitor" },
            "command": self.command,
            "status": self.status(),
            "exit": self.exit,
            "seconds": self.seconds(),
            "lines": (self.kind == Kind::Monitor).then_some(self.lines),
            "output_file": self.output,
        })
    }
}

#[derive(Default)]
struct Table {
    jobs: Vec<Job>,
    /// Finished jobs' notices, by owner.
    notices: Vec<(usize, String)>,
    /// Monitor lines not yet handed to their chat: owner, job, line.
    lines: Vec<(usize, String, String)>,
    next: u64,
}

fn table() -> MutexGuard<'static, Table> {
    static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();
    TABLE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The chat a tool call reports to: its agent list's key.
fn owner(execution: &ExecutionSettings) -> Option<usize> {
    execution.fleet.as_ref().map(|host| host.fleet.key())
}

/// Whether this chat offers background commands.
#[must_use]
pub fn available(execution: &ExecutionSettings) -> bool {
    cfg!(unix) && execution.shell && execution.fleet.is_some() && !crate::approval::gated()
}

#[must_use]
pub fn is_tool(name: &str) -> bool {
    name == SHELL_TOOL || name == MONITOR_TOOL
}

/// Adds `background` to the `Run` tool's definition.
pub fn extend_run(definition: &mut Value) {
    let Some(function) = definition.get_mut("function") else {
        return;
    };
    if let Some(description) = function["description"].as_str() {
        function["description"] = json!(format!(
            "{description} With background true it starts the command and returns at once with its id and output file; a notice with its exit status and last lines arrives as a new message when it ends. Use it for builds, test suites, servers, and anything that may take over two minutes."
        ));
    }
    if let Some(properties) = function
        .get_mut("parameters")
        .and_then(|parameters| parameters.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        properties.insert(
            "background".into(),
            json!({"type":"boolean","description":"Start it in the background and return at once."}),
        );
    }
}

/// `monitor` and `background_shell`, when the chat offers them.
#[must_use]
pub fn tool_definitions(execution: &ExecutionSettings) -> Vec<Value> {
    if !available(execution) {
        return Vec::new();
    }
    vec![
        json!({"type":"function","function":{
            "name": MONITOR_TOOL,
            "description":"Start a command in the background whose every stdout line is an event, and keep working. New lines arrive as a message that starts with \"Monitor\". Filter to the lines that matter, with line buffering, for example `tail -n0 -F app.log | grep --line-buffered ERROR`. It ends when the command ends, when stopped with background_shell, or at its timeout. Stderr goes only to its output file.",
            "parameters":{"type":"object","properties":{
                "command":{"type":"string","minLength":1,"maxLength":8192},
                "timeout_seconds":{"type":"integer","minimum":1,"maximum":MAX_MONITOR_SECS,"description":"Stop it after this long (default 1800)."}
            },"required":["command"],"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name": SHELL_TOOL,
            "description":"Background commands and monitors this chat started: list them, read the end of one's output, or stop one with everything it started.",
            "parameters":{"type":"object","properties":{
                "operation":{"type":"string","enum":["list","output","stop"]},
                "id":{"type":"string","maxLength":32,"description":"The command's id, for output and stop."},
                "tail_bytes":{"type":"integer","minimum":1,"maximum":OUTPUT_BYTES,"description":"How much of the output's end to read (default 16384)."}
            },"required":["operation"],"additionalProperties":false}
        }}),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MonitorArguments {
    command: String,
    #[serde(default)]
    timeout_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellArguments {
    operation: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    tail_bytes: Option<usize>,
}

fn keys(execution: &ExecutionSettings) -> Vec<ApiKey> {
    execution
        .redaction_keys
        .iter()
        .chain(execution.jev_key.iter())
        .cloned()
        .collect()
}

/// `Run` with `background: true`.
///
/// # Errors
/// Background commands are off here, or the command could not start.
pub fn start_shell(execution: &ExecutionSettings, command: &str) -> Result<Value, String> {
    if !available(execution) {
        return Err(
            "Background commands are off in this chat; run the command without background.".into(),
        );
    }
    let owner = owner(execution).ok_or("Background commands need a chat to report to.")?;
    start(
        owner,
        Kind::Shell,
        command,
        &execution.cwd,
        keys(execution),
        None,
    )
}

/// `monitor` and `background_shell`.
///
/// # Errors
/// Invalid arguments, an unknown id, or a command that could not start.
pub fn execute(
    execution: &ExecutionSettings,
    name: &str,
    arguments: Value,
) -> Result<Value, String> {
    if !available(execution) {
        return Err("Background commands are off in this chat.".into());
    }
    let owner = owner(execution).ok_or("Background commands need a chat to report to.")?;
    if name == MONITOR_TOOL {
        let args: MonitorArguments = serde_json::from_value(arguments)
            .map_err(|_| "monitor takes a command and an optional timeout_seconds.")?;
        let timeout = args
            .timeout_seconds
            .unwrap_or(DEFAULT_MONITOR_SECS)
            .clamp(1, MAX_MONITOR_SECS);
        return start(
            owner,
            Kind::Monitor,
            &args.command,
            &execution.cwd,
            keys(execution),
            Some(Duration::from_secs(timeout)),
        );
    }
    let args: ShellArguments = serde_json::from_value(arguments)
        .map_err(|_| "background_shell takes an operation, and an id for output and stop.")?;
    let id = || {
        args.id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| format!("{} needs the command's id.", args.operation))
    };
    match args.operation.as_str() {
        "list" => Ok(list(owner)),
        "output" => output(owner, id()?, args.tail_bytes.unwrap_or(16 * 1024)),
        "stop" => stop(owner, id()?),
        _ => Err("The operation is list, output or stop.".into()),
    }
}

fn output_dir() -> PathBuf {
    if !cfg!(test)
        && let Some(dir) = model_access::store::openagents_dir()
    {
        return dir.join("coder-new/shells");
    }
    std::env::temp_dir().join("openagents-coder-shells")
}

fn redact(text: &str, keys: &[ApiKey]) -> String {
    let mut text = text.to_owned();
    for key in keys {
        if !key.expose().is_empty() {
            text = text.replace(key.expose(), "[redacted]");
        }
    }
    text
}

#[cfg(not(unix))]
fn start(
    _owner: usize,
    _kind: Kind,
    _command: &str,
    _cwd: &Path,
    _keys: Vec<ApiKey>,
    _timeout: Option<Duration>,
) -> Result<Value, String> {
    Err("Background commands run on macOS and Linux only.".into())
}

#[cfg(unix)]
fn start(
    owner: usize,
    kind: Kind,
    command: &str,
    cwd: &Path,
    keys: Vec<ApiKey>,
    timeout: Option<Duration>,
) -> Result<Value, String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    if command.trim().is_empty() || command.len() > 8192 || command.contains('\0') {
        return Err("A background command is 1 to 8192 bytes.".into());
    }
    if crate::approval::gated() {
        return Err("This host asks before commands, so it runs none in the background.".into());
    }
    let cwd = cwd
        .canonicalize()
        .map_err(|_| "The working directory is unavailable.".to_string())?;
    let id = {
        let mut table = table();
        table.next += 1;
        format!(
            "{}{}",
            if kind == Kind::Monitor { "m" } else { "b" },
            table.next
        )
    };
    let dir = output_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("The output folder could not be made: {error}"))?;
    let output = dir.join(format!(
        "{}-{}-{id}.log",
        std::process::id(),
        atif::now_ms()
    ));
    let log = std::fs::File::create(&output)
        .map_err(|error| format!("The output file could not be made: {error}"))?;
    let mut process = std::process::Command::new("/bin/sh");
    process
        .arg("-c")
        .arg(command)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .process_group(0);
    match kind {
        Kind::Shell => {
            let clone = log
                .try_clone()
                .map_err(|error| format!("The output file could not be opened: {error}"))?;
            process.stdout(clone).stderr(log);
        }
        Kind::Monitor => {
            process.stdout(Stdio::piped()).stderr(log);
        }
    }
    crate::bundled_runtime::scrub_credentials(&mut process);
    let mut child = process
        .spawn()
        .map_err(|error| format!("The command could not start: {error}"))?;
    let pid = child.id();
    let shown = redact(command, &keys);
    table().jobs.push(Job {
        id: id.clone(),
        owner,
        kind,
        command: shown.clone(),
        output: output.clone(),
        pid,
        started_ms: atif::now_ms(),
        ended_ms: None,
        exit: None,
        ended_by: None,
        lines: 0,
        keys: keys.clone(),
    });
    match kind {
        Kind::Shell => {
            let id = id.clone();
            std::thread::spawn(move || {
                let exit = child.wait().ok().and_then(|status| status.code());
                finish(&id, exit);
            });
        }
        Kind::Monitor => {
            let stdout = child.stdout.take();
            let id_for_reader = id.clone();
            let path = output.clone();
            std::thread::spawn(move || {
                if let Some(stdout) = stdout {
                    let mut log = std::fs::OpenOptions::new().append(true).open(&path).ok();
                    for line in BufReader::new(stdout).lines() {
                        let Ok(line) = line else { break };
                        let line = redact(&line, &keys);
                        if let Some(log) = log.as_mut() {
                            let _ = writeln!(log, "{line}");
                        }
                        let mut table = table();
                        if let Some(job) = table.jobs.iter_mut().find(|job| job.id == id_for_reader)
                        {
                            job.lines += 1;
                        }
                        table.lines.push((owner, id_for_reader.clone(), line));
                    }
                }
                let exit = child.wait().ok().and_then(|status| status.code());
                finish(&id_for_reader, exit);
            });
            if let Some(timeout) = timeout {
                let id = id.clone();
                std::thread::spawn(move || {
                    let deadline = Instant::now() + timeout;
                    while Instant::now() < deadline {
                        if ended(&id) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    end(&id, "timed out");
                });
            }
        }
    }
    let status = match kind {
        Kind::Shell => format!("Started in the background as {id}. A notice arrives when it ends."),
        Kind::Monitor => format!("Monitor {id} started. Its lines arrive as messages."),
    };
    Ok(json!({
        "background": true,
        "id": id,
        "pid": pid,
        "output_file": output,
        "status": status,
    }))
}

fn ended(id: &str) -> bool {
    table()
        .jobs
        .iter()
        .find(|job| job.id == id)
        .is_none_or(|job| job.ended_ms.is_some())
}

/// Ends a running job's process group, saying why; a group that ignores
/// the request is killed a few seconds later.
fn end(id: &str, why: &'static str) -> Option<Job> {
    let job = {
        let mut table = table();
        let job = table
            .jobs
            .iter_mut()
            .find(|job| job.id == id && job.ended_ms.is_none())?;
        job.ended_by.get_or_insert(why);
        job.clone()
    };
    signal(job.pid, "-TERM");
    let id = id.to_owned();
    let pid = job.pid;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        if !ended(&id) {
            signal(pid, "-KILL");
        }
    });
    Some(job)
}

fn signal(pid: u32, which: &str) {
    if pid == 0 {
        return;
    }
    let _ = std::process::Command::new("kill")
        .args([which, "--", &format!("-{pid}")])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// The end of `path`, at most `bytes`, redacted.
fn tail(path: &Path, bytes: usize, keys: &[ApiKey]) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let start = length.saturating_sub(bytes as u64);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buffer = Vec::new();
    let _ = file.take(bytes as u64).read_to_end(&mut buffer);
    let text = String::from_utf8_lossy(&buffer);
    // Start at a whole line when the read began mid-file.
    let text: &str = if start > 0 {
        text.split_once('\n').map_or(&*text, |(_, rest)| rest)
    } else {
        &text
    };
    redact(text.trim_end(), keys)
}

fn finish(id: &str, exit: Option<i32>) {
    let job = {
        let mut table = table();
        let Some(job) = table.jobs.iter_mut().find(|job| job.id == id) else {
            return;
        };
        job.ended_ms = Some(atif::now_ms());
        job.exit = exit;
        job.clone()
    };
    let last = tail(&job.output, TAIL_BYTES, &job.keys);
    let text = match job.kind {
        Kind::Shell => {
            let mut text = format!(
                "Background command {} {} after {}: `{}`. Full output: {}",
                job.id,
                job.status(),
                span(job.seconds()),
                job.command,
                job.output.display()
            );
            if last.is_empty() {
                text.push_str("\nIt printed nothing.");
            } else {
                text.push_str(&format!("\nLast output:\n```\n{last}\n```"));
            }
            text
        }
        Kind::Monitor => format!(
            "Monitor {} ended ({}) after {} with {} lines: `{}`. Full output: {}",
            job.id,
            job.status(),
            span(job.seconds()),
            job.lines,
            job.command,
            job.output.display()
        ),
    };
    table().notices.push((job.owner, text));
}

/// A span of seconds in words: `45s`, `3m 12s`, `1h 4m`.
fn span(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
    }
}

fn list(owner: usize) -> Value {
    let jobs: Vec<Value> = table()
        .jobs
        .iter()
        .filter(|job| job.owner == owner)
        .map(Job::row)
        .collect();
    json!({ "commands": jobs })
}

fn find(owner: usize, id: &str) -> Result<Job, String> {
    table()
        .jobs
        .iter()
        .find(|job| job.owner == owner && job.id == id)
        .cloned()
        .ok_or_else(|| format!("No background command {id} in this chat."))
}

fn output(owner: usize, id: &str, bytes: usize) -> Result<Value, String> {
    let job = find(owner, id)?;
    let mut row = job.row();
    row["output"] = json!(tail(&job.output, bytes.clamp(1, OUTPUT_BYTES), &job.keys));
    Ok(row)
}

fn stop(owner: usize, id: &str) -> Result<Value, String> {
    let job = find(owner, id)?;
    if job.ended_ms.is_some() {
        return Ok(json!({"id": job.id, "status": format!("Already {}.", job.status())}));
    }
    end(id, "stopped");
    Ok(json!({"id": job.id, "status": "Stopping it and its child processes."}))
}

/// What `owner`'s jobs said since the last call: monitor lines, grouped
/// by monitor, then finished jobs' notices.
fn drain(owner: usize) -> Vec<String> {
    let mut table = table();
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    let mut kept = Vec::new();
    for (from, id, line) in std::mem::take(&mut table.lines) {
        if from != owner {
            kept.push((from, id, line));
            continue;
        }
        match grouped.iter_mut().find(|(seen, _)| *seen == id) {
            Some((_, lines)) => lines.push(line),
            None => grouped.push((id, vec![line])),
        }
    }
    table.lines = kept;
    let mut texts = Vec::new();
    for (id, lines) in grouped {
        let (command, path) = table
            .jobs
            .iter()
            .find(|job| job.id == id)
            .map(|job| (job.command.clone(), job.output.display().to_string()))
            .unwrap_or_default();
        let mut text = format!("Monitor {id} (`{command}`):\n");
        for line in lines.iter().take(NOTICE_LINES) {
            text.push_str(line);
            text.push('\n');
        }
        if lines.len() > NOTICE_LINES {
            text.push_str(&format!(
                "… {} more lines in {path}\n",
                lines.len() - NOTICE_LINES
            ));
        }
        texts.push(text.trim_end().to_owned());
    }
    let mut notices = Vec::new();
    table.notices.retain(|(from, text)| {
        if *from == owner {
            notices.push(text.clone());
            false
        } else {
            true
        }
    });
    texts.extend(notices);
    texts
}

impl App {
    /// Hands this chat's background command lines and notices to it: the
    /// model reads them as its next input, like a background agent's.
    pub(crate) fn poll_shells(&mut self) {
        for text in drain(self.fleet.key()) {
            if self.mode == Mode::Live && self.plugins.enabled && self.plugins.key_configured {
                self.queue_notice(text);
            } else {
                self.live.entries.push(live::Entry::User(text));
                self.scroll_main_to_end();
            }
            self.history.dirty = true;
        }
    }

    /// `/shells` lists this chat's background commands; `/shells stop ID`
    /// stops one.
    pub(crate) fn shells_command_with(&mut self, argument: &str) {
        let owner = self.fleet.key();
        let argument = argument.trim();
        if let Some(id) = argument.strip_prefix("stop").map(str::trim) {
            self.notice = Some(if id.is_empty() {
                "Use /shells stop ID; /shells lists them.".into()
            } else {
                match stop(owner, id) {
                    Ok(result) => format!("{id}: {}", result["status"].as_str().unwrap_or("")),
                    Err(error) => error,
                }
            });
            return;
        }
        let jobs: Vec<Job> = table()
            .jobs
            .iter()
            .filter(|job| job.owner == owner)
            .cloned()
            .collect();
        self.notice = Some(if jobs.is_empty() {
            "No background commands in this chat. The model starts them with Run in the background or monitor.".into()
        } else {
            let mut text = String::from("Background commands:\n");
            for job in jobs {
                text.push_str(&format!(
                    "{}  {}  {}  {}\n",
                    job.id,
                    job.status(),
                    span(job.seconds()),
                    crate::long_session::clip(&job.command, 80)
                ));
            }
            text.push_str("/shells stop ID stops one.");
            text
        });
    }

    pub(crate) fn shells_command(&mut self) {
        self.shells_command_with("");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// A fresh owner number for each test, so parallel tests never read
    /// each other's notices.
    fn fresh_owner() -> usize {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        usize::MAX - NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    fn wait_for(owner: usize, done: impl Fn(&[String]) -> bool) -> Vec<String> {
        let started = Instant::now();
        let mut seen = Vec::new();
        while started.elapsed() < Duration::from_secs(20) {
            seen.extend(drain(owner));
            if done(&seen) {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out; saw {seen:?}");
    }

    #[test]
    fn a_background_command_notifies_its_chat_when_done() {
        let owner = fresh_owner();
        let root = tempfile::tempdir().unwrap();
        let started = start(
            owner,
            Kind::Shell,
            "printf 'one\\ntwo\\n'; printf 'secret-fixture-key\\n'; exit 3",
            root.path(),
            vec![ApiKey::new("secret-fixture-key")],
            None,
        )
        .unwrap();
        let id = started["id"].as_str().unwrap().to_owned();
        assert!(started["status"].as_str().unwrap().contains("background"));
        let seen = wait_for(owner, |seen| !seen.is_empty());
        assert_eq!(seen.len(), 1);
        let notice = &seen[0];
        assert!(
            notice.starts_with(&format!("Background command {id} failed (exit 3)")),
            "{notice}"
        );
        assert!(notice.contains("two"));
        assert!(!notice.contains("secret-fixture-key"));
        assert!(notice.contains("[redacted]"));
        // Another chat never sees it.
        assert!(drain(fresh_owner()).is_empty());
        assert_eq!(list(owner)["commands"][0]["status"], "failed (exit 3)");
        assert!(
            output(owner, &id, 1024).unwrap()["output"]
                .as_str()
                .unwrap()
                .contains("one")
        );
    }

    #[test]
    fn a_monitor_emits_each_line_and_ends() {
        let owner = fresh_owner();
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("app.log"),
            "ok\nERROR disk full\nok\nERROR again\n",
        )
        .unwrap();
        start(
            owner,
            Kind::Monitor,
            "grep --line-buffered ERROR app.log",
            root.path(),
            Vec::new(),
            Some(Duration::from_secs(60)),
        )
        .unwrap();
        let seen = wait_for(owner, |seen| {
            seen.iter().any(|text| text.contains(" ended ("))
        });
        let lines: String = seen
            .iter()
            .filter(|text| !text.contains(" ended ("))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        assert!(lines.starts_with("Monitor m"));
        assert!(lines.contains("ERROR disk full"));
        assert!(lines.contains("ERROR again"));
        assert!(!lines.contains("\nok"));
        assert!(seen.last().unwrap().contains("with 2 lines"));
    }

    #[test]
    fn a_monitor_stops_at_its_timeout_and_a_stop_ends_a_command() {
        let owner = fresh_owner();
        let root = tempfile::tempdir().unwrap();
        start(
            owner,
            Kind::Monitor,
            "sleep 30",
            root.path(),
            Vec::new(),
            Some(Duration::from_secs(1)),
        )
        .unwrap();
        let seen = wait_for(owner, |seen| !seen.is_empty());
        assert!(seen[0].contains("(timed out)"), "{seen:?}");
        let started = start(
            owner,
            Kind::Shell,
            "sleep 30",
            root.path(),
            Vec::new(),
            None,
        )
        .unwrap();
        let id = started["id"].as_str().unwrap();
        stop(owner, id).unwrap();
        let seen = wait_for(owner, |seen| !seen.is_empty());
        assert!(
            seen[0].contains(&format!("Background command {id} stopped")),
            "{seen:?}"
        );
    }

    #[test]
    fn the_chat_receives_its_notices() {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        let root = tempfile::tempdir().unwrap();
        start(
            app.fleet.key(),
            Kind::Shell,
            "echo built",
            root.path(),
            Vec::new(),
            None,
        )
        .unwrap();
        let started = Instant::now();
        while app.live.entries.is_empty() && started.elapsed() < Duration::from_secs(20) {
            app.poll_shells();
            std::thread::sleep(Duration::from_millis(20));
        }
        // Without a model key the notice is shown, not sent.
        assert!(
            matches!(&app.live.entries[0], live::Entry::User(text) if text.contains("finished (exit 0)") && text.contains("built"))
        );
    }

    #[test]
    fn the_run_definition_gains_background() {
        let mut run = crate::bundled_runtime::run_tool_definition();
        extend_run(&mut run);
        assert_eq!(
            run["function"]["parameters"]["properties"]["background"]["type"],
            "boolean"
        );
        assert_eq!(span(45), "45s");
        assert_eq!(span(192), "3m 12s");
        assert_eq!(span(3840), "1h 4m");
    }
}
