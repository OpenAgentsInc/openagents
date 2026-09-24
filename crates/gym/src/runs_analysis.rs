//! One run's analysis, written when the run ends.
//!
//! `gym runs analyze RUN` reads a finished trial's records and computes
//! what the hand-written analyses in `docs/terminal-bench/` worked out by
//! hand, such as `2026-09-24-microluna-v6-embedding-definitive.md`:
//!
//! - **Outcome.** The verifier's reward and every test it ran, the
//!   assertion behind each failure, the true total cost (every Luna
//!   session from the Microluna usage records, the suite writers
//!   included; Jev from the episode's ledger; and any other executor), and
//!   Harbor's time spans.
//! - **Timeline.** Every component invocation and every Luna session with
//!   its start offset and duration, the critical path, the effective
//!   concurrency, and the cost by phase.
//! - **The suite against the verifier.** Each acceptance test and each
//!   verifier test, mapped by [`crate::runs_analysis_suite`].
//! - **Reversals.** A session that removed lines an earlier session added
//!   in the same file, rebuilt from the sessions' patches, with the test
//!   that drove it and whether the undone change was the better state.
//! - **Anomalies.** Blocked sessions, failed patches and tools, repeated
//!   identical failures, idle gaps, hung tests, repeated suite runs, and a
//!   Harbor cost that disagrees with the true total.
//! - **Fable 5.1.** The cheapest passing public attempt on the task and
//!   the mean of its cheapest effort tier, from
//!   `bench/terminal-bench/reference/fable-5.1-replays.json`.
//!
//! Code computes every number. Jev answers one narrow question, whether an
//! acceptance test checks what a verifier test checks, only for the pairs
//! the rules leave open, and every answer is cached. No other model runs.
//! [`crate::runs_analysis_markdown`] renders the result; `--write` keeps
//! `analysis.md` and `analysis.json` in the trial directory, the Runs pane
//! shows them under `A`, and `tbench` writes them when a trial ends.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::runs::{Catalog, Outcome, Run, Sources, read_json, text};
use crate::runs_analysis_suite as suite;
use crate::runs_learning::Judge;
use crate::terminal_bench::timestamp_ms;

/// The schema of `analysis.json`.
pub const SCHEMA: &str = "openagents.gym.run-analysis.v1";

/// The analysis's version. Change it with any change to what it computes,
/// so a stored analysis says which rules made it.
pub const VERSION: &str = "runs-analysis-v1";

/// The Markdown file `--write` keeps in the trial directory.
pub const MARKDOWN_FILE: &str = "analysis.md";

/// The JSON file `--write` keeps in the trial directory.
pub const JSON_FILE: &str = "analysis.json";

/// A gap between activities on the critical path at least this long is
/// idle time worth naming.
pub const IDLE_MS: i64 = 5_000;

/// The Harbor cost counts as matching the true total within this share.
pub const COST_TOLERANCE: f64 = 0.05;

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// One component invocation from Coder One's episode log.
#[derive(Clone, Debug, Default)]
pub struct Invocation {
    pub id: String,
    pub parent: Option<String>,
    pub component: String,
    pub name: String,
    pub start: i64,
    pub end: Option<i64>,
    pub outcome: Option<String>,
    pub summary: Value,
    pub cost_usd: Option<f64>,
    /// Whether the invocation is one Jev request.
    pub jev: bool,
    pub jev_input_tokens: u64,
    /// The System messages recorded under it, in order.
    pub messages: Vec<String>,
}

impl Invocation {
    fn end_or(&self, fallback: i64) -> i64 {
        self.end.unwrap_or(fallback).max(self.start)
    }
}

/// One acceptance-suite run, by the host or inside a session.
#[derive(Clone, Debug, Default)]
pub struct SuiteRun {
    pub label: String,
    pub start: i64,
    pub end: i64,
    pub passed: u64,
    pub total: u64,
    pub green: bool,
    /// Each test: its ID, whether it passed, whether it was killed, and
    /// its time.
    pub tests: Vec<(String, bool, bool, u64)>,
    /// The session that ran it, or `None` for the host.
    pub session: Option<String>,
}

/// Coder One's episode log, read.
#[derive(Clone, Debug, Default)]
pub struct Episode {
    pub origin: i64,
    pub end: i64,
    pub invocations: Vec<Invocation>,
    pub suite_runs: Vec<SuiteRun>,
    pub version: Option<String>,
    pub policy: Option<String>,
    pub policy_digest: Option<String>,
}

/// One model request in a Luna session.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    pub at: i64,
    pub ms: u64,
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cost_usd: f64,
}

/// One tool call in a Luna session.
#[derive(Clone, Debug, Default)]
pub struct Call {
    pub at: i64,
    pub name: String,
    pub arguments: Value,
    pub output: String,
    pub ok: bool,
    pub ms: u64,
    /// The exit code a command printed, `[exit N]`.
    pub exit: Option<i64>,
}

impl Call {
    fn command(&self) -> Option<&str> {
        self.arguments.get("command").and_then(Value::as_str)
    }

    /// Whether the call runs an acceptance suite's `run.sh`.
    fn runs_suite(&self) -> bool {
        self.name == "run_command"
            && self.command().is_some_and(|command| {
                command.contains("accept-suite") && command.contains("run.sh")
            })
    }
}

/// What a session was for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Writes the acceptance suite.
    Writer,
    /// Writes tests for the suite's gaps after it went green.
    GapWriter,
    /// Changes the workspace toward the suite or the task.
    Edit,
    /// Reviews the workspace after the suite went green.
    Audit,
    /// Runs under `verify.repair`.
    Repair,
    Other,
}

impl Role {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Role::Writer => "suite writer",
            Role::GapWriter => "gap writer",
            Role::Edit => "edit",
            Role::Audit => "audit",
            Role::Repair => "repair",
            Role::Other => "session",
        }
    }

    fn edits_workspace(self) -> bool {
        matches!(self, Role::Edit | Role::Audit | Role::Repair | Role::Other)
    }
}

/// One Luna session log.
#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub file: String,
    pub directive: String,
    pub repository: String,
    pub start: i64,
    pub end: i64,
    pub turns: Vec<Turn>,
    pub calls: Vec<Call>,
    /// The `finish` call's status and summary.
    pub finish: Option<(String, String)>,
    /// The first user messages: the brief.
    pub brief: String,
    pub role: Role,
    /// The invocation that ran it.
    pub invocation: Option<String>,
}

impl Session {
    #[must_use]
    pub fn cost_usd(&self) -> f64 {
        self.turns.iter().map(|turn| turn.cost_usd).sum()
    }

    #[must_use]
    pub fn model_ms(&self) -> u64 {
        self.turns.iter().map(|turn| turn.ms).sum()
    }

    #[must_use]
    pub fn tool_ms(&self) -> u64 {
        self.calls.iter().map(|call| call.ms).sum()
    }
}

/// One verifier test, from `ctrf.json` and the task's test source.
#[derive(Clone, Debug, Default, Serialize)]
pub struct VerifierTest {
    pub name: String,
    pub status: String,
    /// The message the verifier printed for a failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// The docstring's first line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    /// For a failure, the assertions in the test's source.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assertions: Vec<String>,
    #[serde(skip)]
    pub source: String,
}

impl VerifierTest {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.status == "passed"
    }
}

/// Everything the analysis reads for one run.
#[derive(Clone, Debug)]
pub struct Records {
    pub run: Run,
    pub trial: PathBuf,
    pub episode_dir: Option<PathBuf>,
    pub result: Option<Value>,
    pub usage: Option<Value>,
    pub accept: Option<Value>,
    pub suite_dir: Option<PathBuf>,
    pub episode: Option<Episode>,
    pub sessions: Vec<Session>,
    pub verifier: Vec<VerifierTest>,
    pub verifier_seconds: Option<f64>,
}

fn jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn read_episode(path: &Path) -> Option<Episode> {
    let records = jsonl(path);
    if records.is_empty() {
        return None;
    }
    let mut episode = Episode::default();
    let mut index: HashMap<String, usize> = HashMap::new();
    let (mut first, mut last) = (None, 0_i64);
    for record in &records {
        let step = record.get("step");
        let at = step
            .and_then(|step| step.get("at"))
            .or_else(|| record.get("at"))
            .and_then(Value::as_i64);
        if let Some(at) = at {
            first.get_or_insert(at);
            last = last.max(at);
        }
        if record["record"] == "session" {
            episode.version = episode.version.or_else(|| text(record, "/session/version"));
            continue;
        }
        let Some(step) = step else { continue };
        let extensions = &step["extensions"];
        if let Some(invocation) = extensions.get("invocation").filter(|v| v.is_object()) {
            let id = invocation["id"].as_str().unwrap_or_default().to_owned();
            let when = invocation["at"].as_i64().or(at).unwrap_or(0);
            match invocation["event"].as_str() {
                Some("start") => {
                    let component = text(invocation, "/component").unwrap_or_default();
                    if component == "episode" && episode.policy.is_none() {
                        episode.policy = text(invocation, "/implementation/name")
                            .map(|name| name.strip_prefix("policy ").unwrap_or(&name).to_owned());
                        episode.policy_digest = text(invocation, "/implementation/digest");
                    }
                    index.insert(id.clone(), episode.invocations.len());
                    episode.invocations.push(Invocation {
                        id,
                        parent: text(invocation, "/parent"),
                        component,
                        name: text(invocation, "/name").unwrap_or_default(),
                        start: when,
                        ..Invocation::default()
                    });
                }
                Some("end") => {
                    if let Some(&at) = index.get(&id) {
                        let found = &mut episode.invocations[at];
                        found.end = Some(when);
                        found.outcome = text(invocation, "/outcome");
                        found.summary = invocation
                            .pointer("/output/summary")
                            .cloned()
                            .unwrap_or(Value::Null);
                        found.cost_usd = invocation.pointer("/cost/usd").and_then(Value::as_f64);
                    }
                }
                _ => {}
            }
            continue;
        }
        let owner = extensions
            .get("invocation_id")
            .and_then(Value::as_str)
            .and_then(|id| index.get(id))
            .copied();
        if let (Some(usage), Some(owner)) = (extensions.get("jev_usage"), owner) {
            let found = &mut episode.invocations[owner];
            found.jev = true;
            found.jev_input_tokens += usage["input_tokens"].as_u64().unwrap_or(0);
        }
        if let Some(run) = extensions.get("accept.run.v1") {
            let end = at.unwrap_or(0);
            let label = text(run, "/label").unwrap_or_default();
            // The step may belong to the dispatch; the run's own
            // invocation is the latest `accept.run` of that name.
            let start = episode
                .invocations
                .iter()
                .rev()
                .find(|i| i.component == "accept.run" && i.start <= end && i.name == label)
                .or_else(|| {
                    owner
                        .map(|owner| &episode.invocations[owner])
                        .filter(|i| i.component == "accept.run")
                })
                .map_or(end, |i| i.start);
            episode.suite_runs.push(SuiteRun {
                label,
                start,
                end,
                passed: run["passed"].as_u64().unwrap_or(0),
                total: run["total"].as_u64().unwrap_or(0),
                green: run["green"].as_bool().unwrap_or(false),
                tests: run["tests"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|test| {
                        (
                            text(test, "/id").unwrap_or_default(),
                            test["green"].as_bool().unwrap_or(false),
                            test["killed"].as_bool().unwrap_or(false),
                            test["milliseconds"].as_u64().unwrap_or(0),
                        )
                    })
                    .collect(),
                session: None,
            });
        }
        if step["source"] == "System"
            && let (Some(owner), Some(message)) = (owner, step["message"].as_str())
            && !message.is_empty()
            && !message.starts_with("executor event")
        {
            episode.invocations[owner].messages.push(message.to_owned());
        }
    }
    let root = episode
        .invocations
        .iter()
        .find(|invocation| invocation.parent.is_none());
    episode.origin = root.map(|root| root.start).or(first).unwrap_or(0);
    episode.end = root
        .and_then(|root| root.end)
        .unwrap_or(last)
        .max(episode.origin);
    Some(episode)
}

fn read_session(path: &Path) -> Option<Session> {
    let records = jsonl(path);
    let head = records
        .iter()
        .find(|record| record["record"] == "session")?;
    let mut session = Session {
        id: text(head, "/session/id").unwrap_or_default(),
        file: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        directive: text(head, "/session/directive").unwrap_or_default(),
        repository: text(head, "/session/repository").unwrap_or_default(),
        start: head["at"].as_i64().unwrap_or(0),
        end: 0,
        turns: Vec::new(),
        calls: Vec::new(),
        finish: None,
        brief: String::new(),
        role: Role::Other,
        invocation: None,
    };
    let mut last = session.start;
    let mut users = 0;
    for record in &records {
        if record["record"] == "end" {
            if let Some(at) = record["at"].as_i64() {
                session.end = at;
            }
            continue;
        }
        let Some(step) = record.get("step") else {
            continue;
        };
        let at = step["at"].as_i64().unwrap_or(last);
        last = last.max(at);
        match step["source"].as_str() {
            Some("User") if users < 2 => {
                users += 1;
                if let Some(message) = step["message"].as_str() {
                    session.brief.push_str(message);
                    session.brief.push('\n');
                }
            }
            Some("Agent") => {
                if let Some(call) = step.get("call").filter(|call| call.is_object()) {
                    let output = call["output"].as_str().unwrap_or_default().to_owned();
                    let exit = output
                        .strip_prefix("[exit ")
                        .and_then(|rest| rest.split(']').next())
                        .and_then(|code| code.trim().parse().ok());
                    let name = text(call, "/name").unwrap_or_default();
                    let arguments = call["arguments"].clone();
                    if name == "finish" {
                        session.finish = Some((
                            text(&arguments, "/status").unwrap_or_default(),
                            text(&arguments, "/summary").unwrap_or_default(),
                        ));
                    }
                    session.calls.push(Call {
                        at,
                        name,
                        arguments,
                        ok: call["outcome"].as_str().is_none_or(|o| o == "Completed"),
                        output,
                        ms: call["milliseconds"].as_u64().unwrap_or(0),
                        exit,
                    });
                } else if let Some(ms) = step["milliseconds"].as_u64() {
                    let usage = &step["extensions"]["microluna.usage.v1"];
                    let tokens = step["tokens"].as_array();
                    let token = |index: usize| {
                        tokens
                            .and_then(|tokens| tokens.get(index))
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                    };
                    session.turns.push(Turn {
                        at,
                        ms,
                        input: usage["input"].as_u64().unwrap_or_else(|| token(0)),
                        cached: usage["cached"].as_u64().unwrap_or(0),
                        output: usage["output"].as_u64().unwrap_or_else(|| token(1)),
                        reasoning: usage["reasoning"].as_u64().unwrap_or(0),
                        cost_usd: usage["cost_usd"].as_f64().unwrap_or(0.0),
                    });
                }
            }
            _ => {}
        }
    }
    if session.end == 0 {
        session.end = last;
    }
    Some(session)
}

/// The test functions in a task's `tests/*.py`, by name, with their source.
fn test_sources(task: &Path) -> HashMap<String, String> {
    let mut sources = HashMap::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(task.join("tests"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "py"))
        .collect();
    files.sort();
    for file in files {
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };
        let lines: Vec<&str> = source.lines().collect();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            let name = line
                .strip_prefix("def ")
                .or_else(|| line.strip_prefix("async def "))
                .and_then(|rest| rest.split('(').next())
                .filter(|name| name.starts_with("test"));
            let Some(name) = name else {
                index += 1;
                continue;
            };
            let mut end = index + 1;
            while end < lines.len() {
                let next = lines[end];
                if !next.is_empty()
                    && !next.starts_with(char::is_whitespace)
                    && !next.starts_with('#')
                {
                    break;
                }
                end += 1;
            }
            sources.insert(name.to_owned(), lines[index..end].join("\n"));
            index = end;
        }
    }
    sources
}

/// The docstring's first line of a Python function's source.
fn docstring(source: &str) -> Option<String> {
    let body = source.split_once("\"\"\"")?.1;
    let body = body.split("\"\"\"").next().unwrap_or(body);
    let paragraph: Vec<&str> = body
        .trim_start()
        .lines()
        .map(str::trim)
        .take_while(|line| !line.is_empty())
        .collect();
    let paragraph = paragraph.join(" ");
    // The first sentence, or the paragraph when it has none.
    let sentence = paragraph
        .match_indices(". ")
        .next()
        .map_or(paragraph.as_str(), |(at, _)| &paragraph[..=at]);
    let sentence = sentence.trim();
    (!sentence.is_empty()).then(|| sentence.to_owned())
}

/// Each `assert` statement in a Python function's source, with its
/// continuation lines, up to six lines each.
fn assertions(source: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if !line.trim_start().starts_with("assert ") {
            index += 1;
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let mut end = index + 1;
        let mut depth: i64 = line.matches(['(', '[', '{']).count() as i64
            - line.matches([')', ']', '}']).count() as i64;
        while end < lines.len() && end - index < 6 {
            let next = lines[end];
            let next_indent = next.len() - next.trim_start().len();
            if depth <= 0 && (next.trim().is_empty() || next_indent <= indent) {
                break;
            }
            depth += next.matches(['(', '[', '{']).count() as i64
                - next.matches([')', ']', '}']).count() as i64;
            end += 1;
        }
        let text: Vec<String> = lines[index..end]
            .iter()
            .map(|line| {
                line.get(indent.min(line.len())..)
                    .unwrap_or(line)
                    .to_owned()
            })
            .collect();
        found.push(text.join("\n"));
        index = end;
    }
    found
}

fn read_verifier(trial: &Path, task: Option<&Path>) -> (Vec<VerifierTest>, Option<f64>) {
    let verifier = trial.join("verifier");
    let sources = task.map(test_sources).unwrap_or_default();
    let mut tests = Vec::new();
    if let Some(ctrf) = read_json(&verifier.join("ctrf.json")) {
        for test in ctrf
            .pointer("/results/tests")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let full = text(test, "/name").unwrap_or_default();
            let name = full.rsplit("::").next().unwrap_or(&full).to_owned();
            let source = sources.get(&name).cloned().unwrap_or_default();
            let status = text(test, "/status").unwrap_or_default();
            tests.push(VerifierTest {
                message: text(test, "/trace").or_else(|| text(test, "/message")),
                doc: docstring(&source),
                assertions: if status == "passed" {
                    Vec::new()
                } else {
                    assertions(&source)
                },
                name,
                status,
                source,
            });
        }
    }
    let stdout = std::fs::read_to_string(verifier.join("test-stdout.txt")).unwrap_or_default();
    // pytest's last line: `== 1 failed, 10 passed in 27.18s ==`.
    let seconds = stdout.lines().rev().find_map(|line| {
        let rest = line.rsplit_once(" in ")?.1;
        rest.split('s').next()?.trim().parse::<f64>().ok()
    });
    if tests.is_empty() {
        // No ctrf: read pytest's short summary.
        for line in stdout.lines() {
            let (status, rest) = if let Some(rest) = line.strip_prefix("PASSED ") {
                ("passed", rest)
            } else if let Some(rest) = line.strip_prefix("FAILED ") {
                ("failed", rest)
            } else {
                continue;
            };
            let full = rest.split(" - ").next().unwrap_or(rest).trim();
            let name = full.rsplit("::").next().unwrap_or(full).to_owned();
            if tests.iter().any(|test: &VerifierTest| test.name == name) {
                continue;
            }
            let source = sources.get(&name).cloned().unwrap_or_default();
            tests.push(VerifierTest {
                message: rest.split_once(" - ").map(|(_, why)| why.to_owned()),
                doc: docstring(&source),
                assertions: if status == "passed" {
                    Vec::new()
                } else {
                    assertions(&source)
                },
                name,
                status: status.to_owned(),
                source,
            });
        }
    }
    (tests, seconds)
}

impl Records {
    /// Reads every record the analysis uses for `run`.
    #[must_use]
    pub fn load(run: &Run) -> Self {
        let trial = run.files.dir.clone();
        let episode_dir = run.files.episode.clone();
        let result = run.files.result.as_deref().and_then(read_json);
        let usage = episode_dir.as_ref().and_then(|dir| {
            read_json(&dir.join("evaluation/usage.json"))
                .or_else(|| read_json(&dir.join("usage.json")))
        });
        let episode = episode_dir
            .as_ref()
            .map(|dir| dir.join("episode.atif.jsonl"))
            .filter(|path| path.is_file())
            .or_else(|| run.files.live.clone())
            .and_then(|path| read_episode(&path));
        let mut accept_files: Vec<PathBuf> = episode_dir
            .iter()
            .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".accept.json"))
            })
            .collect();
        accept_files.sort();
        let accept = accept_files.last().and_then(|path| read_json(path));
        let suite_dir = accept
            .as_ref()
            .and_then(|accept| text(accept, "/dir"))
            .and_then(|dir| dir.rsplit('/').next().map(str::to_owned))
            .and_then(|name| episode_dir.as_ref().map(|dir| dir.join(name)))
            .filter(|dir| dir.is_dir());
        let mut session_files: Vec<PathBuf> = episode_dir
            .iter()
            .map(|dir| dir.join("artifacts"))
            .chain(std::iter::once(trial.join("agent/live/artifacts")))
            .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".atif.jsonl"))
            })
            .collect();
        session_files.sort();
        let mut seen = BTreeSet::new();
        let mut sessions: Vec<Session> = session_files
            .iter()
            .filter(|path| seen.insert(path.file_name().map(|n| n.to_owned())))
            .filter_map(|path| read_session(path))
            .collect();
        sessions.sort_by_key(|session| (session.start, session.id.clone()));
        let (verifier, verifier_seconds) = read_verifier(&trial, run.task_path.as_deref());
        let mut records = Records {
            run: run.clone(),
            trial,
            episode_dir,
            result,
            usage,
            accept,
            suite_dir,
            episode,
            sessions,
            verifier,
            verifier_seconds,
        };
        records.place_sessions();
        records
    }

    /// Gives each session its role and the invocation that ran it, and
    /// each suite run inside a session its session.
    fn place_sessions(&mut self) {
        let Some(episode) = &self.episode else {
            for session in &mut self.sessions {
                session.role = role_by_name(&session.id, None);
            }
            return;
        };
        let by_id: HashMap<&str, &Invocation> = episode
            .invocations
            .iter()
            .map(|invocation| (invocation.id.as_str(), invocation))
            .collect();
        let ancestors = |id: &str| {
            let mut chain = Vec::new();
            let mut current = by_id.get(id).copied();
            while let Some(invocation) = current {
                chain.push(invocation);
                current = invocation
                    .parent
                    .as_deref()
                    .and_then(|p| by_id.get(p).copied());
            }
            chain
        };
        for session in &mut self.sessions {
            let traced = episode.invocations.iter().find(|invocation| {
                invocation.component == "microluna.session"
                    && text(&invocation.summary, "/trace")
                        .is_some_and(|trace| trace.ends_with(&session.file))
            });
            let contained = || {
                episode
                    .invocations
                    .iter()
                    .filter(|invocation| {
                        invocation.component == "microluna.session"
                            && invocation.start <= session.start + 500
                            && invocation.end_or(episode.end) + 500 >= session.end
                    })
                    .min_by_key(|invocation| invocation.end_or(episode.end) - invocation.start)
            };
            // A writer runs beside the session it overlaps, not inside it.
            let owner = traced.or_else(|| {
                (!session.id.starts_with("accept-writer"))
                    .then(contained)
                    .flatten()
            });
            session.invocation = owner.map(|invocation| invocation.id.clone());
            let under_repair = owner.is_some_and(|invocation| {
                ancestors(&invocation.id)
                    .iter()
                    .any(|a| a.component == "verify.repair")
            });
            session.role = role_by_name(
                &session.id,
                owner.map(|invocation| invocation.name.as_str()),
            );
            if under_repair {
                session.role = Role::Repair;
            }
        }
    }
}

fn role_by_name(id: &str, invocation: Option<&str>) -> Role {
    if id.starts_with("accept-writer-gap") {
        Role::GapWriter
    } else if id.starts_with("accept-writer") {
        Role::Writer
    } else if invocation.is_some_and(|name| name.contains("audit")) {
        Role::Audit
    } else if id.starts_with("microluna") {
        Role::Edit
    } else {
        Role::Other
    }
}

// ---------------------------------------------------------------------------
// The analysis
// ---------------------------------------------------------------------------

/// Which run the analysis is of.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RunInfo {
    pub id: String,
    pub job: String,
    pub trial: String,
    pub task: String,
    pub agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_digest: Option<String>,
    /// The episode's start, milliseconds since the epoch: every offset in
    /// the analysis counts from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_ms: Option<i64>,
}

/// The verifier's result.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Verdict {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reward: Option<f64>,
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
    pub tests: Vec<VerifierTest>,
}

/// One of Harbor's time spans.
#[derive(Clone, Debug, Default, Serialize)]
pub struct TimeSpan {
    pub span: String,
    /// Milliseconds since the epoch.
    pub start_ms: i64,
    pub duration_ms: u64,
}

/// A session's line in the cost table.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SessionCost {
    pub session: String,
    pub role: String,
    pub turns: usize,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cost_usd: f64,
}

/// Jev's requests of one kind.
#[derive(Clone, Debug, Default, Serialize)]
pub struct JevGroup {
    pub name: String,
    pub requests: usize,
    pub cost_usd: f64,
}

/// The true total cost against Harbor's.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Cost {
    pub sessions: Vec<SessionCost>,
    pub luna_usd: f64,
    pub luna_turns: usize,
    pub jev: Vec<JevGroup>,
    pub jev_usd: f64,
    pub jev_requests: usize,
    /// Generation and any executor other than Microluna.
    pub other_usd: f64,
    pub total_usd: f64,
    /// What Harbor's `result.json` reports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harbor_usd: Option<f64>,
    /// What the Harbor figure leaves out, when it disagrees.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harbor_missing: Option<String>,
}

/// One row of the per-phase timeline.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PhaseRow {
    pub depth: usize,
    /// Milliseconds from the episode's start.
    pub start_ms: i64,
    pub duration_ms: u64,
    pub phase: String,
    pub what: String,
}

/// One session's line in the per-session table.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SessionRow {
    pub session: String,
    pub role: String,
    pub start_ms: i64,
    pub duration_ms: u64,
    pub turns: usize,
    pub model_ms: u64,
    pub tool_ms: u64,
    pub patches: usize,
    pub failed_patches: usize,
    pub cost_usd: f64,
    /// `done`, `blocked`, `failed`, or `no finish`.
    pub ending: String,
    pub summary: String,
    pub directive: String,
    /// Each turn: model seconds, then tool seconds.
    pub pairs: String,
}

/// One segment of the critical path.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Segment {
    pub start_ms: i64,
    pub duration_ms: u64,
    pub label: String,
    pub category: String,
}

/// A phase's cost.
#[derive(Clone, Debug, Default, Serialize)]
pub struct PhaseCost {
    pub phase: String,
    pub luna_usd: f64,
    pub jev_usd: f64,
    pub jev_requests: usize,
}

/// The timeline.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Timeline {
    pub episode_ms: u64,
    pub phases: Vec<PhaseRow>,
    pub sessions: Vec<SessionRow>,
    pub critical_path: Vec<Segment>,
    /// Critical-path time by category, largest first.
    pub categories: Vec<(String, u64)>,
    /// Luna session time over the episode's wall time.
    pub concurrency: f64,
    pub peak_sessions: usize,
    pub session_ms: u64,
    /// The time from the last moment two sessions overlapped to the end.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_tail_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_edit_ms: Option<i64>,
    pub cost_by_phase: Vec<PhaseCost>,
}

/// A session that undid an earlier session's change.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Reversal {
    /// `revert`: the later session put back something like the code the
    /// earlier one replaced. `rewrite`: it replaced the earlier session's
    /// lines with new code.
    pub kind: String,
    pub file: String,
    pub earlier: String,
    pub later: String,
    pub at_ms: i64,
    /// Lines the later session removed that the earlier one added.
    pub undone: usize,
    /// The lines the earlier session added to the file.
    pub earlier_added: usize,
    pub symbols: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restored_by: Option<String>,
    /// The tests named red in the later session's brief.
    pub drivers: Vec<String>,
    /// Which of them were guards: green on the untouched workspace.
    pub guards: Vec<String>,
    /// Why the undone change looks like the better state, when it does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub better_then_reverted: Option<String>,
    pub removed: Vec<String>,
    pub added: Vec<String>,
}

/// A guard that turned red and made a session edit.
#[derive(Clone, Debug, Default, Serialize)]
pub struct GuardEdit {
    pub test: String,
    pub red_in: String,
    pub session: String,
    pub files: Vec<String>,
}

/// Something that went wrong or was wasted.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Anomaly {
    pub kind: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f64>,
}

/// The run against Fable 5.1's public attempts on its task.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Fable {
    pub attempts: usize,
    pub passes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cheapest_pass: Option<FableAttempt>,
    /// The mean of the cheapest effort tier with attempts on the task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<FableTier>,
    pub all_mean_usd: f64,
    pub all_mean_sec: f64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FableAttempt {
    pub id: String,
    pub effort: String,
    pub cost_usd: f64,
    pub seconds: f64,
    pub steps: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct FableTier {
    pub effort: String,
    pub attempts: usize,
    pub passes: usize,
    pub mean_usd: f64,
    pub mean_sec: f64,
}

/// What Jev did for this analysis.
#[derive(Clone, Debug, Default, Serialize)]
pub struct JevUse {
    pub mode: String,
    pub asked: usize,
    pub cached: usize,
    pub failed: usize,
    pub input_tokens: u64,
    pub cost_usd: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

/// One run's analysis.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Analysis {
    pub schema: String,
    pub version: String,
    pub run: RunInfo,
    pub verdict: Verdict,
    pub agent_ms: Option<u64>,
    pub trial_ms: Option<u64>,
    pub spans: Vec<TimeSpan>,
    pub cost: Cost,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<Timeline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<suite::Section>,
    pub reversals: Vec<Reversal>,
    pub guard_edits: Vec<GuardEdit>,
    pub anomalies: Vec<Anomaly>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fable: Option<Fable>,
    pub jev: JevUse,
}

/// Computes everything but the suite mapping's Jev answers, which
/// [`suite::section`] adds.
#[must_use]
pub fn compute(records: &Records, fable: Option<&Value>) -> Analysis {
    let run = &records.run;
    let episode = records.episode.as_ref();
    let mut analysis = Analysis {
        schema: SCHEMA.to_owned(),
        version: VERSION.to_owned(),
        run: RunInfo {
            id: run.id(),
            job: run.job.clone(),
            trial: run.trial.clone(),
            task: run.task.clone(),
            agent: run.agent_label(),
            artifact: episode.and_then(|e| e.version.clone()),
            policy: episode.and_then(|e| e.policy.clone()),
            policy_digest: episode.and_then(|e| e.policy_digest.clone()),
            origin_ms: episode.map(|e| e.origin),
        },
        agent_ms: run.agent_ms,
        trial_ms: run.elapsed_ms(crate::runs::now_ms()),
        ..Analysis::default()
    };
    analysis.verdict = verdict(records);
    analysis.spans = spans(records);
    analysis.cost = cost(records);
    if let Some(episode) = episode {
        analysis.timeline = Some(timeline(records, episode));
    }
    let (reversals, guard_edits) = reversals(records);
    analysis.reversals = reversals;
    analysis.guard_edits = guard_edits;
    analysis.fable = fable.and_then(|manifest| fable_of(manifest, &run.task));
    analysis.anomalies = anomalies(records, &analysis);
    analysis
}

fn verdict(records: &Records) -> Verdict {
    let run = &records.run;
    let passed = records.verifier.iter().filter(|t| t.passed()).count();
    let total = records.verifier.len();
    Verdict {
        outcome: match &run.outcome {
            Outcome::NotGraded(why) => format!("not graded: {why}"),
            other => other.word().to_owned(),
        },
        reward: run.reward,
        passed,
        failed: total - passed,
        total,
        seconds: records.verifier_seconds,
        tests: records.verifier.clone(),
    }
}

fn spans(records: &Records) -> Vec<TimeSpan> {
    let Some(result) = &records.result else {
        return Vec::new();
    };
    let at = |pointer: &str| text(result, pointer).and_then(|t| timestamp_ms(&t));
    let span = |name: &str, start: Option<i64>, end: Option<i64>| {
        let (start, end) = (start?, end?);
        Some(TimeSpan {
            span: name.to_owned(),
            start_ms: start,
            duration_ms: u64::try_from(end - start).ok()?,
        })
    };
    let mut spans: Vec<TimeSpan> = [
        ("Environment setup", "/environment_setup"),
        ("Agent setup", "/agent_setup"),
        ("Agent execution", "/agent_execution"),
    ]
    .iter()
    .filter_map(|(name, key)| {
        span(
            name,
            at(&format!("{key}/started_at")),
            at(&format!("{key}/finished_at")),
        )
    })
    .collect();
    if let Some(episode) = &records.episode {
        spans.extend(span(
            "The episode inside it",
            Some(episode.origin),
            Some(episode.end),
        ));
    }
    spans.extend(span(
        "Artifact collection",
        at("/agent_execution/finished_at"),
        at("/verifier/started_at"),
    ));
    spans.extend(span(
        "Verifier",
        at("/verifier/started_at"),
        at("/verifier/finished_at"),
    ));
    spans.extend(span("Trial", at("/started_at"), at("/finished_at")));
    spans
}

fn cost(records: &Records) -> Cost {
    let mut cost = Cost::default();
    for session in &records.sessions {
        let turns = &session.turns;
        cost.sessions.push(SessionCost {
            session: session.id.clone(),
            role: session.role.word().to_owned(),
            turns: turns.len(),
            input_tokens: turns.iter().map(|t| t.input).sum(),
            cached_tokens: turns.iter().map(|t| t.cached).sum(),
            output_tokens: turns.iter().map(|t| t.output).sum(),
            reasoning_tokens: turns.iter().map(|t| t.reasoning).sum(),
            cost_usd: session.cost_usd(),
        });
    }
    cost.luna_usd = cost.sessions.iter().map(|s| s.cost_usd).sum();
    cost.luna_turns = cost.sessions.iter().map(|s| s.turns).sum();
    // Jev: every request in the episode log, priced at Jev's rate unless
    // the invocation carries its own cost.
    let rate = crate::runs_learning::USD_PER_MILLION_INPUT / 1_000_000.0;
    let mut groups: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for invocation in records
        .episode
        .iter()
        .flat_map(|e| &e.invocations)
        .filter(|i| i.jev)
    {
        let usd = invocation
            .cost_usd
            .unwrap_or(invocation.jev_input_tokens as f64 * rate);
        let entry = groups.entry(invocation.name.clone()).or_default();
        entry.0 += 1;
        entry.1 += usd;
    }
    cost.jev = groups
        .into_iter()
        .map(|(name, (requests, cost_usd))| JevGroup {
            name,
            requests,
            cost_usd,
        })
        .collect();
    cost.jev_usd = cost.jev.iter().map(|g| g.cost_usd).sum();
    cost.jev_requests = cost.jev.iter().map(|g| g.requests).sum();
    let usage = records.usage.as_ref();
    if cost.jev_requests == 0
        && let Some(jev) = usage.and_then(|u| u.pointer("/components/jev"))
    {
        cost.jev_usd = jev["cost_usd"].as_f64().unwrap_or(0.0);
        cost.jev_requests = usize::try_from(jev["requests"].as_u64().unwrap_or(0)).unwrap_or(0);
    }
    let generation = usage
        .and_then(|u| u.pointer("/components/generation/cost_usd"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let delegate = usage.and_then(|u| u.pointer("/components/delegate"));
    let delegate_usd = delegate.and_then(|d| d["cost_usd"].as_f64()).unwrap_or(0.0);
    let microluna = delegate.is_some_and(|d| {
        d["agents"]
            .as_array()
            .is_some_and(|agents| agents.iter().all(|a| a == "microluna"))
            || d["agent"] == "microluna"
    }) || (delegate.is_none() && !records.sessions.is_empty());
    cost.other_usd = generation + if microluna { 0.0 } else { delegate_usd };
    cost.harbor_usd = records
        .result
        .as_ref()
        .and_then(|r| r.pointer("/agent_result/cost_usd"))
        .and_then(Value::as_f64)
        .or(records.run.cost_usd);
    if records.episode.is_none() && records.sessions.is_empty() {
        // Not a Coder One episode: Harbor's figure, or the catalog's
        // estimate, is all there is.
        cost.other_usd = records.run.cost_usd.unwrap_or(0.0);
    }
    cost.total_usd = cost.luna_usd + cost.jev_usd + cost.other_usd;
    if let Some(harbor) = cost.harbor_usd
        && cost.total_usd > 0.0
        && (cost.total_usd - harbor).abs() > COST_TOLERANCE * cost.total_usd
    {
        cost.harbor_missing = Some(missing_cost(records, harbor, &cost));
    }
    cost
}

/// What Harbor's figure leaves out: the sessions whose cost makes up the
/// difference, when some do.
fn missing_cost(records: &Records, harbor: f64, cost: &Cost) -> String {
    let difference = cost.total_usd - harbor;
    if difference < 0.0 {
        return format!(
            "Harbor reports {} more than the sessions and Jev add up to",
            usd(-difference)
        );
    }
    let close = |amount: f64| (amount - difference).abs() <= 0.02 * difference.max(1e-9) + 1e-6;
    let of = |roles: &[Role]| -> (f64, usize) {
        let chosen: Vec<&Session> = records
            .sessions
            .iter()
            .filter(|s| roles.contains(&s.role))
            .collect();
        (chosen.iter().map(|s| s.cost_usd()).sum(), chosen.len())
    };
    // Whole roles first: a harness that drops a kind of session drops
    // all of them.
    for (roles, one, many) in [
        (&[Role::GapWriter][..], "the gap writer", "the gap writers"),
        (
            &[Role::Writer, Role::GapWriter][..],
            "the suite writer",
            "the suite writers",
        ),
        (&[Role::Writer][..], "the suite writer", "the suite writers"),
        (
            &[Role::Repair][..],
            "the repair session",
            "the repair sessions",
        ),
    ] {
        let (amount, count) = of(roles);
        if count > 0 && close(amount) {
            return if count == 1 {
                format!("{one} ({})", usd(amount))
            } else {
                format!("{many} ({}, {count} sessions)", usd(amount))
            };
        }
    }
    if let Some(session) = records
        .sessions
        .iter()
        .filter(|s| close(s.cost_usd()))
        .min_by(|a, b| {
            (a.cost_usd() - difference)
                .abs()
                .total_cmp(&(b.cost_usd() - difference).abs())
        })
    {
        return format!("`{}` ({})", session.id, usd(session.cost_usd()));
    }
    if close(cost.jev_usd) {
        return format!("Jev ({})", usd(cost.jev_usd));
    }
    format!("{} it can't attribute", usd(difference))
}

// ---------------------------------------------------------------------------
// Timeline
// ---------------------------------------------------------------------------

fn clip(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        text
    } else {
        let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
        format!("{}…", kept.trim_end())
    }
}

/// What an invocation did, in a line. `jev` counts the Jev requests of
/// its component while it ran.
fn what(invocation: &Invocation, children: &[&Invocation], jev: usize) -> String {
    let summary = &invocation.summary;
    let jev_note = |text: String| {
        if jev > 0 && !invocation.jev {
            format!(
                "{text}{}{jev} Jev request{}",
                if text.is_empty() { "" } else { "; " },
                if jev == 1 { "" } else { "s" }
            )
        } else {
            text
        }
    };
    let text = match invocation.component.as_str() {
        "task.requirements" => summary["requirements"]
            .as_u64()
            .map(|n| format!("{n} requirements"))
            .unwrap_or_default(),
        "evidence.probes.planner" => {
            format!(
                "{} host operations",
                children
                    .iter()
                    .filter(|c| c.component == "host.operation")
                    .count()
            )
        }
        "evidence.pack" => summary["chars"]
            .as_u64()
            .map(|n| {
                format!(
                    "a briefing of {} characters",
                    crate::runs_analysis_markdown::thousands(n)
                )
            })
            .unwrap_or_default(),
        "exec.explore" => summary["steps"]
            .as_u64()
            .map(|n| format!("{n} steps"))
            .unwrap_or_default(),
        "accept.define" => {
            let gaps: Vec<String> = summary["gaps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|gap| text(gap, "/requirement"))
                .collect();
            format!(
                "froze `{}`: {} tests{}",
                text(summary, "/status").unwrap_or_else(|| "?".to_owned()),
                summary["tests"].as_u64().unwrap_or(0),
                if gaps.is_empty() {
                    String::new()
                } else {
                    format!(", gaps {}", gaps.join(", "))
                }
            )
        }
        "accept.run" => format!(
            "{} of {} green{}",
            summary["passed"].as_u64().unwrap_or(0),
            summary["total"].as_u64().unwrap_or(0),
            if summary["complete"] == false && summary["green"] == true {
                ", suite incomplete"
            } else {
                ""
            }
        ),
        "microluna.session" => {
            let status = text(summary, "/status").unwrap_or_default();
            let said = text(summary, "/summary").unwrap_or_default();
            let said = said.split("\n\nAnswer:").next().unwrap_or(&said);
            format!(
                "{} turns, {status}: {}",
                summary["turns"].as_u64().unwrap_or(0),
                clip(said, 140)
            )
        }
        "verify.close" => summary["done"]
            .as_f64()
            .map(|p| format!("done at p={p:.2}"))
            .unwrap_or_default(),
        "verify.checks" => {
            let states: Vec<String> = summary["requirements"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(state, n)| format!("{} {state}", n.as_u64().unwrap_or(0)))
                .collect();
            format!(
                "{} scenarios run; requirements {}",
                summary["scenarios"].as_u64().unwrap_or(0),
                states.join(", ")
            )
        }
        "verify.repair" => text(summary, "/skipped")
            .map(|why| format!("skipped: {why}"))
            .or_else(|| {
                summary["brief"]["requirements"].as_array().map(|reqs| {
                    format!(
                        "repaired {}{}",
                        reqs.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", "),
                        if summary["changed"] == true {
                            "; the workspace changed"
                        } else {
                            ""
                        }
                    )
                })
            })
            .unwrap_or_default(),
        "exec.session" => {
            let turns = summary["turns"].as_u64();
            let status = text(summary, "/status").unwrap_or_default();
            match turns {
                Some(turns) => format!("{status}, {turns} turns"),
                None => status,
            }
        }
        _ if invocation.jev => String::new(),
        _ => invocation
            .messages
            .last()
            .map(|m| clip(m, 140))
            .unwrap_or_default(),
    };
    jev_note(text)
}

/// A leaf of the critical path.
struct Leaf {
    start: i64,
    end: i64,
    label: String,
    category: String,
    /// For a session, the share of its time the model took.
    model_share: Option<f64>,
}

fn timeline(records: &Records, episode: &Episode) -> Timeline {
    let origin = episode.origin;
    let at = |ms: i64| ms - origin;
    let by_id: HashMap<&str, &Invocation> = episode
        .invocations
        .iter()
        .map(|i| (i.id.as_str(), i))
        .collect();
    let children = |id: &str| -> Vec<&Invocation> {
        episode
            .invocations
            .iter()
            .filter(|i| i.parent.as_deref() == Some(id))
            .collect()
    };
    let root = episode.invocations.iter().find(|i| i.parent.is_none());
    let mut rows = Vec::new();
    // Rows: the root's children, and inside each, what isn't a Jev
    // request, a host operation, or a component's own internals.
    fn walk<'a>(
        invocation: &'a Invocation,
        depth: usize,
        children: &dyn Fn(&str) -> Vec<&'a Invocation>,
        sessions: &[Session],
        episode: &Episode,
        rows: &mut Vec<(i64, usize, PhaseRow)>,
    ) {
        let kids = children(&invocation.id);
        let end = invocation.end_or(episode.end);
        let placed_session = sessions
            .iter()
            .find(|s| s.invocation.as_deref() == Some(invocation.id.as_str()));
        rows.push((
            invocation.start,
            depth,
            PhaseRow {
                depth,
                start_ms: invocation.start - episode.origin,
                duration_ms: u64::try_from(end - invocation.start).unwrap_or(0),
                phase: match placed_session {
                    Some(session) => format!("{} ({})", invocation.name, session.id),
                    None if invocation.name == invocation.component => invocation.component.clone(),
                    None => format!("{}: {}", invocation.component, invocation.name),
                },
                what: what(
                    invocation,
                    &kids,
                    episode
                        .invocations
                        .iter()
                        .filter(|i| {
                            i.jev
                                && i.component == invocation.component
                                && i.start >= invocation.start
                                && i.start <= end
                        })
                        .count(),
                ),
            },
        ));
        for kid in kids {
            let internal = kid.jev
                || kid.component == "host.operation"
                || (kid
                    .component
                    .starts_with(&format!("{}.", invocation.component))
                    && kid.component != invocation.component);
            if !internal && depth < 3 {
                walk(kid, depth + 1, children, sessions, episode, rows);
            }
        }
    }
    let mut ordered: Vec<(i64, usize, PhaseRow)> = Vec::new();
    if let Some(root) = root {
        for child in children(&root.id) {
            if child.jev {
                continue;
            }
            walk(
                child,
                0,
                &children,
                &records.sessions,
                episode,
                &mut ordered,
            );
        }
    }
    // Sessions no invocation ran, such as the suite writers, beside the
    // invocation that covers them.
    for session in records.sessions.iter().filter(|s| s.invocation.is_none()) {
        let covering = episode
            .invocations
            .iter()
            .filter(|i| {
                i.parent.is_some()
                    && !i.jev
                    && i.start <= session.start
                    && i.end_or(episode.end) >= session.start
            })
            .min_by_key(|i| i.end_or(episode.end) - i.start);
        let depth = covering
            .and_then(|c| {
                ordered.iter().find(|(_, _, row)| {
                    row.start_ms == c.start - origin && row.phase.starts_with(&c.component)
                })
            })
            .map_or(1, |(_, depth, _)| depth + 1);
        ordered.push((
            session.start,
            depth,
            PhaseRow {
                depth,
                start_ms: at(session.start),
                duration_ms: u64::try_from(session.end - session.start).unwrap_or(0),
                phase: format!("{} ({})", session.role.word(), session.id),
                what: session_what(session),
            },
        ));
    }
    ordered.sort_by_key(|(start, depth, _)| (*start, *depth));
    rows.extend(ordered.into_iter().map(|(_, _, row)| row));

    // Sessions.
    let sessions: Vec<SessionRow> = records
        .sessions
        .iter()
        .map(|session| {
            let patches: Vec<&Call> = session
                .calls
                .iter()
                .filter(|c| matches!(c.name.as_str(), "apply_patch" | "write_file"))
                .collect();
            SessionRow {
                session: session.id.clone(),
                role: session.role.word().to_owned(),
                start_ms: at(session.start),
                duration_ms: u64::try_from(session.end - session.start).unwrap_or(0),
                turns: session.turns.len(),
                model_ms: session.model_ms(),
                tool_ms: session.tool_ms(),
                patches: patches.iter().filter(|c| c.ok).count(),
                failed_patches: patches.iter().filter(|c| !c.ok).count(),
                cost_usd: session.cost_usd(),
                ending: session
                    .finish
                    .as_ref()
                    .map_or_else(|| "no finish".to_owned(), |(status, _)| status.clone()),
                summary: session
                    .finish
                    .as_ref()
                    .map(|(_, summary)| clip(summary, 200))
                    .unwrap_or_default(),
                directive: session.directive.clone(),
                pairs: pairs(session),
            }
        })
        .collect();

    // Leaves for the critical path.
    let session_owned: BTreeSet<&str> = records
        .sessions
        .iter()
        .filter_map(|s| s.invocation.as_deref())
        .collect();
    let has_children: BTreeSet<&str> = episode
        .invocations
        .iter()
        .filter_map(|i| i.parent.as_deref())
        .collect();
    let mut leaves: Vec<Leaf> = records
        .sessions
        .iter()
        .map(|session| {
            let span = (session.end - session.start).max(1) as f64;
            Leaf {
                start: session.start,
                end: session.end,
                label: format!("{} ({})", session.id, session.role.word()),
                category: "session".to_owned(),
                model_share: Some((session.model_ms() as f64 / span).min(1.0)),
            }
        })
        .collect();
    for invocation in &episode.invocations {
        if invocation.parent.is_none()
            || has_children.contains(invocation.id.as_str())
            || session_owned.contains(invocation.id.as_str())
        {
            continue;
        }
        let category = if invocation.jev {
            "Jev"
        } else if invocation.component == "accept.run" {
            "host suite runs"
        } else if invocation.component.starts_with("verify.checks") {
            "checks"
        } else {
            "other host work"
        };
        leaves.push(Leaf {
            start: invocation.start,
            end: invocation.end_or(episode.end),
            label: if invocation.jev {
                format!("Jev ({})", invocation.name)
            } else {
                format!("{}: {}", invocation.component, invocation.name)
            },
            category: category.to_owned(),
            model_share: None,
        });
    }
    let innermost = |from: i64, to: i64| -> Option<&Invocation> {
        let middle = from + (to - from) / 2;
        episode
            .invocations
            .iter()
            .filter(|i| !i.jev && i.start <= middle && i.end_or(episode.end) >= middle)
            .min_by_key(|i| i.end_or(episode.end) - i.start)
    };
    let mut path = Vec::new();
    let mut t = episode.end;
    while t > origin {
        let best = leaves
            .iter()
            .filter(|leaf| leaf.start < t)
            .max_by_key(|leaf| (leaf.end.min(t), -(leaf.start)));
        let Some(leaf) = best else {
            path.push(Segment {
                start_ms: 0,
                duration_ms: u64::try_from(t - origin).unwrap_or(0),
                label: "before the first recorded activity".to_owned(),
                category: "idle".to_owned(),
            });
            break;
        };
        let end = leaf.end.min(t);
        if t - end > 50 {
            let owner = innermost(end, t);
            let (label, category) = match owner {
                Some(owner) if owner.parent.is_some() && owner.component != "exec.session" => (
                    format!("host work in {}", owner.component),
                    if owner.component == "accept.define" {
                        "host suite runs"
                    } else {
                        "other host work"
                    },
                ),
                _ => ("between activities".to_owned(), "idle"),
            };
            path.push(Segment {
                start_ms: at(end),
                duration_ms: u64::try_from(t - end).unwrap_or(0),
                label,
                category: category.to_owned(),
            });
        }
        let start = leaf.start.max(origin);
        let length = u64::try_from(end - start).unwrap_or(0);
        path.push(Segment {
            start_ms: at(start),
            duration_ms: length,
            label: leaf.label.clone(),
            category: if leaf.model_share.is_some() {
                "Luna sessions".to_owned()
            } else {
                leaf.category.clone()
            },
        });
        t = start;
    }
    path.reverse();
    // Merge runs of Jev requests and host operations into one segment.
    let mut merged: Vec<Segment> = Vec::new();
    for segment in path {
        if let Some(last) = merged.last_mut()
            && last.category == segment.category
            && matches!(segment.category.as_str(), "Jev" | "checks" | "idle")
        {
            last.duration_ms =
                u64::try_from(segment.start_ms + segment.duration_ms as i64 - last.start_ms)
                    .unwrap_or(0);
            if !last.label.ends_with(" and more") && last.label != segment.label {
                last.label.push_str(" and more");
            }
            continue;
        }
        merged.push(segment);
    }
    let mut categories: BTreeMap<String, u64> = BTreeMap::new();
    for segment in &merged {
        if segment.category == "Luna sessions" {
            let share = leaves
                .iter()
                .find(|leaf| leaf.label == segment.label)
                .and_then(|leaf| leaf.model_share)
                .unwrap_or(0.0);
            let model = (segment.duration_ms as f64 * share).round() as u64;
            *categories
                .entry("Luna model latency".to_owned())
                .or_default() += model;
            *categories
                .entry("tools inside sessions".to_owned())
                .or_default() += segment.duration_ms.saturating_sub(model);
        } else {
            *categories.entry(segment.category.clone()).or_default() += segment.duration_ms;
        }
    }
    let mut categories: Vec<(String, u64)> = categories.into_iter().collect();
    categories.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    // Concurrency.
    let session_ms: u64 = records
        .sessions
        .iter()
        .map(|s| u64::try_from(s.end - s.start).unwrap_or(0))
        .sum();
    let episode_ms = u64::try_from(episode.end - origin).unwrap_or(0);
    let mut events: Vec<(i64, i64)> = records
        .sessions
        .iter()
        .flat_map(|s| [(s.start, 1), (s.end, -1)])
        .collect();
    events.sort_by_key(|(time, delta)| (*time, *delta));
    let (mut live, mut peak, mut last_overlap) = (0_i64, 0_i64, None);
    for (time, delta) in events {
        if delta < 0 && live > 1 {
            last_overlap = Some(time);
        }
        live += delta;
        peak = peak.max(live);
    }
    let first_edit_ms = records
        .sessions
        .iter()
        .filter(|s| s.role.edits_workspace())
        .flat_map(|s| s.calls.iter())
        .filter(|c| c.ok && matches!(c.name.as_str(), "apply_patch" | "write_file"))
        .map(|c| c.at)
        .min()
        .map(at);

    // Cost by phase.
    let phase_of_invocation = |invocation: &Invocation| -> String {
        let mut current = Some(invocation);
        let mut chain = Vec::new();
        while let Some(i) = current {
            chain.push(i);
            current = i.parent.as_deref().and_then(|p| by_id.get(p).copied());
        }
        for i in &chain {
            match i.component.as_str() {
                "accept.define" => return "suite writing".to_owned(),
                "verify.close" => {
                    return if chain.iter().any(|a| a.component == "exec.session") {
                        "joined close".to_owned()
                    } else {
                        "closing check".to_owned()
                    };
                }
                "microluna.session" => return phase_of_session_name(records, i),
                "verify.repair" => return "repair".to_owned(),
                _ => {}
            }
        }
        // The root's child.
        let top = chain.iter().rev().nth(1).copied();
        let during_gap = records.sessions.iter().any(|s| {
            s.role == Role::GapWriter && s.start <= invocation.start && s.end >= invocation.start
        });
        if during_gap {
            return "gap round".to_owned();
        }
        match top.map(|t| t.component.as_str()) {
            Some("task.requirements" | "exec.explore") => "preparation".to_owned(),
            Some(c) if c.starts_with("evidence") => "preparation".to_owned(),
            Some("exec.session") => {
                if invocation.component == "microluna.handoff" {
                    "hand-offs".to_owned()
                } else {
                    format!("dispatch: {}", invocation.component)
                }
            }
            Some("verify.close") => "closing check".to_owned(),
            Some("verify.checks") => "checks".to_owned(),
            Some(c) => c.to_owned(),
            None => "other".to_owned(),
        }
    };
    let mut phases: Vec<PhaseCost> = Vec::new();
    let mut add = |phase: String, luna: f64, jev: f64, requests: usize| match phases
        .iter_mut()
        .find(|p| p.phase == phase)
    {
        Some(found) => {
            found.luna_usd += luna;
            found.jev_usd += jev;
            found.jev_requests += requests;
        }
        None => phases.push(PhaseCost {
            phase,
            luna_usd: luna,
            jev_usd: jev,
            jev_requests: requests,
        }),
    };
    let rate = crate::runs_learning::USD_PER_MILLION_INPUT / 1_000_000.0;
    let mut items: Vec<(i64, String, f64, f64, usize)> = Vec::new();
    for invocation in episode.invocations.iter().filter(|i| i.jev) {
        let usd = invocation
            .cost_usd
            .unwrap_or(invocation.jev_input_tokens as f64 * rate);
        items.push((
            invocation.start,
            phase_of_invocation(invocation),
            0.0,
            usd,
            1,
        ));
    }
    for session in &records.sessions {
        let phase = match session.role {
            Role::Writer => "suite writing".to_owned(),
            Role::GapWriter => "gap round".to_owned(),
            Role::Repair => "repair".to_owned(),
            _ => session
                .invocation
                .as_deref()
                .and_then(|id| by_id.get(id))
                .map_or_else(|| session.id.clone(), |i| phase_of_session_name(records, i)),
        };
        items.push((session.start, phase, session.cost_usd(), 0.0, 0));
    }
    items.sort_by_key(|(start, ..)| *start);
    for (_, phase, luna, jev, requests) in items {
        add(phase, luna, jev, requests);
    }

    Timeline {
        episode_ms,
        phases: rows,
        sessions,
        critical_path: merged,
        categories,
        concurrency: if episode_ms == 0 {
            0.0
        } else {
            session_ms as f64 / episode_ms as f64
        },
        peak_sessions: usize::try_from(peak).unwrap_or(0),
        session_ms,
        serial_tail_ms: last_overlap.and_then(|time| u64::try_from(episode.end - time).ok()),
        first_edit_ms,
        cost_by_phase: phases,
    }
}

fn phase_of_session_name(records: &Records, invocation: &Invocation) -> String {
    records
        .sessions
        .iter()
        .find(|s| s.invocation.as_deref() == Some(invocation.id.as_str()))
        .map_or_else(
            || invocation.name.clone(),
            |s| match s.role {
                Role::Repair => "repair".to_owned(),
                role => format!("{} ({})", invocation.name, role.word()),
            },
        )
}

fn session_what(session: &Session) -> String {
    let patches = session
        .calls
        .iter()
        .filter(|c| c.ok && matches!(c.name.as_str(), "apply_patch" | "write_file"))
        .count();
    let ending = session
        .finish
        .as_ref()
        .map_or("no finish".to_owned(), |(status, _)| status.clone());
    format!(
        "{} turns, {patches} edits, {ending}{}",
        session.turns.len(),
        session
            .finish
            .as_ref()
            .map(|(_, summary)| format!(": {}", clip(summary, 120)))
            .unwrap_or_default()
    )
}

/// Each turn as `model/tool` seconds, rounded.
fn pairs(session: &Session) -> String {
    let mut out = Vec::new();
    let mut calls = session.calls.iter().peekable();
    let turns = &session.turns;
    for (index, turn) in turns.iter().enumerate() {
        let next = turns.get(index + 1).map_or(i64::MAX, |t| t.at);
        let mut tool = 0;
        while let Some(call) = calls.peek() {
            if call.at > next {
                break;
            }
            tool += call.ms;
            calls.next();
        }
        out.push(format!(
            "{}/{}",
            (turn.ms as f64 / 1000.0).round(),
            (tool as f64 / 1000.0).round()
        ));
    }
    out.join(" ")
}

// ---------------------------------------------------------------------------
// Reversals
// ---------------------------------------------------------------------------

/// One file's change in one edit.
struct Change {
    file: String,
    removed: Vec<String>,
    added: Vec<String>,
}

/// A line worth tracking: long enough to be specific.
fn significant(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let words: String = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    (words.chars().filter(|c| c.is_alphanumeric()).count() >= 6).then_some(words)
}

fn relative(path: &str, repository: &str) -> String {
    let path = path.trim();
    let repository = repository.trim_end_matches('/');
    if !repository.is_empty()
        && let Some(rest) = path.strip_prefix(repository)
    {
        return rest.trim_start_matches('/').to_owned();
    }
    path.trim_start_matches("./").to_owned()
}

/// The changes an `apply_patch` patch makes, per file.
fn patch_changes(patch: &str, repository: &str) -> Vec<Change> {
    let mut changes: Vec<Change> = Vec::new();
    let mut adding = false;
    for line in patch.lines() {
        if let Some(path) = line
            .strip_prefix("*** Update File: ")
            .or_else(|| line.strip_prefix("*** Add File: "))
            .or_else(|| line.strip_prefix("*** Delete File: "))
        {
            adding = line.starts_with("*** Add File: ");
            changes.push(Change {
                file: relative(path, repository),
                removed: Vec::new(),
                added: Vec::new(),
            });
            continue;
        }
        if line.starts_with("*** ") {
            continue;
        }
        let Some(change) = changes.last_mut() else {
            continue;
        };
        if let Some(rest) = line.strip_prefix('+') {
            change.added.extend(significant(rest));
        } else if let Some(rest) = line.strip_prefix('-')
            && !adding
        {
            change.removed.extend(significant(rest));
        }
    }
    // A line both removed and added in one change only moved.
    for change in &mut changes {
        let added: BTreeSet<String> = change.added.iter().cloned().collect();
        let removed: BTreeSet<String> = change.removed.iter().cloned().collect();
        change.added.retain(|line| !removed.contains(line));
        change.removed.retain(|line| !added.contains(line));
    }
    changes
}

/// The share of two lines' words they have in common.
fn similarity(a: &str, b: &str) -> f64 {
    let words = |line: &str| -> BTreeSet<String> {
        line.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|w| w.chars().count() > 1)
            .map(str::to_lowercase)
            .collect()
    };
    let (a, b) = (words(a), words(b));
    let union = a.union(&b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(&b).count() as f64 / union as f64
    }
}

/// The red tests a session's brief names: `T10 (R4) is red`.
fn red_tests(brief: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in brief.lines() {
        if !line.contains("is red") {
            continue;
        }
        let first = line.trim_start().trim_start_matches(['-', '*', ' ']);
        let id: String = first.chars().take_while(|c| c.is_alphanumeric()).collect();
        if id.starts_with('T')
            && id.len() > 1
            && id[1..].chars().all(|c| c.is_ascii_digit())
            && !found.contains(&id)
        {
            found.push(id);
        }
    }
    found
}

/// The function or class around `line` in a source file.
fn enclosing(source: &str, line: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| significant(l).as_deref() == Some(line))?;
    for candidate in lines[..=at].iter().rev() {
        let trimmed = candidate.trim_start();
        for prefix in [
            "def ",
            "async def ",
            "class ",
            "fn ",
            "pub fn ",
            "function ",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}

/// The guards: tests that passed on the untouched workspace.
fn guards(records: &Records) -> BTreeSet<String> {
    let mut guards = BTreeSet::new();
    if let Some(accept) = &records.accept {
        for test in accept
            .pointer("/start/tests")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if test["green"] == true
                && let Some(id) = text(test, "/id")
            {
                guards.insert(id);
            }
        }
        for (id, judged) in accept
            .pointer("/detail/judged/tests")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if judged["green_at_start"] == true {
                guards.insert(id.clone());
            }
        }
    }
    guards
}

/// Per earlier session, later session, and file: when the later one
/// first removed the earlier one's lines, those lines, and what the later
/// one added in the same changes.
type Undone = BTreeMap<(usize, usize, String), (i64, Vec<String>, Vec<String>)>;

fn reversals(records: &Records) -> (Vec<Reversal>, Vec<GuardEdit>) {
    let origin = records.episode.as_ref().map_or(0, |e| e.origin);
    let guards = guards(records);
    let final_root = |repository: &str| {
        records
            .trial
            .join("artifacts")
            .join(repository.trim_start_matches('/'))
    };
    // Every workspace edit, in time order.
    let mut edits: Vec<(i64, usize, Vec<Change>)> = Vec::new();
    for (index, session) in records.sessions.iter().enumerate() {
        if !session.role.edits_workspace() {
            continue;
        }
        for call in session.calls.iter().filter(|c| c.ok) {
            let changes = match call.name.as_str() {
                "apply_patch" => call.arguments["patch"]
                    .as_str()
                    .map(|p| patch_changes(p, &session.repository))
                    .unwrap_or_default(),
                "write_file" => {
                    let path = call.arguments["path"].as_str().unwrap_or_default();
                    let content = call.arguments["content"].as_str().unwrap_or_default();
                    vec![Change {
                        file: relative(path, &session.repository),
                        removed: Vec::new(),
                        added: content.lines().filter_map(significant).collect(),
                    }]
                }
                _ => Vec::new(),
            };
            if !changes.is_empty() {
                edits.push((call.at, index, changes));
            }
        }
    }
    edits.sort_by_key(|(at, index, _)| (*at, *index));
    // Who added each line still there, and who removed each line, with
    // whose it was.
    let mut owner: HashMap<(String, String), usize> = HashMap::new();
    let mut removed_by: HashMap<(String, String), (usize, Option<usize>)> = HashMap::new();
    // (earlier, later, file) -> (first time, lines undone, lines removed,
    // lines added by the later session in that file)
    let mut undone: Undone = BTreeMap::new();
    // (earlier, undoer, file) -> restorer
    let mut restored: HashMap<(usize, usize, String), usize> = HashMap::new();
    let mut added_by: HashMap<(usize, String), usize> = HashMap::new();
    // The lines each session removed from each file: the state before it.
    let mut removed_of: HashMap<(usize, String), Vec<String>> = HashMap::new();
    for (at, session, changes) in &edits {
        for change in changes {
            let mut undid: BTreeSet<usize> = BTreeSet::new();
            removed_of
                .entry((*session, change.file.clone()))
                .or_default()
                .extend(change.removed.iter().cloned());
            for line in &change.removed {
                let key = (change.file.clone(), line.clone());
                let previous = owner.remove(&key);
                if let Some(earlier) = previous
                    && earlier != *session
                {
                    let entry = undone
                        .entry((earlier, *session, change.file.clone()))
                        .or_insert_with(|| (*at, Vec::new(), Vec::new()));
                    entry.1.push(line.clone());
                    undid.insert(earlier);
                }
                removed_by.insert(key, (*session, previous));
            }
            for line in &change.added {
                let key = (change.file.clone(), line.clone());
                if let Some((undoer, Some(earlier))) = removed_by.get(&key).copied()
                    && undoer != *session
                    && earlier != undoer
                {
                    restored
                        .entry((earlier, undoer, change.file.clone()))
                        .or_insert(*session);
                }
                owner.insert(key, *session);
                *added_by.entry((*session, change.file.clone())).or_default() += 1;
            }
            // What replaced the undone lines: this change's own additions.
            for ((earlier, later, file), entry) in undone.iter_mut() {
                if later == session && *file == change.file && undid.contains(earlier) {
                    for line in &change.added {
                        if !entry.2.contains(line) {
                            entry.2.push(line.clone());
                        }
                    }
                }
            }
        }
    }
    let failing: Vec<&VerifierTest> = records.verifier.iter().filter(|t| !t.passed()).collect();
    let passed = records.run.reward.is_some_and(|r| r >= 1.0);
    let mut reversals = Vec::new();
    for ((earlier, later, file), (at, lines, added)) in undone {
        let earlier_added = added_by.get(&(earlier, file.clone())).copied().unwrap_or(0);
        if lines.len() < 2 && (lines.is_empty() || lines.len() * 2 < earlier_added) {
            continue;
        }
        // A restore that undid the undoer is reported with the first
        // reversal, not as one of its own.
        if restored.iter().any(|((_, undoer, f), restorer)| {
            *undoer == earlier && *f == file && *restorer == later
        }) {
            continue;
        }
        // A revert puts back something like what the earlier session
        // replaced; a rewrite replaces its lines with new code.
        let before = removed_of
            .get(&(earlier, file.clone()))
            .cloned()
            .unwrap_or_default();
        let resembling = added
            .iter()
            .filter(|line| before.iter().any(|old| similarity(line, old) >= 0.5))
            .count();
        let kind = if (added.is_empty() && before.is_empty())
            || (resembling > 0 && resembling * 3 >= added.len())
        {
            "revert"
        } else {
            "rewrite"
        };
        let earlier_session = &records.sessions[earlier];
        let later_session = &records.sessions[later];
        let final_source =
            std::fs::read_to_string(final_root(&later_session.repository).join(&file))
                .unwrap_or_default();
        let mut symbols: Vec<String> = lines
            .iter()
            .chain(added.iter())
            .filter_map(|line| enclosing(&final_source, line))
            .collect();
        symbols.sort();
        symbols.dedup();
        let restorer = restored.get(&(earlier, later, file.clone())).copied();
        let in_final = lines
            .iter()
            .filter(|line| {
                final_source
                    .lines()
                    .any(|l| significant(l).as_deref() == Some(line.as_str()))
            })
            .count();
        let drivers = red_tests(&later_session.brief);
        let driving_guards: Vec<String> = drivers
            .iter()
            .filter(|t| guards.contains(*t))
            .cloned()
            .collect();
        let better = if passed && in_final * 2 >= lines.len() && !final_source.is_empty() {
            Some(format!(
                "the final workspace, which passed the verifier, has {in_final} of the {} lines `{}` removed{}",
                lines.len(),
                later_session.id,
                restorer
                    .map(|r| format!(", because `{}` put them back", records.sessions[r].id))
                    .unwrap_or_default()
            ))
        } else if !passed && in_final == 0 {
            failing
                .iter()
                .find(|test| {
                    symbols.iter().any(|symbol| {
                        test.name.contains(symbol.as_str())
                            || test.source.contains(&format!("{symbol}("))
                    })
                })
                .map(|test| {
                    format!(
                        "the verifier failed `{}`, which exercises `{}`, the code `{}` changed back",
                        test.name,
                        symbols.join("`, `"),
                        later_session.id
                    )
                })
        } else {
            None
        };
        reversals.push(Reversal {
            kind: kind.to_owned(),
            file,
            earlier: earlier_session.id.clone(),
            later: later_session.id.clone(),
            at_ms: at - origin,
            undone: lines.len(),
            earlier_added,
            symbols,
            restored_by: restorer.map(|r| records.sessions[r].id.clone()),
            drivers,
            guards: driving_guards,
            better_then_reverted: better,
            removed: lines.iter().take(4).cloned().collect(),
            added: added.iter().take(4).cloned().collect(),
        });
    }
    // Guards that turned red and made a session edit.
    let mut guard_edits = Vec::new();
    let runs: Vec<&SuiteRun> = records.episode.iter().flat_map(|e| &e.suite_runs).collect();
    for (index, session) in records.sessions.iter().enumerate() {
        if !session.role.edits_workspace() {
            continue;
        }
        let named = red_tests(&session.brief);
        let edited: BTreeSet<String> = edits
            .iter()
            .filter(|(_, s, _)| *s == index)
            .flat_map(|(_, _, changes)| changes.iter().map(|c| c.file.clone()))
            .collect();
        if edited.is_empty() {
            continue;
        }
        for test in named.iter().filter(|t| guards.contains(*t)) {
            let red_in = runs
                .iter()
                .filter(|run| run.end <= session.start)
                .rev()
                .find(|run| run.tests.iter().any(|(id, green, ..)| id == test && !green))
                .map_or_else(
                    || "a suite run".to_owned(),
                    |run| format!("the run `{}`", run.label),
                );
            guard_edits.push(GuardEdit {
                test: test.clone(),
                red_in,
                session: session.id.clone(),
                files: edited.iter().cloned().collect(),
            });
        }
    }
    (reversals, guard_edits)
}

// ---------------------------------------------------------------------------
// Anomalies
// ---------------------------------------------------------------------------

fn anomalies(records: &Records, analysis: &Analysis) -> Vec<Anomaly> {
    let mut found = Vec::new();
    let mut push = |kind: &str, text: String, seconds: Option<f64>| {
        found.push(Anomaly {
            kind: kind.to_owned(),
            text: format!("{}.", text.trim_end_matches('.')),
            seconds,
        });
    };
    let runs: Vec<&SuiteRun> = records.episode.iter().flat_map(|e| &e.suite_runs).collect();
    // The suite against the verifier.
    if let Some(last) = runs.iter().rev().find(|run| !run.label.contains("only"))
        && last.green
        && records.run.reward.is_some_and(|r| r < 1.0)
    {
        push(
            "false green",
            format!(
                "The acceptance suite's last run (`{}`) was green, {} of {}, and the verifier failed the trial",
                last.label, last.passed, last.total
            ),
            None,
        );
    }
    for session in &records.sessions {
        let id = &session.id;
        match &session.finish {
            Some((status, summary)) if status == "blocked" => push(
                "blocked session",
                format!("`{id}` ended blocked: {}", clip(summary, 160)),
                None,
            ),
            Some(_) => {}
            None => push(
                "no finish",
                format!(
                    "`{id}` ({}) ended after {} turns without calling finish, as at a turn limit",
                    session.role.word(),
                    session.turns.len()
                ),
                None,
            ),
        }
        let failed_patches: Vec<&Call> = session
            .calls
            .iter()
            .filter(|c| !c.ok && matches!(c.name.as_str(), "apply_patch" | "write_file"))
            .collect();
        if let Some(first) = failed_patches.first() {
            push(
                "failed patch",
                format!(
                    "`{id}`: {} of its edits failed; the first: {}",
                    failed_patches.len(),
                    clip(&first.output, 140)
                ),
                None,
            );
        }
        // Refused reads and commands the container doesn't have. A
        // command that fails a test isn't friction.
        let friction: Vec<&Call> = session
            .calls
            .iter()
            .filter(|c| match c.name.as_str() {
                "read_file" => !c.ok,
                "run_command" => {
                    c.arguments["command"]
                        .as_str()
                        .is_some_and(|c| c.trim().is_empty())
                        || matches!(c.exit, Some(126 | 127))
                        || (!c.ok && c.exit.is_none())
                }
                _ => false,
            })
            .collect();
        if let Some(first) = friction.first() {
            let what = match first.name.as_str() {
                "read_file" => clip(&first.output, 110),
                _ if first.command().is_some_and(|c| c.trim().is_empty()) => {
                    "an empty command".to_owned()
                }
                _ => format!(
                    "`{}` exited {}",
                    clip(first.command().unwrap_or_default(), 60),
                    first
                        .exit
                        .map_or_else(|| "without a code".to_owned(), |c| c.to_string())
                ),
            };
            push(
                "tool friction",
                format!(
                    "`{id}`: {} tool call{} refused or couldn't run; the first, {}: {what}",
                    friction.len(),
                    if friction.len() == 1 {
                        " was"
                    } else {
                        "s were"
                    },
                    first.name
                ),
                None,
            );
        }
        // Repeated identical failures.
        let mut failures: BTreeMap<(String, String), usize> = BTreeMap::new();
        for call in session.calls.iter().filter(|c| c.name == "run_command") {
            let code = call.exit.unwrap_or(0);
            if code == 0 || call.runs_suite() {
                continue;
            }
            let first_line = call
                .output
                .lines()
                .skip(1)
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .to_owned();
            *failures
                .entry((
                    call.command().unwrap_or_default().trim().to_owned(),
                    first_line,
                ))
                .or_default() += 1;
        }
        for ((command, output), count) in failures {
            if count >= 2 {
                push(
                    "repeated failure",
                    format!(
                        "`{id}` ran `{}` {count} times, failing the same way each time: {}",
                        clip(&command, 80),
                        clip(&output, 100)
                    ),
                    None,
                );
            }
        }
        // Hung commands.
        for call in session.calls.iter().filter(|c| c.name == "run_command") {
            let limit = call.arguments["timeout_seconds"].as_f64().unwrap_or(0.0);
            // Microluna's bound is in milliseconds when it's this large.
            let limit_ms = if limit > 10_000.0 {
                limit
            } else {
                limit * 1000.0
            };
            let lowered = call.output.to_lowercase();
            if (limit_ms > 0.0 && call.ms as f64 >= limit_ms * 0.95)
                || lowered.starts_with("[timed out")
                || lowered.contains("command timed out")
            {
                push(
                    "hung command",
                    format!(
                        "`{id}`: `{}` ran {:.0} s and was stopped",
                        clip(call.command().unwrap_or_default(), 80),
                        call.ms as f64 / 1000.0
                    ),
                    Some(call.ms as f64 / 1000.0),
                );
            }
        }
    }
    for run in &runs {
        let killed: Vec<&str> = run
            .tests
            .iter()
            .filter(|(_, _, killed, _)| *killed)
            .map(|(id, ..)| id.as_str())
            .collect();
        if !killed.is_empty() {
            push(
                "hung test",
                format!(
                    "The suite run `{}` killed {} at its bound",
                    run.label,
                    killed.join(", ")
                ),
                None,
            );
        }
    }
    // Workspace edits and suite runs, in time order, to find runs of an
    // unchanged workspace and edits no run saw.
    let mut edit_times: Vec<(i64, &str)> = records
        .sessions
        .iter()
        .filter(|s| s.role.edits_workspace())
        .flat_map(|s| {
            s.calls
                .iter()
                .filter(|c| c.ok && matches!(c.name.as_str(), "apply_patch" | "write_file"))
                .map(move |c| (c.at, s.id.as_str()))
        })
        .collect();
    edit_times.sort_unstable();
    // (start, end, label, counts as a repeat, sets the baseline)
    let mut all_runs: Vec<(i64, i64, String, bool, bool)> = runs
        .iter()
        .map(|run| {
            let partial = run.label.contains("only");
            let first = run.label.contains("start") || run.label.contains("snapshot");
            (
                run.start,
                run.end,
                format!("the host's run `{}`", run.label),
                !partial && !first,
                !partial && !run.label.contains("snapshot"),
            )
        })
        .collect();
    for session in records.sessions.iter().filter(|s| s.role.edits_workspace()) {
        for call in session.calls.iter().filter(|c| c.runs_suite()) {
            let command = call.command().unwrap_or_default();
            let partial = command
                .split("run.sh")
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .is_some_and(|word| {
                    word.len() > 1
                        && word.starts_with('T')
                        && word[1..].chars().all(|c| c.is_ascii_digit())
                });
            all_runs.push((
                call.at - i64::try_from(call.ms).unwrap_or(0),
                call.at,
                format!("`{}`'s own run", session.id),
                !partial,
                !partial,
            ));
        }
    }
    all_runs.sort_by_key(|(start, ..)| *start);
    let mut previous: Option<i64> = None;
    let mut wasted = 0.0;
    let mut repeats = Vec::new();
    for (start, end, label, counts, baseline) in &all_runs {
        if let Some(previous) = previous
            && *counts
            && !edit_times
                .iter()
                .any(|(at, _)| *at > previous && *at <= *start)
        {
            let seconds = (end - start) as f64 / 1000.0;
            wasted += seconds;
            repeats.push(format!("{label} ({seconds:.1} s)"));
        }
        if *baseline {
            previous = Some(*end);
        }
    }
    if !repeats.is_empty() {
        push(
            "repeated suite run",
            format!(
                "{} full suite runs repeated a run on a workspace nothing had changed since: {}",
                repeats.len(),
                repeats.join("; ")
            ),
            Some(wasted),
        );
    }
    if let (Some(last_run), Some((last_edit, _))) = (
        all_runs.iter().filter(|r| r.4).map(|r| r.1).max(),
        edit_times.last(),
    ) && *last_edit > last_run
    {
        let after: BTreeSet<&str> = edit_times
            .iter()
            .filter(|(at, _)| *at > last_run)
            .map(|(_, who)| *who)
            .collect();
        push(
            "unchecked edits",
            format!(
                "{} changed the workspace after the suite's last full run, and nothing reran the suite: the final workspace's suite result is unknown",
                after
                    .into_iter()
                    .map(|who| format!("`{who}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None,
        );
    }
    // The checks' budget.
    if let Some(episode) = &records.episode {
        for invocation in episode
            .invocations
            .iter()
            .filter(|i| i.component == "verify.checks.select")
        {
            let admitted = invocation.summary["admitted"].as_u64().unwrap_or(0);
            let selected = invocation.summary["selected"]
                .as_array()
                .map_or(0, Vec::len);
            let budget_skips = invocation.summary["skipped"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|s| text(s, "/why").is_some_and(|w| w.contains("budget")))
                .count();
            if budget_skips > 0 {
                push(
                    "checks cut by budget",
                    format!(
                        "The checks ran {selected} of {admitted} admitted scenarios; {budget_skips} were skipped because their bound exceeded the time left"
                    ),
                    None,
                );
            }
        }
    }
    // Idle gaps on the critical path.
    if let Some(timeline) = &analysis.timeline {
        for segment in timeline
            .critical_path
            .iter()
            .filter(|s| s.category == "idle" && s.duration_ms as i64 >= IDLE_MS)
        {
            push(
                "idle gap",
                format!(
                    "Nothing recorded ran for {:.1} s from {}",
                    segment.duration_ms as f64 / 1000.0,
                    offset(segment.start_ms)
                ),
                Some(segment.duration_ms as f64 / 1000.0),
            );
        }
    }
    if let (Some(harbor), Some(missing)) = (analysis.cost.harbor_usd, &analysis.cost.harbor_missing)
    {
        push(
            "cost mismatch",
            format!(
                "Harbor reports {} and the true total is {}; Harbor's figure leaves out {missing}",
                usd(harbor),
                usd(analysis.cost.total_usd)
            ),
            None,
        );
    }
    for reversal in analysis.reversals.iter().filter(|r| r.kind == "revert") {
        push(
            "reversal",
            format!(
                "`{}` reverted {} of `{}`'s lines in `{}`{}",
                reversal.later,
                reversal.undone,
                reversal.earlier,
                reversal.file,
                reversal
                    .restored_by
                    .as_ref()
                    .map(|r| format!(", and `{r}` restored them"))
                    .unwrap_or_default()
            ),
            None,
        );
    }
    found
}

// ---------------------------------------------------------------------------
// Fable 5.1
// ---------------------------------------------------------------------------

/// Where the public Fable 5.1 manifest is: the replay cache's copy, or the
/// checkout's.
#[must_use]
pub fn fable_manifest_path() -> PathBuf {
    let cached = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".openagents/gym/public-replays/manifest.json"))
        .filter(|path| path.is_file());
    cached.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/terminal-bench/reference/fable-5.1-replays.json")
    })
}

const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

fn fable_of(manifest: &Value, task: &str) -> Option<Fable> {
    let attempts: Vec<&Value> = manifest["trials"]
        .as_array()?
        .iter()
        .filter(|t| t["task"] == task)
        .collect();
    if attempts.is_empty() {
        return None;
    }
    let seconds = |t: &Value| -> Option<f64> {
        let start = timestamp_ms(t["started_at"].as_str()?)?;
        let end = timestamp_ms(t["finished_at"].as_str()?)?;
        Some((end - start) as f64 / 1000.0)
    };
    let passed = |t: &Value| t["reward"].as_f64().is_some_and(|r| r >= 1.0);
    let mean = |values: &[f64]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        }
    };
    let cheapest_pass = attempts
        .iter()
        .filter(|t| passed(t))
        .filter(|t| t["cost_usd"].is_number())
        .min_by(|a, b| {
            a["cost_usd"]
                .as_f64()
                .partial_cmp(&b["cost_usd"].as_f64())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|t| FableAttempt {
            id: t["id"].as_str().unwrap_or_default().to_owned(),
            effort: t["effort"].as_str().unwrap_or_default().to_owned(),
            cost_usd: t["cost_usd"].as_f64().unwrap_or(0.0),
            seconds: seconds(t).unwrap_or(0.0),
            steps: t["steps"].as_u64().unwrap_or(0),
        });
    let tier = EFFORTS.iter().find_map(|effort| {
        let members: Vec<&&Value> = attempts.iter().filter(|t| t["effort"] == *effort).collect();
        (!members.is_empty()).then(|| FableTier {
            effort: (*effort).to_owned(),
            attempts: members.len(),
            passes: members.iter().filter(|t| passed(t)).count(),
            mean_usd: mean(
                &members
                    .iter()
                    .filter_map(|t| t["cost_usd"].as_f64())
                    .collect::<Vec<_>>(),
            ),
            mean_sec: mean(
                &members
                    .iter()
                    .filter_map(|t| seconds(t))
                    .collect::<Vec<_>>(),
            ),
        })
    });
    Some(Fable {
        attempts: attempts.len(),
        passes: attempts.iter().filter(|t| passed(t)).count(),
        cheapest_pass,
        tier,
        all_mean_usd: mean(
            &attempts
                .iter()
                .filter_map(|t| t["cost_usd"].as_f64())
                .collect::<Vec<_>>(),
        ),
        all_mean_sec: mean(
            &attempts
                .iter()
                .filter_map(|t| seconds(t))
                .collect::<Vec<_>>(),
        ),
    })
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// `$0.0357`, `$0.00755`, `$0.87`.
#[must_use]
pub fn usd(amount: f64) -> String {
    if amount == 0.0 {
        "$0".to_owned()
    } else if amount.abs() < 0.01 {
        format!("${amount:.5}")
    } else if amount.abs() < 0.1 {
        format!("${amount:.4}")
    } else {
        format!("${amount:.2}")
    }
}

/// An offset from the episode's start: `03:30.5`.
#[must_use]
pub fn offset(ms: i64) -> String {
    let ms = ms.max(0);
    let tenths = (ms + 50) / 100;
    format!(
        "{:02}:{:02}.{}",
        tenths / 600,
        (tenths / 10) % 60,
        tenths % 10
    )
}

/// A span: `0.37 s`, `27.5 s`, `3:27.7`.
#[must_use]
pub fn span(ms: u64) -> String {
    if ms < 1_000 {
        format!("{:.2} s", ms as f64 / 1000.0)
    } else if ms < 60_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        let tenths = (ms + 50) / 100;
        format!("{}:{:02}.{}", tenths / 600, (tenths / 10) % 60, tenths % 10)
    }
}

/// A long span in words: `17 min 14 s`.
#[must_use]
pub fn long(ms: u64) -> String {
    let seconds = (ms + 500) / 1000;
    match seconds {
        0..60 => format!("{seconds} s"),
        60..3600 => format!("{} min {} s", seconds / 60, seconds % 60),
        _ => format!("{} h {} min", seconds / 3600, (seconds % 3600) / 60),
    }
}

// ---------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------

const USAGE: &str = "\
gym runs analyze: one run's analysis, computed from its records.

Usage:
  gym runs analyze RUN [--json] [--write] [--no-jev] [--recorded FILE]
                       [--record FILE] [--cache-dir PATH] [--fable PATH]

RUN is a job name, job/trial, a trial name, or a piece of a job name that
only one job has. The analysis covers the verifier's result with each
failing assertion, the true total cost against Harbor's, the timeline of
every component and Luna session with the critical path, the acceptance
suite against the verifier's tests, sessions that undid an earlier
session's change, anomalies, and the cheapest passing Fable 5.1 attempt.

Code computes every number. Jev judges only whether an acceptance test
checks what a verifier test checks, for the pairs the rules leave open;
answers are cached under ~/.openagents/gym/analysis (--cache-dir PATH).
--no-jev uses cached answers only. --recorded FILE replays answers, and
--record FILE writes the answers used.

It prints Markdown, or JSON with --json. --write also keeps analysis.md
and analysis.json in the trial directory and prints their paths.
--jobs-dir PATH and --traces-dir PATH read other directories; --fable
PATH reads another public-attempt manifest.";

/// Where cached Jev answers are kept: `~/.openagents/gym/analysis`.
#[must_use]
pub fn default_cache_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/analysis"))
}

/// Analyzes `run`: computes the numbers, asks Jev what the cache doesn't
/// hold, and returns the analysis.
///
/// # Errors
///
/// Returns a message when the runtime for Jev's requests can't start.
pub fn analyze(
    run: &Run,
    judge: &Judge,
    cache_dir: Option<PathBuf>,
    fable: Option<&Value>,
    record: Option<&mut crate::runs_learning::Recorded>,
) -> Result<Analysis, String> {
    let records = Records::load(run);
    let mut analysis = compute(&records, fable);
    let mut cache = suite::Cache::open(cache_dir);
    let (section, jev) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start a runtime: {error}"))?
        .block_on(suite::section(&records, judge, &mut cache, record));
    analysis.jev = jev;
    if let Some(section) = &section {
        for anomaly in suite::anomalies(section) {
            analysis.anomalies.push(anomaly);
        }
    }
    analysis.suite = section;
    Ok(analysis)
}

/// Analyzes `run` with cached Jev answers only, for the Runs pane.
#[must_use]
pub fn analyze_offline(run: &Run) -> Analysis {
    let manifest = read_json(&fable_manifest_path());
    analyze(
        run,
        &Judge::Off("the pane reads cached answers only".to_owned()),
        default_cache_dir(),
        manifest.as_ref(),
        None,
    )
    .unwrap_or_else(|_| compute(&Records::load(run), manifest.as_ref()))
}

/// The stored analysis beside `run`, when `--write` kept one.
#[must_use]
pub fn stored_markdown(run: &Run) -> Option<String> {
    std::fs::read_to_string(run.files.dir.join(MARKDOWN_FILE)).ok()
}

/// Writes `analysis.md` and `analysis.json` into the trial directory.
///
/// # Errors
///
/// Returns a message when a file can't be written.
pub fn write(run: &Run, analysis: &Analysis) -> Result<(PathBuf, PathBuf), String> {
    let dir = &run.files.dir;
    let markdown = dir.join(MARKDOWN_FILE);
    let json_path = dir.join(JSON_FILE);
    let atomic = |path: &Path, text: &str| {
        let temporary = path.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&temporary, text)
            .and_then(|()| std::fs::rename(&temporary, path))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    };
    atomic(&markdown, &crate::runs_analysis_markdown::render(analysis))?;
    atomic(
        &json_path,
        &format!(
            "{}\n",
            serde_json::to_string_pretty(analysis).map_err(|e| e.to_string())?
        ),
    )?;
    Ok((markdown, json_path))
}

/// `gym runs analyze`.
///
/// # Errors
///
/// Returns the usage text when the arguments don't parse, and a message
/// when the run isn't found or a file can't be written.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let mut sources = Sources::standard();
    let (mut json_out, mut keep, mut no_jev) = (false, false, false);
    let mut name: Option<String> = None;
    let mut cache_dir = default_cache_dir();
    let mut fable_path = fable_manifest_path();
    let (mut recorded, mut record): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut index = 0;
    let value = |index: usize| args.get(index + 1).cloned().ok_or_else(|| USAGE.to_owned());
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json_out = true,
            "--write" => keep = true,
            "--no-jev" => no_jev = true,
            "--cache-dir" => {
                cache_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--fable" => {
                fable_path = PathBuf::from(value(index)?);
                index += 1;
            }
            "--recorded" => {
                recorded = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--record" => {
                record = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--jobs-dir" => {
                sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-traces" => sources.traces = None,
            "--no-jobs" => sources.jobs = None,
            "--help" | "-h" => {
                writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            other if !other.starts_with("--") && name.is_none() => name = Some(other.to_owned()),
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    let name = name.ok_or_else(|| USAGE.to_owned())?;
    let catalog = Catalog::load(sources);
    let run = catalog
        .find(&name)
        .ok_or_else(|| format!("no run matches {name}"))?;
    if run.outcome == Outcome::Running {
        return Err(format!(
            "{} is still running; analyze it when it ends",
            run.id()
        ));
    }
    let judge = match (&recorded, no_jev) {
        (_, true) => Judge::Off("--no-jev uses cached answers only".to_owned()),
        (Some(path), false) => Judge::Recorded(crate::runs_learning::Recorded::load(path)?),
        (None, false) => Judge::from_environment(),
    };
    let mut recording = record
        .as_ref()
        .map(|_| crate::runs_learning::Recorded::empty());
    let manifest = read_json(&fable_path);
    let analysis = analyze(
        run,
        &judge,
        cache_dir,
        manifest.as_ref(),
        recording.as_mut(),
    )?;
    if let (Some(path), Some(recording)) = (&record, &recording) {
        recording.save(path)?;
    }
    if keep {
        let (markdown, json_path) = write(run, &analysis)?;
        if !json_out {
            writeln!(out, "Wrote {}", markdown.display()).map_err(|e| e.to_string())?;
            writeln!(out, "Wrote {}", json_path.display()).map_err(|e| e.to_string())?;
            return Ok(0);
        }
    }
    let text = if json_out {
        serde_json::to_string_pretty(&analysis).map_err(|e| e.to_string())?
    } else {
        crate::runs_analysis_markdown::render(&analysis)
    };
    writeln!(out, "{}", text.trim_end()).map_err(|e| e.to_string())?;
    Ok(0)
}

#[cfg(test)]
#[path = "runs_analysis_tests.rs"]
mod tests;
