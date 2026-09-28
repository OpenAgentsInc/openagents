//! The offline measurement for `evidence.environment` (issue #9632): on
//! retained session logs, how many commands before a session's first edit
//! failed because a program or a file wasn't there, and how many of those
//! the environment line would have prevented.
//!
//! Two kinds of log hold sessions:
//!
//! - A Microluna session log (`microluna-<dispatch>-<session>.atif.jsonl`):
//!   every `run_command` call with its output, and `apply_patch` and
//!   `write_file` calls, the edits.
//! - A Coder One episode log (`episode.atif.jsonl`): the normalized
//!   executor events of each session it dispatched, Claude Code, Codex, or
//!   Microluna: `command_completed` with its exit code and output, and
//!   `artifact_changed`, the edits.
//!
//! A Microluna session appears in both. It is read from its own log when
//! that log was retained beside the episode log, and from the episode's
//! events otherwise. Copies of one log in several directories count once.
//!
//! A command counts as a miss when a line of its output is a shell's
//! `not found` or a `No such file or directory` diagnostic, whatever its
//! exit code: a command such as `python …; true` exits 0 and still missed.
//! The line would have prevented a miss when the miss names a program the
//! presence probes ask about ([`super::FIXED`] and the programs the
//! session's files imply). The error itself shows the program was absent
//! from that container, so the line would have listed it as absent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The schema of a measurement's summary.
pub const SCHEMA: &str = "openagents.coder-one.environment-offline.v1";

/// What a miss's diagnostic says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A shell's `not found` or `command not found`.
    NotFound,
    /// `No such file or directory`, or `No such file`.
    NoSuchFile,
}

/// One diagnostic line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Miss {
    pub kind: Kind,
    /// The program it names, when it names one: the command a shell
    /// couldn't find, the program `env` couldn't start, or the executable
    /// path a shell couldn't open, by its file name.
    pub program: Option<String>,
    pub line: String,
}

const NO_SUCH: [&str; 2] = ["No such file or directory", "No such file"];
const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "dash", "ash"];

fn unquote(word: &str) -> String {
    word.trim()
        .trim_matches(|c| "'\"‘’`".contains(c))
        .to_string()
}

fn base(word: &str) -> String {
    let word = unquote(word);
    word.rsplit('/').next().unwrap_or(&word).to_string()
}

/// Whether `word` could be a program name: no spaces, and not empty.
fn plausible(word: &str) -> bool {
    !word.is_empty() && word.len() <= 64 && !word.contains(char::is_whitespace)
}

/// The diagnostics in `output`, one per line that is one.
#[must_use]
pub fn misses(output: &str) -> Vec<Miss> {
    let mut out = Vec::new();
    for raw in output.lines() {
        let line = raw.trim();
        if line.is_empty() || line.len() > 400 {
            continue;
        }
        let parts: Vec<&str> = line.split(": ").collect();
        let last = parts.last().copied().unwrap_or("").trim();
        // `sh: 1: python: not found`, `bash: python: command not found`.
        if parts.len() >= 2 && (last == "not found" || last == "command not found") {
            let program = unquote(parts[parts.len() - 2]);
            if plausible(&program) {
                out.push(Miss {
                    kind: Kind::NotFound,
                    program: Some(program),
                    line: line.to_string(),
                });
                continue;
            }
        }
        // `zsh: command not found: python`.
        if let Some(at) = parts.iter().position(|p| p.trim() == "command not found")
            && let Some(program) = parts.get(at + 1)
        {
            let program = unquote(program);
            if plausible(&program) {
                out.push(Miss {
                    kind: Kind::NotFound,
                    program: Some(program),
                    line: line.to_string(),
                });
                continue;
            }
        }
        if !NO_SUCH.iter().any(|text| line.contains(text)) {
            continue;
        }
        // `/usr/bin/env: 'python': No such file or directory`, and a shell
        // that couldn't open an executable path.
        let program = if parts.len() >= 3 && NO_SUCH.contains(&last) {
            let head = base(parts[0]);
            let named = parts[parts.len() - 2];
            if head == "env" || SHELLS.contains(&head.as_str()) {
                Some(base(named)).filter(|p| plausible(p))
            } else {
                None
            }
        } else {
            None
        };
        out.push(Miss {
            kind: Kind::NoSuchFile,
            program,
            line: line.to_string(),
        });
    }
    out
}

/// One command a session ran before its first edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub text: String,
    pub exit: Option<i32>,
    pub output: String,
}

/// One session, read from a log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// What identifies the session across copies of its log.
    pub key: String,
    /// `microluna`, `claude-code`, or `codex`.
    pub agent: String,
    /// The Terminal-Bench task, from the log's path.
    pub task: String,
    /// The log it was read from.
    pub source: String,
    /// The commands before the first edit, in order.
    pub commands: Vec<Command>,
    /// Whether the session edited anything.
    pub edited: bool,
    /// Every command the session ran.
    pub total_commands: usize,
    /// Text the host would have seen before the session: the task and the
    /// file names the session's first message or commands carry. Used for
    /// the implied programs only.
    #[serde(skip)]
    pub context: String,
}

/// The task a log's path names: the part before `__` of the trial
/// directory, such as `embedding-drift-monitor` in
/// `embedding-drift-monitor__6zRjd9n.episode`.
#[must_use]
pub fn task_of(path: &Path) -> String {
    path.ancestors()
        .filter_map(|dir| dir.file_name()?.to_str())
        .find_map(|name| name.split_once("__").map(|(task, _)| task.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn records(text: &str) -> impl Iterator<Item = Value> + '_ {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
}

/// When a log's session record says it opened, in milliseconds.
fn opened_at(text: &str) -> Option<i64> {
    records(text)
        .find(|r| r["record"] == "session")
        .and_then(|r| r["at"].as_i64())
}

/// The session in a Microluna session log. `episode_at` is when its
/// episode's log opened, when that log was retained beside it.
#[must_use]
pub fn microluna_session(text: &str, path: &Path, episode_at: Option<i64>) -> Option<Session> {
    let header = records(text).find(|r| r["record"] == "session")?;
    let id = header["session"]["id"].as_str()?.to_string();
    let at = header["at"].as_i64().unwrap_or(0);
    let mut session = Session {
        key: match episode_at {
            Some(episode) => format!("microluna:{episode}:{id}"),
            None => format!("microluna-log:{at}:{id}"),
        },
        agent: "microluna".to_string(),
        task: task_of(path),
        source: path.display().to_string(),
        commands: Vec::new(),
        edited: false,
        total_commands: 0,
        context: String::new(),
    };
    for record in records(text) {
        let step = &record["step"];
        if step["source"] == "User"
            && session.context.is_empty()
            && let Some(message) = step["message"].as_str()
        {
            session.context = message.to_string();
        }
        let call = &step["call"];
        match call["name"].as_str() {
            Some("run_command") => {
                session.total_commands += 1;
                if !session.edited {
                    let output = call["output"].as_str().unwrap_or("").to_string();
                    session.commands.push(Command {
                        text: call["arguments"]["command"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        exit: call["extra"]["exit"]
                            .as_i64()
                            .and_then(|e| i32::try_from(e).ok())
                            .or_else(|| exit_marker(&output)),
                        output,
                    });
                }
            }
            Some("apply_patch" | "write_file") if call["outcome"] != "Failed" => {
                session.edited = true;
            }
            _ => {}
        }
    }
    Some(session)
}

/// The `[exit N]` marker a Microluna command output opens with.
fn exit_marker(output: &str) -> Option<i32> {
    output
        .strip_prefix("[exit ")?
        .split(']')
        .next()?
        .parse()
        .ok()
}

/// The executor sessions in a Coder One episode log, from their normalized
/// events. Microluna sessions whose key is in `skip` are left out: their
/// own log was read instead.
#[must_use]
pub fn executor_sessions(text: &str, path: &Path, skip: &BTreeSet<String>) -> Vec<Session> {
    let episode_at = opened_at(text).unwrap_or(0);
    let mut sessions: BTreeMap<(String, String, i64), Session> = BTreeMap::new();
    let mut started: BTreeMap<(String, String, i64), Vec<String>> = BTreeMap::new();
    let mut order: Vec<(String, String, i64)> = Vec::new();
    for record in records(text) {
        let event = &record["step"]["extensions"]["executor_event"];
        if event.is_null() {
            continue;
        }
        let adapter = event["adapter"].as_str().unwrap_or("").to_string();
        let session_id = event["session_id"].as_str().unwrap_or("").to_string();
        let generation = event["generation"].as_i64().unwrap_or(0);
        let key = if adapter == "microluna" {
            format!("microluna:{episode_at}:{session_id}")
        } else {
            format!("{adapter}:{episode_at}:{session_id}:{generation}")
        };
        if skip.contains(&key) {
            continue;
        }
        let id = (adapter.clone(), session_id, generation);
        let session = sessions.entry(id.clone()).or_insert_with(|| {
            order.push(id.clone());
            Session {
                key,
                agent: adapter.clone(),
                task: task_of(path),
                source: path.display().to_string(),
                commands: Vec::new(),
                edited: false,
                total_commands: 0,
                context: String::new(),
            }
        });
        let inner = &event["event"];
        match inner["kind"].as_str() {
            Some("command_started") => {
                let command = inner["command"].as_str().unwrap_or("").to_string();
                if !session.edited {
                    session.context.push_str(&command);
                    session.context.push('\n');
                }
                started.entry(id).or_default().push(command);
            }
            Some("command_completed") => {
                session.total_commands += 1;
                let queue = started.entry(id).or_default();
                let named = inner["command"].as_str().unwrap_or("");
                // Claude Code names the tool call here, not the command.
                let text = if let Some(at) = queue.iter().position(|c| c == named) {
                    queue.remove(at)
                } else if queue.is_empty() {
                    named.to_string()
                } else {
                    queue.remove(0)
                };
                if !session.edited {
                    let output = inner["output"].as_str().unwrap_or("").to_string();
                    session.context.push_str(&output);
                    session.context.push('\n');
                    session.commands.push(Command {
                        text,
                        exit: inner["exit_code"]
                            .as_i64()
                            .and_then(|e| i32::try_from(e).ok()),
                        output,
                    });
                }
            }
            Some("artifact_changed") => session.edited = true,
            _ => {}
        }
    }
    order
        .into_iter()
        .filter_map(|id| sessions.remove(&id))
        .collect()
}

/// What one session's misses were.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub key: String,
    pub agent: String,
    pub task: String,
    pub source: String,
    pub edited: bool,
    pub commands_before_edit: usize,
    /// Commands before the first edit with a `not found` diagnostic.
    pub not_found: usize,
    /// Commands before the first edit with only a `No such file`
    /// diagnostic.
    pub no_such_file: usize,
    /// Of those, the ones that exited non-zero.
    pub nonzero: usize,
    /// Misses that name a program the presence probes ask about.
    pub prevented: usize,
    /// Of those, the ones the fixed set alone covers.
    pub prevented_fixed: usize,
    /// The programs the misses named, with counts.
    pub programs: BTreeMap<String, usize>,
    /// The programs the session's files imply.
    pub implied: Vec<String>,
    /// Each miss's command, clipped, and its diagnostic.
    pub examples: Vec<Value>,
}

/// Counts one session's misses before its first edit.
#[must_use]
pub fn count(session: &Session) -> Row {
    let implied = super::implied_by(
        super::named_files(&session.context)
            .iter()
            .map(String::as_str),
    );
    let probed: BTreeSet<String> = super::programs(&implied).into_iter().collect();
    let mut row = Row {
        key: session.key.clone(),
        agent: session.agent.clone(),
        task: session.task.clone(),
        source: session.source.clone(),
        edited: session.edited,
        commands_before_edit: session.commands.len(),
        not_found: 0,
        no_such_file: 0,
        nonzero: 0,
        prevented: 0,
        prevented_fixed: 0,
        programs: BTreeMap::new(),
        implied,
        examples: Vec::new(),
    };
    for command in &session.commands {
        let found = misses(&command.output);
        if found.is_empty() {
            continue;
        }
        if found.iter().any(|m| m.kind == Kind::NotFound) {
            row.not_found += 1;
        } else {
            row.no_such_file += 1;
        }
        if command.exit.is_some_and(|code| code != 0) {
            row.nonzero += 1;
        }
        let named: BTreeSet<String> = found.iter().filter_map(|m| m.program.clone()).collect();
        for program in &named {
            *row.programs.entry(program.clone()).or_default() += 1;
        }
        if named.iter().any(|p| probed.contains(p)) {
            row.prevented += 1;
        }
        if named.iter().any(|p| super::FIXED.contains(&p.as_str())) {
            row.prevented_fixed += 1;
        }
        row.examples.push(json!({
            "command": crate::judge::clip(&command.text, 160),
            "exit": command.exit,
            "diagnostic": found[0].line,
        }));
    }
    row
}

/// The logs under `roots`: episode logs and Microluna session logs, root
/// by root in the order given and sorted within each root, so a log
/// copied under two roots is attributed to the first.
#[must_use]
pub fn logs(roots: &[PathBuf]) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut episodes = Vec::new();
    let mut micro = Vec::new();
    for root in roots {
        let (mut here_episodes, mut here_micro) = (Vec::new(), Vec::new());
        let mut pending = vec![root.clone()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let name = entry.file_name().to_string_lossy().into_owned();
                if kind.is_dir() {
                    if !matches!(name.as_str(), ".git" | "target" | "node_modules") {
                        pending.push(entry.path());
                    }
                } else if kind.is_file() {
                    if name == "episode.atif.jsonl" {
                        here_episodes.push(entry.path());
                    } else if name.starts_with("microluna-") && name.ends_with(".atif.jsonl") {
                        here_micro.push(entry.path());
                    }
                }
            }
        }
        here_episodes.sort();
        here_micro.sort();
        episodes.extend(here_episodes);
        micro.extend(here_micro);
    }
    (episodes, micro)
}

/// The episode log retained beside a Microluna session log: the
/// `episode.atif.jsonl` in the directory that holds its `artifacts`.
fn episode_beside(log: &Path) -> Option<PathBuf> {
    let artifacts = log.parent()?;
    let episode = artifacts.parent()?.join("episode.atif.jsonl");
    episode.is_file().then_some(episode)
}

/// A Wilson score interval at 95%, as `[low, high]`.
#[must_use]
pub fn wilson(successes: usize, n: usize) -> [f64; 2] {
    if n == 0 {
        return [0.0, 0.0];
    }
    let z = 1.959_964_f64;
    let n_f = n as f64;
    let p = successes as f64 / n_f;
    let centre = p + z * z / (2.0 * n_f);
    let spread = z * (p * (1.0 - p) / n_f + z * z / (4.0 * n_f * n_f)).sqrt();
    let denominator = 1.0 + z * z / n_f;
    [
        crate::component::pack::round((centre - spread) / denominator),
        crate::component::pack::round((centre + spread) / denominator),
    ]
}

/// Reads every log under `roots` once by content, counts each session
/// once by key, leaves out the sessions of the tasks in `exclude`, and
/// returns the rows and a summary.
#[must_use]
pub fn measure(roots: &[PathBuf], exclude: &BTreeSet<String>) -> (Vec<Row>, Value) {
    let (episodes, micro) = logs(roots);
    let mut seen_files: BTreeSet<String> = BTreeSet::new();
    let mut duplicate_files = 0usize;
    let mut read = |path: &Path| -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let digest = crate::ops::hex(&Sha256::digest(&bytes));
        if !seen_files.insert(digest) {
            duplicate_files += 1;
            return None;
        }
        Some(String::from_utf8_lossy(&bytes).into_owned())
    };
    let mut sessions: BTreeMap<String, Session> = BTreeMap::new();
    let keep = |session: Session, sessions: &mut BTreeMap<String, Session>| {
        // A partial copy, such as a live log, loses to the fuller one.
        let fuller = sessions
            .get(&session.key)
            .is_none_or(|kept| session.total_commands > kept.total_commands);
        if fuller {
            sessions.insert(session.key.clone(), session);
        }
    };
    let mut from_logs: BTreeSet<String> = BTreeSet::new();
    for path in &micro {
        let Some(text) = read(path) else { continue };
        let episode_at = episode_beside(path)
            .and_then(|episode| std::fs::read_to_string(episode).ok())
            .and_then(|text| opened_at(&text));
        if let Some(session) = microluna_session(&text, path, episode_at) {
            from_logs.insert(session.key.clone());
            keep(session, &mut sessions);
        }
    }
    for path in &episodes {
        let Some(text) = read(path) else { continue };
        for session in executor_sessions(&text, path, &from_logs) {
            keep(session, &mut sessions);
        }
    }
    let excluded = sessions
        .values()
        .filter(|s| exclude.contains(&s.task))
        .count();
    let rows: Vec<Row> = sessions
        .values()
        .filter(|s| !exclude.contains(&s.task))
        .map(count)
        .collect();
    let mut summary = summarize(&rows, episodes.len() + micro.len(), duplicate_files);
    summary["excluded_tasks"] = json!(exclude);
    summary["excluded_sessions"] = json!(excluded);
    (rows, summary)
}

/// The totals over `rows`, overall and by agent.
#[must_use]
pub fn summarize(rows: &[Row], files: usize, duplicate_files: usize) -> Value {
    let totals = |rows: &[&Row]| {
        let sessions = rows.len();
        let with_miss = rows
            .iter()
            .filter(|r| r.not_found + r.no_such_file > 0)
            .count();
        let with_prevented = rows.iter().filter(|r| r.prevented > 0).count();
        let not_found: usize = rows.iter().map(|r| r.not_found).sum();
        let no_such_file: usize = rows.iter().map(|r| r.no_such_file).sum();
        let misses = not_found + no_such_file;
        let prevented: usize = rows.iter().map(|r| r.prevented).sum();
        let commands: usize = rows.iter().map(|r| r.commands_before_edit).sum();
        json!({
            "sessions": sessions,
            "edited": rows.iter().filter(|r| r.edited).count(),
            "commands_before_edit": commands,
            "not_found": not_found,
            "no_such_file": no_such_file,
            "misses": misses,
            "nonzero": rows.iter().map(|r| r.nonzero).sum::<usize>(),
            "misses_per_session": crate::component::pack::round(misses as f64 / sessions.max(1) as f64),
            "sessions_with_miss": with_miss,
            "sessions_with_miss_share": crate::component::pack::round(with_miss as f64 / sessions.max(1) as f64),
            "sessions_with_miss_interval": wilson(with_miss, sessions),
            "prevented": prevented,
            "prevented_fixed": rows.iter().map(|r| r.prevented_fixed).sum::<usize>(),
            "prevented_share_of_misses": crate::component::pack::round(prevented as f64 / misses.max(1) as f64),
            "prevented_share_interval": wilson(prevented, misses),
            "sessions_with_prevented": with_prevented,
            "sessions_with_prevented_interval": wilson(with_prevented, sessions),
            "max_misses_in_a_session": rows.iter().map(|r| r.not_found + r.no_such_file).max().unwrap_or(0),
        })
    };
    let all: Vec<&Row> = rows.iter().collect();
    let mut by_agent = serde_json::Map::new();
    let agents: BTreeSet<&str> = rows.iter().map(|r| r.agent.as_str()).collect();
    for agent in agents {
        let some: Vec<&Row> = rows.iter().filter(|r| r.agent == agent).collect();
        by_agent.insert(agent.to_string(), totals(&some));
    }
    let mut by_task = serde_json::Map::new();
    let tasks: BTreeSet<&str> = rows.iter().map(|r| r.task.as_str()).collect();
    for task in tasks {
        let some: Vec<&Row> = rows.iter().filter(|r| r.task == task).collect();
        by_task.insert(
            task.to_string(),
            json!({
                "sessions": some.len(),
                "misses": some.iter().map(|r| r.not_found + r.no_such_file).sum::<usize>(),
                "prevented": some.iter().map(|r| r.prevented).sum::<usize>(),
                "sessions_with_prevented": some.iter().filter(|r| r.prevented > 0).count(),
            }),
        );
    }
    let mut programs: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        for (program, n) in &row.programs {
            *programs.entry(program.clone()).or_default() += n;
        }
    }
    json!({
        "schema": SCHEMA,
        "implementation": super::implementation(&super::Params::default()).digest,
        "log_files": files,
        "duplicate_files": duplicate_files,
        "totals": totals(&all),
        "by_agent": by_agent,
        "names": programs,
        "by_task": by_task,
    })
}

/// `coder-one environment measure --root DIR... [--exclude TASK]...
/// [--rows FILE]`: prints
/// the summary as JSON and, with `--rows`, writes one row per session as
/// JSON lines. Paths under the home directory print as `~/…`.
///
/// # Errors
///
/// Returns a message for a usage error or a file that can't be written.
pub fn command(args: &[String]) -> Result<i32, String> {
    const USAGE: &str = "usage: coder-one environment measure --root DIR [--root DIR]... [--exclude TASK]... [--rows FILE]";
    if args.first().map(String::as_str) != Some("measure") {
        return Err(USAGE.to_string());
    }
    let mut roots = Vec::new();
    let mut exclude = BTreeSet::new();
    let mut rows_out = None;
    let mut rest = args[1..].iter();
    while let Some(flag) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))?;
        match flag.as_str() {
            "--root" => roots.push(PathBuf::from(value)),
            "--rows" => rows_out = Some(PathBuf::from(value)),
            "--exclude" => {
                exclude.insert(value.clone());
            }
            _ => return Err(format!("unknown flag {flag}\n{USAGE}")),
        }
    }
    if roots.is_empty() {
        return Err(USAGE.to_string());
    }
    let (mut rows, summary) = measure(&roots, &exclude);
    let home = std::env::var("HOME").unwrap_or_default();
    let here = std::env::current_dir()
        .map(|dir| format!("{}/", dir.display()))
        .unwrap_or_default();
    for row in &mut rows {
        if let Some(rest) = row.source.strip_prefix(&here) {
            row.source = rest.to_string();
        } else if !home.is_empty()
            && let Some(rest) = row.source.strip_prefix(&home)
        {
            row.source = format!("~{rest}");
        }
    }
    if let Some(path) = rows_out {
        let text: String = rows
            .iter()
            .map(|row| format!("{}\n", serde_json::to_string(row).unwrap_or_default()))
            .collect();
        crate::record::write_atomic(&path, text.as_bytes())?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?
    );
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_diagnostics_name_the_missing_program() {
        let cases = [
            (
                "/bin/sh: 1: python: not found",
                Kind::NotFound,
                Some("python"),
            ),
            (
                "bash: line 1: git: command not found",
                Kind::NotFound,
                Some("git"),
            ),
            ("zsh: command not found: make", Kind::NotFound, Some("make")),
            (
                "/usr/bin/env: 'python': No such file or directory",
                Kind::NoSuchFile,
                Some("python"),
            ),
            (
                "bash: /usr/local/bin/pytest: No such file or directory",
                Kind::NoSuchFile,
                Some("pytest"),
            ),
            (
                "cat: /app/out.csv: No such file or directory",
                Kind::NoSuchFile,
                None,
            ),
            (
                "python3: can't open file '/app/x.py': [Errno 2] No such file or directory",
                Kind::NoSuchFile,
                None,
            ),
        ];
        for (line, kind, program) in cases {
            let found = misses(&format!("[exit 127]\n[stderr]\n{line}"));
            assert_eq!(found.len(), 1, "{line}");
            assert_eq!(found[0].kind, kind, "{line}");
            assert_eq!(found[0].program.as_deref(), program, "{line}");
        }
        assert!(misses("ModuleNotFoundError: No module named 'x'\nPage not found").is_empty());
    }

    #[test]
    fn a_microluna_log_counts_misses_before_the_first_edit_only() {
        let log = [
            json!({"record": "session", "at": 5, "session": {"id": "microluna-1-1"}}),
            json!({"record": "step", "step": {"source": "User", "message": "Fix /app/drift_monitor/monitor.py"}}),
            json!({"record": "step", "step": {"source": "Agent", "call": {"name": "run_command", "arguments": {"command": "python -m drift_monitor"}, "output": "[exit 127]\n[stderr]\n/bin/sh: 1: python: not found", "outcome": "Failed", "extra": {"exit": 127}}}}),
            json!({"record": "step", "step": {"source": "Agent", "call": {"name": "run_command", "arguments": {"command": "cat missing.txt; true"}, "output": "[exit 0]\n[stderr]\ncat: missing.txt: No such file or directory", "outcome": "Completed", "extra": {"exit": 0}}}}),
            json!({"record": "step", "step": {"source": "Agent", "call": {"name": "apply_patch", "arguments": {}, "output": "ok", "outcome": "Completed"}}}),
            json!({"record": "step", "step": {"source": "Agent", "call": {"name": "run_command", "arguments": {"command": "git diff"}, "output": "[exit 127]\n/bin/sh: 1: git: not found", "outcome": "Failed", "extra": {"exit": 127}}}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let path =
            Path::new("/x/embedding-drift-monitor__abc.episode/artifacts/microluna-1-1.atif.jsonl");
        let session = microluna_session(&log, path, Some(9)).unwrap();
        assert_eq!(session.key, "microluna:9:microluna-1-1");
        assert_eq!(session.task, "embedding-drift-monitor");
        assert_eq!(session.total_commands, 3);
        let row = count(&session);
        assert_eq!(
            (
                row.commands_before_edit,
                row.not_found,
                row.no_such_file,
                row.nonzero
            ),
            (2, 1, 1, 1)
        );
        assert_eq!((row.prevented, row.prevented_fixed), (1, 1));
        assert_eq!(row.programs.get("python"), Some(&1));
        assert!(row.edited);
    }

    #[test]
    fn executor_events_pair_commands_and_skip_microluna_sessions_read_from_their_logs() {
        let event = |adapter: &str, session: &str, inner: Value| {
            json!({"record": "step", "step": {"extensions": {"executor_event": {
                "adapter": adapter, "session_id": session, "generation": 1, "event": inner}}}})
        };
        let log = [
            json!({"record": "session", "at": 7, "session": {"id": "episode"}}),
            event("claude-code", "u1", json!({"kind": "command_started", "command": "coqc Main.v"})),
            event("claude-code", "u1", json!({"kind": "command_completed", "command": "toolu_1", "exit_code": 127, "output": "/bin/bash: line 1: coqc: command not found"})),
            event("claude-code", "u1", json!({"kind": "artifact_changed", "path": "Main.v"})),
            event("microluna", "microluna-1-1", json!({"kind": "command_completed", "command": "python x", "exit_code": 127, "output": "/bin/sh: 1: python: not found"})),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let path = Path::new("/j/proof-task__Q1.episode/episode.atif.jsonl");
        let skip: BTreeSet<String> = ["microluna:7:microluna-1-1".to_string()].into();
        let sessions = executor_sessions(&log, path, &skip);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].commands[0].text, "coqc Main.v");
        let row = count(&sessions[0]);
        assert_eq!(row.implied, ["coqc"]);
        // `coqc` is implied by the `.v` file, not in the fixed set.
        assert_eq!(
            (row.not_found, row.prevented, row.prevented_fixed),
            (1, 1, 0)
        );
        assert_eq!(executor_sessions(&log, path, &BTreeSet::new()).len(), 2);
    }

    #[test]
    fn a_wilson_interval_brackets_the_share() {
        let [low, high] = wilson(6, 20);
        assert!(low < 0.3 && 0.3 < high);
        assert_eq!(wilson(0, 0), [0.0, 0.0]);
    }
}
