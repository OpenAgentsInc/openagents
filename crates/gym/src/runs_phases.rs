//! Each step of a trajectory, placed in a phase of the work.
//!
//! A trajectory is a list of tool calls. This module turns each call into a
//! [`Step`] and places it in one of eight phases: orient, read, plan, edit,
//! build, test, verify, and finish.
//!
//! - [`extract`] reads the actions in one raw record: a Harbor ATIF step
//!   (Claude Code, Codex, or the public Fable trajectories), a Coder One or
//!   Microluna episode-log step, a Claude Code `stream-json` line, or a
//!   Codex `exec --json` line. [`crate::runs_replay`] keeps what it finds
//!   beside each replay event, so replay and fingerprints read one parse.
//! - [`steps`] numbers the actions and places each with rules over tool
//!   names and command text. A command that runs a program the rules can't
//!   name, such as `python solve.py` or an inline script, stays unplaced.
//! - For an unplaced step, one Jev request asks a Choice over the phases
//!   and three Nouls: does the step check an assumption, does it use
//!   evidence from an earlier step, and is it a retry of a failed step.
//!   [`StepStore`] keeps each answer under the digest of the step's state
//!   and the question set, so a step is asked once.
//!
//! Rules come first because they are free, exact, and easy to audit. Jev
//! answers only the steps the rules leave, and its answers are labeled as
//! Jev's wherever they show.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use jev::{Choice, Entry, Noul, NoulCriteria, Questions, SystemOneRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::runs_learning::{Judge, USD_PER_MILLION_INPUT};
use crate::runs_replay::Replay;
use crate::runs_transcript::{content_text, unwrap_shell};

/// The rules' version. Change it with any change to [`steps`]' placement,
/// so a fingerprint says which rules made it.
pub const RULES_VERSION: &str = "runs-phases-rules-v2";

/// The Jev question set's version. Change it with any change to a
/// question's wording or to what [`state`] puts in a step's state.
pub const QUESTION_SET: &str = "runs-phases-v1";

/// How many Jev requests run at once.
pub const CONCURRENCY: usize = 12;

/// A Noul at or above this probability counts as yes.
pub const YES: f64 = 0.5;

/// A phase of the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Orient,
    Read,
    Plan,
    Edit,
    Build,
    Test,
    Verify,
    Finish,
}

impl Phase {
    /// Every phase, in the order a task usually moves through them.
    pub const ALL: [Phase; 8] = [
        Phase::Orient,
        Phase::Read,
        Phase::Plan,
        Phase::Edit,
        Phase::Build,
        Phase::Test,
        Phase::Verify,
        Phase::Finish,
    ];

    /// The phase's word.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Phase::Orient => "orient",
            Phase::Read => "read",
            Phase::Plan => "plan",
            Phase::Edit => "edit",
            Phase::Build => "build",
            Phase::Test => "test",
            Phase::Verify => "verify",
            Phase::Finish => "finish",
        }
    }

    /// The phase's letter in a compressed sequence.
    #[must_use]
    pub fn letter(self) -> char {
        match self {
            Phase::Orient => 'O',
            Phase::Read => 'R',
            Phase::Plan => 'P',
            Phase::Edit => 'E',
            Phase::Build => 'B',
            Phase::Test => 'T',
            Phase::Verify => 'V',
            Phase::Finish => 'F',
        }
    }

    /// The phase a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Phase::ALL.into_iter().find(|phase| phase.name() == word)
    }

    /// The option's meaning, as the Choice question sends it.
    fn meaning(self) -> &'static str {
        match self {
            Phase::Orient => {
                "Orient: looks around the environment, such as listing files, checking tool versions, or finding where things are."
            }
            Phase::Read => {
                "Read: reads source code, documentation, data, or logs to understand the task or the existing behavior, without running the code under study."
            }
            Phase::Plan => {
                "Plan: writes down a plan, a to-do list, or notes about what to do next, without changing the task's files."
            }
            Phase::Edit => {
                "Edit: changes the files the task is about, to build or fix the solution."
            }
            Phase::Build => {
                "Build: installs dependencies, compiles, or sets up the environment so code can run."
            }
            Phase::Test => {
                "Test: runs tests, a reproduction script, or the task's own example to see whether the behavior is right or to reproduce a bug."
            }
            Phase::Verify => {
                "Verify: after changes, checks the result directly, such as reading an output file, diffing changes, or confirming that a requirement from the task holds."
            }
            Phase::Finish => {
                "Finish: wraps up, such as removing scratch files, stopping processes, or writing the final report."
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Actions in a record
// ---------------------------------------------------------------------------

/// What kind of tool call an action is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ActionKind {
    /// A shell command.
    Command,
    /// A file read.
    Read(String),
    /// A text search.
    Search,
    /// A directory listing or file-name search.
    List,
    /// A change to files.
    Edit(Vec<String>),
    /// A plan or to-do list.
    Plan,
    /// The agent's finish call.
    Finish,
    /// A check Coder One's controller ran.
    Check,
    /// Anything else.
    #[default]
    Other,
}

/// One tool call, as a raw record shows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Action {
    /// The call's ID, when a later record carries its result.
    pub id: Option<String>,
    /// The tool's name, such as `Bash` or `exec_command`.
    pub tool: String,
    /// The command, the path, or the pattern.
    pub input: String,
    /// What came back, clipped to [`OUTPUT_KEEP`] characters.
    pub output: String,
    /// Whether it failed, when the record says.
    pub failed: Option<bool>,
    pub kind: ActionKind,
}

/// The most output an action keeps: the start and the end.
pub const OUTPUT_KEEP: usize = 1_200;

/// What one record holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extracted {
    pub actions: Vec<Action>,
    /// Results for calls made in earlier records: ID, output, failed.
    pub results: Vec<(String, String, Option<bool>)>,
    /// A user message that isn't wrapped in markup: the task, usually.
    pub user: Option<String>,
}

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The start and the end of `text`, within about `limit` characters.
#[must_use]
pub fn clip_ends(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let half = limit / 2;
    let head: String = text.chars().take(half).collect();
    let tail: String = text.chars().skip(count - half).collect();
    format!("{head} … {tail}")
}

/// Whether an output shows a failure, when no exit status says.
fn looks_failed(output: &str) -> Option<bool> {
    let lower = output.to_lowercase();
    for marker in [
        "exited with code ",
        "exit code: ",
        "exit code ",
        "exit status ",
    ] {
        if let Some(at) = lower.find(marker) {
            let code: String = lower[at + marker.len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(code) = code.parse::<i64>() {
                return Some(code != 0);
            }
        }
    }
    if output.contains("Traceback (most recent call last)")
        || lower.contains("command not found")
        || lower.contains("no such file or directory")
        || lower.contains("syntaxerror")
    {
        return Some(true);
    }
    None
}

/// The paths an `apply_patch` patch changes.
fn patch_paths(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            ["*** Update File:", "*** Add File:", "*** Delete File:"]
                .iter()
                .find_map(|prefix| line.strip_prefix(prefix))
                .map(|path| path.trim().to_owned())
        })
        .collect()
}

/// Reads a JavaScript string literal that starts right after its quote.
fn js_string(rest: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            },
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    None
}

/// The actions in a Codex `exec` script: each `exec_command`, each patch,
/// and each image view.
fn codex_script(id: Option<String>, script: &str, output: &str) -> Vec<Action> {
    let mut actions = Vec::new();
    let mut rest = script;
    while let Some(at) = rest.find("cmd:\"") {
        rest = &rest[at + 5..];
        if let Some(command) = js_string(rest) {
            actions.push(Action {
                id: id.clone(),
                tool: "exec_command".to_owned(),
                input: command,
                kind: ActionKind::Command,
                ..Action::default()
            });
        }
    }
    if script.contains("apply_patch") {
        let paths = patch_paths(&script.replace("\\n", "\n"));
        actions.push(Action {
            id: id.clone(),
            tool: "apply_patch".to_owned(),
            input: paths.join(", "),
            kind: ActionKind::Edit(paths),
            ..Action::default()
        });
    }
    if let Some(at) = script.find("view_image({path:\"") {
        let path = js_string(&script[at + 18..]).unwrap_or_default();
        actions.push(Action {
            id: id.clone(),
            tool: "view_image".to_owned(),
            input: path.clone(),
            kind: ActionKind::Read(path),
            ..Action::default()
        });
    }
    if actions.is_empty() {
        actions.push(Action {
            id,
            tool: "exec".to_owned(),
            input: script.to_owned(),
            kind: ActionKind::Other,
            ..Action::default()
        });
    }
    let failed = looks_failed(output);
    let count = actions.len();
    for action in &mut actions {
        // One script shares one output; each action keeps its share.
        action.output = clip_ends(output, OUTPUT_KEEP / count.max(1));
        action.failed = failed;
    }
    actions
}

/// The action one named tool call is.
fn from_tool(id: Option<String>, name: &str, input: &Value) -> Action {
    let path = || {
        [
            "file_path",
            "path",
            "notebook_path",
            "filePath",
            "target_file",
        ]
        .iter()
        .map(|key| string(input, key))
        .find(|path| !path.is_empty())
        .unwrap_or_default()
    };
    let (text, kind) = match name {
        "Bash" | "bash" | "shell" | "execute_command" | "run_command" | "exec_command"
        | "run_shell_command" | "terminal" | "run_terminal_cmd" => {
            let command = ["command", "cmd"]
                .iter()
                .map(|key| string(input, key))
                .find(|text| !text.is_empty())
                .unwrap_or_else(|| input.as_str().unwrap_or_default().to_owned());
            (unwrap_shell(&command), ActionKind::Command)
        }
        "Read" | "read_file" | "View" | "view" | "NotebookRead" | "view_image" => {
            let path = path();
            (path.clone(), ActionKind::Read(path))
        }
        "Grep" | "grep" | "search" | "Search" | "codebase_search" | "grep_search" => {
            (string(input, "pattern"), ActionKind::Search)
        }
        "WebFetch" | "WebSearch" | "web_search" | "fetch" => (
            [string(input, "url"), string(input, "query")].join(" "),
            ActionKind::Search,
        ),
        "Glob" | "LS" | "list_dir" | "list_files" | "ls" | "glob" => (
            [string(input, "pattern"), path()].join(" "),
            ActionKind::List,
        ),
        "str_replace_editor" | "str_replace_based_edit_tool"
            if string(input, "command") == "view" =>
        {
            let path = path();
            (path.clone(), ActionKind::Read(path))
        }
        "Edit"
        | "MultiEdit"
        | "Write"
        | "write_file"
        | "create_file"
        | "edit_file"
        | "str_replace_editor"
        | "str_replace_based_edit_tool"
        | "NotebookEdit" => {
            let path = path();
            (path.clone(), ActionKind::Edit(vec![path]))
        }
        "apply_patch" => {
            let patch = ["patch", "input"]
                .iter()
                .map(|key| string(input, key))
                .find(|text| !text.is_empty())
                .unwrap_or_else(|| input.as_str().unwrap_or_default().to_owned());
            let paths = patch_paths(&patch);
            (paths.join(", "), ActionKind::Edit(paths))
        }
        "TodoWrite" | "todo_write" | "update_plan" | "Task" | "Agent" | "ExitPlanMode"
        | "think" => (clip_ends(&input.to_string(), 300), ActionKind::Plan),
        "finished" | "finish" | "submit" | "attempt_completion" => {
            (clip_ends(&input.to_string(), 300), ActionKind::Finish)
        }
        _ => (clip_ends(&input.to_string(), 300), ActionKind::Other),
    };
    Action {
        id,
        tool: name.to_owned(),
        input: text,
        kind,
        ..Action::default()
    }
}

/// The actions and results one raw record holds.
#[must_use]
pub fn extract(record: &Value) -> Extracted {
    let mut out = Extracted::default();
    let source = record
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if source.eq_ignore_ascii_case("user") {
        let text = content_text(record.get("message").unwrap_or(&Value::Null));
        if !text.trim().is_empty() && !text.trim_start().starts_with('<') {
            out.user = Some(text);
        }
    }
    // A Harbor ATIF step: calls with results in the step's observation.
    if let Some(calls) = record.get("tool_calls").and_then(Value::as_array) {
        let results: HashMap<String, &Value> = record
            .pointer("/observation/results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|result| (string(result, "source_call_id"), result))
            .collect();
        for call in calls {
            let name = string(call, "function_name");
            if name.starts_with("jev_") || name == "delegate" {
                continue;
            }
            let id = string(call, "tool_call_id");
            let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
            let result = results.get(&id);
            let output = result
                .map(|result| content_text(result.get("content").unwrap_or(&Value::Null)))
                .unwrap_or_default();
            if name == "exec" {
                let script = string(&arguments, "input");
                out.actions.extend(codex_script(Some(id), &script, &output));
                continue;
            }
            let mut action = from_tool(Some(id), &name, &arguments);
            let flagged = result.is_some_and(|result| {
                result.pointer("/extra/status").and_then(Value::as_str) == Some("failed")
                    || result
                        .pointer("/extra/tool_result_is_error")
                        .and_then(Value::as_bool)
                        == Some(true)
            }) || call
                .pointer("/extra/exit")
                .and_then(Value::as_i64)
                .is_some_and(|code| code != 0);
            action.failed = if flagged {
                Some(true)
            } else if result.is_some() {
                looks_failed(&output).or(Some(false))
            } else {
                None
            };
            action.output = clip_ends(&output, OUTPUT_KEEP);
            out.actions.push(action);
        }
        return out;
    }
    // A Coder One or Microluna episode-log step: calls carry their output.
    // An ATIF log step holds one `call`; an exported document, `calls`.
    let single = record.get("call").map(std::slice::from_ref);
    if let Some(calls) = single.or_else(|| {
        record
            .get("calls")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
    }) {
        for call in calls {
            let name = string(call, "name");
            if name.starts_with("jev_")
                || name == "delegate"
                || call
                    .pointer("/extra/schema")
                    .and_then(Value::as_str)
                    .is_some_and(|schema| schema.contains("decision"))
            {
                continue;
            }
            let arguments = call.get("arguments").cloned().unwrap_or(Value::Null);
            let mut action = from_tool(Some(string(call, "id")), &name, &arguments);
            let output = string(call, "output");
            action.failed = match call.get("outcome").and_then(Value::as_str) {
                Some("failed") => Some(true),
                Some(_) => looks_failed(&output).or(Some(false)),
                None => looks_failed(&output),
            };
            action.output = clip_ends(&output, OUTPUT_KEEP);
            out.actions.push(action);
        }
        return out;
    }
    // Coder One's controller running its checks.
    if let Some(invocation) = record.pointer("/extensions/invocation")
        && invocation["event"] == "start"
        && invocation["component"] == "verify.checks.run"
    {
        out.actions.push(Action {
            id: None,
            tool: "coder-one check".to_owned(),
            input: string(invocation, "name"),
            kind: ActionKind::Check,
            ..Action::default()
        });
        return out;
    }
    match record.get("type").and_then(Value::as_str) {
        // Claude Code's stream: calls in assistant lines, results in user
        // lines.
        Some("assistant") => {
            for item in record
                .pointer("/message/content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if item.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let name = string(item, "name");
                    let input = item.get("input").cloned().unwrap_or(Value::Null);
                    out.actions
                        .push(from_tool(Some(string(item, "id")), &name, &input));
                }
            }
        }
        Some("user") => {
            for item in record
                .pointer("/message/content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if item.get("type").and_then(Value::as_str) == Some("tool_result") {
                    let output = content_text(item.get("content").unwrap_or(&Value::Null));
                    let failed = if item.get("is_error").and_then(Value::as_bool) == Some(true) {
                        Some(true)
                    } else {
                        looks_failed(&output).or(Some(false))
                    };
                    out.results.push((
                        string(item, "tool_use_id"),
                        clip_ends(&output, OUTPUT_KEEP),
                        failed,
                    ));
                }
            }
        }
        // Codex's stream: only completed items, so a started command
        // doesn't count twice.
        Some("item.completed") => {
            let item = &record["item"];
            let id = Some(string(item, "id"));
            match item.get("type").and_then(Value::as_str) {
                Some("command_execution") => {
                    let output = string(item, "aggregated_output");
                    let exit = item.get("exit_code").and_then(Value::as_i64);
                    out.actions.push(Action {
                        id,
                        tool: "command_execution".to_owned(),
                        input: unquote(&unwrap_shell(&string(item, "command"))),
                        failed: exit
                            .map(|code| code != 0)
                            .or_else(|| (string(item, "status") == "failed").then_some(true)),
                        output: clip_ends(&output, OUTPUT_KEEP),
                        kind: ActionKind::Command,
                    });
                }
                Some("file_change") => {
                    let paths: Vec<String> = item
                        .get("changes")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|change| string(change, "path"))
                        .collect();
                    out.actions.push(Action {
                        id,
                        tool: "file_change".to_owned(),
                        input: paths.join(", "),
                        failed: Some(string(item, "status") == "failed"),
                        kind: ActionKind::Edit(paths),
                        ..Action::default()
                    });
                }
                Some("web_search") => out.actions.push(Action {
                    id,
                    tool: "web_search".to_owned(),
                    input: string(item, "query"),
                    kind: ActionKind::Search,
                    ..Action::default()
                }),
                Some("todo_list") => out.actions.push(Action {
                    id,
                    tool: "todo_list".to_owned(),
                    input: clip_ends(&item["items"].to_string(), 300),
                    kind: ActionKind::Plan,
                    ..Action::default()
                }),
                Some("mcp_tool_call") => out.actions.push(Action {
                    id,
                    tool: string(item, "tool"),
                    input: clip_ends(&item["arguments"].to_string(), 300),
                    kind: ActionKind::Other,
                    ..Action::default()
                }),
                _ => {}
            }
        }
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// What one piece of a shell command does.
#[derive(Clone, Debug, PartialEq)]
enum Piece {
    /// Writes a file: the path, and whether it is scratch.
    Write(String),
    Test,
    Build,
    /// Runs a program the rules can't name.
    Run,
    /// Runs a script by path.
    Script(String),
    Diff,
    Read(String),
    Orient,
    Cleanup,
    Noise,
    Unknown,
}

/// Whether `path` is scratch: outside the task's files.
#[must_use]
pub fn scratch(path: &str) -> bool {
    let path = path.trim_matches(|c| c == '"' || c == '\'');
    path.starts_with("/tmp/")
        || path.starts_with("/var/tmp/")
        || path.starts_with("/dev/")
        || path.contains("/.cache/")
        || path.starts_with("~/")
}

/// Whether a file name reads as a test, a reproduction, or a check.
#[must_use]
pub fn testish(path: &str) -> bool {
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    name.starts_with("test")
        || name.contains("_test")
        || name.contains("-test")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.starts_with("repro")
        || name.starts_with("check")
        || name.starts_with("verify")
        || name.starts_with("validate")
        || lower.contains("/tests/")
        || lower.contains("/test/")
}

/// Whether a path names notes: Markdown or plain text.
fn notes(path: &str) -> bool {
    let lower = path.to_lowercase();
    (lower.ends_with(".md") || lower.ends_with(".txt")) && !testish(path)
}

/// A command whose shell wrapper left its opening quote, as a cut-off
/// Codex command does.
fn unquote(command: &str) -> String {
    match command.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let inner = &command[1..];
            inner.strip_suffix(quote).unwrap_or(inner).to_owned()
        }
        _ => command.to_owned(),
    }
}

/// Whether a path names a script or a program's source.
fn script_file(path: &str) -> bool {
    [
        ".py", ".sh", ".js", ".mjs", ".ts", ".rb", ".pl", ".jl", ".r", ".c", ".cc", ".cpp", ".rs",
        ".go", ".java", ".lua", ".php", ".v",
    ]
    .iter()
    .any(|ext| path.to_lowercase().ends_with(ext))
}

/// Splits a command into pieces, skipping heredoc bodies. Returns the
/// pieces and the heredoc bodies, each with the command line that opened
/// it.
fn split(command: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut pieces = Vec::new();
    let mut bodies = Vec::new();
    let mut lines = command.lines();
    while let Some(line) = lines.next() {
        let mut opener = None;
        if let Some(at) = line.find("<<") {
            let rest = line[at + 2..].trim_start_matches(['-', '~']).trim_start();
            let delimiter: String = rest
                .trim_start_matches(['\'', '"'])
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !delimiter.is_empty() {
                let mut body = String::new();
                for next in lines.by_ref() {
                    if next.trim() == delimiter {
                        break;
                    }
                    body.push_str(next);
                    body.push('\n');
                }
                bodies.push((line.to_owned(), body));
                opener = Some(line.to_owned());
            }
        }
        let line = opener.as_deref().unwrap_or(line);
        let mut current = String::new();
        let mut quote: Option<char> = None;
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            match quote {
                Some(q) if c == q => quote = None,
                Some(_) => {}
                None if c == '\'' || c == '"' => quote = Some(c),
                None if c == ';' || c == '|' || (c == '&' && chars.get(i + 1) == Some(&'&')) => {
                    if !current.trim().is_empty() {
                        pieces.push(current.trim().to_owned());
                    }
                    current.clear();
                    if chars.get(i + 1) == Some(&c) {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                None => {}
            }
            current.push(c);
            i += 1;
        }
        if !current.trim().is_empty() {
            pieces.push(current.trim().to_owned());
        }
    }
    (pieces, bodies)
}

/// The file a piece redirects its output to, if any.
fn redirect(piece: &str) -> Option<String> {
    let mut words = piece.split_whitespace().peekable();
    while let Some(word) = words.next() {
        let target = if word == ">" || word == ">>" || word == "1>" {
            words.peek().map(|next| (*next).to_owned())
        } else {
            word.strip_prefix(">>")
                .or_else(|| word.strip_prefix('>'))
                .filter(|rest| !rest.is_empty() && !rest.starts_with('&'))
                .map(str::to_owned)
        };
        if let Some(target) = target {
            let target = target.trim_matches(|c| c == '"' || c == '\'').to_owned();
            if target != "/dev/null" && !target.starts_with('&') {
                return Some(target);
            }
        }
    }
    None
}

/// The piece's words, without leading environment settings and wrappers.
fn words(piece: &str) -> Vec<String> {
    let mut words: Vec<String> = piece
        .split_whitespace()
        .map(|word| word.trim_matches(|c| c == '(' || c == ')').to_owned())
        .collect();
    while let Some(first) = words.first() {
        let wrapper = matches!(
            first.as_str(),
            "sudo" | "time" | "nice" | "env" | "exec" | "xvfb-run" | "stdbuf" | "nohup" | "command"
        ) || (first.contains('=') && !first.starts_with('-'));
        if first == "timeout" {
            words.remove(0);
            while words.first().is_some_and(|w| {
                w.starts_with('-') || w.chars().next().is_some_and(|c| c.is_ascii_digit())
            }) {
                words.remove(0);
            }
            continue;
        }
        if wrapper {
            words.remove(0);
            continue;
        }
        break;
    }
    words
}

fn first_path(words: &[String]) -> String {
    words
        .iter()
        .skip(1)
        .find(|word| !word.starts_with('-'))
        .cloned()
        .unwrap_or_default()
}

/// What one piece of a command does.
fn piece(text: &str) -> Piece {
    let words = words(text);
    let Some(program) = words.first() else {
        return Piece::Noise;
    };
    // Shell syntax: a loop or branch keyword leads the command it runs.
    match program.as_str() {
        "do" | "then" | "else" | "elif" | "if" | "while" | "until" | "{" | "!" => {
            let rest = text
                .trim_start()
                .trim_start_matches(['(', ' '])
                .strip_prefix(program.as_str())
                .unwrap_or_default()
                .trim();
            return if rest.is_empty() {
                Piece::Noise
            } else {
                piece(rest)
            };
        }
        "for" | "done" | "fi" | "esac" | "case" | "}" | "select" | "function" | "in" | ";;" => {
            return Piece::Noise;
        }
        _ => {}
    }
    let program = program.rsplit('/').next().unwrap_or(program).to_owned();
    let has = |word: &str| words.iter().skip(1).any(|w| w == word);
    let second = words.get(1).map(String::as_str).unwrap_or_default();
    let writer = matches!(
        program.as_str(),
        "cat"
            | "echo"
            | "printf"
            | "tee"
            | "sed"
            | "awk"
            | "perl"
            | "sort"
            | "cut"
            | "jq"
            | "base64"
            | "head"
            | "tail"
            | "tr"
            | "envsubst"
    );
    if writer && let Some(target) = redirect(text) {
        return Piece::Write(target);
    }
    if program == "tee" {
        return Piece::Write(first_path(&words));
    }
    if (program == "sed" || program == "perl")
        && words.iter().any(|w| w.starts_with("-i") || w == "-pi")
    {
        // The file is the last word that isn't the script or a flag.
        let file = words
            .iter()
            .skip(1)
            .rev()
            .find(|w| !w.starts_with('-') && !w.contains(['\\', ';', '\'', '"']))
            .cloned()
            .unwrap_or_default();
        return Piece::Write(file);
    }
    if words
        .iter()
        .any(|w| w == "--version" || w == "-V" || w == "--help")
        && words.len() <= 3
    {
        return Piece::Orient;
    }
    match program.as_str() {
        "cd" | "echo" | "printf" | "export" | "set" | "true" | "false" | "sleep" | "source"
        | "." | "clear" | ":" | "unset" | "wait" | "trap" | "mkdir" | "chmod" | "chown"
        | "exit" | "ulimit" | "alias" | "read" | "shopt" | "pushd" | "popd" | "umask" => {
            Piece::Noise
        }
        "ls" | "pwd" | "find" | "tree" | "which" | "whoami" | "uname" | "nproc" | "printenv"
        | "df" | "du" | "free" | "id" | "ps" | "hostname" | "lscpu" | "type" | "file" | "stat"
        | "date" | "top" | "locale" | "realpath" | "dirname" | "basename" | "whereis" | "lsof"
        | "ss" | "netstat" | "nvidia-smi" | "getconf" | "lsb_release" | "readlink" => Piece::Orient,
        "cat" | "head" | "tail" | "less" | "more" | "nl" | "wc" | "grep" | "egrep" | "rg"
        | "ag" | "awk" | "xxd" | "od" | "hexdump" | "strings" | "jq" | "yq" | "sed" | "sort"
        | "uniq" | "cut" | "column" | "objdump" | "readelf" | "nm" | "ldd" | "tr" | "fold"
        | "fmt" | "md5sum" | "sha256sum" | "zcat" | "bat" | "view" => {
            let path = words
                .iter()
                .skip(1)
                .rev()
                .find(|w| !w.starts_with('-') && w.contains('/'))
                .cloned()
                .unwrap_or_default();
            Piece::Read(path)
        }
        "diff" | "cmp" | "comm" => Piece::Diff,
        "git" => match second {
            "diff" | "show" => Piece::Diff,
            "apply" | "checkout" | "restore" | "revert" | "reset" | "am" | "cherry-pick" => {
                Piece::Write("git".to_owned())
            }
            "add" | "commit" | "stash" => Piece::Noise,
            _ => Piece::Orient,
        },
        "cp" | "mv" | "install" | "touch" | "ln" | "rsync" | "patch" | "truncate" | "dd"
        | "unzip" | "tar" => {
            if program == "tar" && words.iter().any(|w| w.contains('t') && w.starts_with('-')) {
                return Piece::Orient;
            }
            if program == "unzip" && has("-l") {
                return Piece::Orient;
            }
            Piece::Write(words.last().cloned().unwrap_or_default())
        }
        "rm" | "rmdir" | "pkill" | "kill" | "killall" | "shred" => Piece::Cleanup,
        "pytest" | "py.test" | "tox" | "nox" | "jest" | "vitest" | "mocha" | "ctest" | "bats"
        | "rspec" | "phpunit" | "prove" | "nextest" => Piece::Test,
        "make" | "gmake" | "ninja" => {
            if words
                .iter()
                .skip(1)
                .any(|w| w.contains("test") || w.contains("check") || w.contains("repro"))
            {
                Piece::Test
            } else {
                Piece::Build
            }
        }
        "cargo" => match second {
            "test" | "nextest" => Piece::Test,
            "run" => Piece::Run,
            _ => Piece::Build,
        },
        "go" => match second {
            "test" => Piece::Test,
            "run" => Piece::Run,
            _ => Piece::Build,
        },
        "npm" | "yarn" | "pnpm" | "bun" => {
            let target = if second == "run" {
                words.get(2).map(String::as_str).unwrap_or_default()
            } else {
                second
            };
            if target.contains("test") || target == "t" {
                Piece::Test
            } else if matches!(
                target,
                "install" | "i" | "ci" | "add" | "build" | "lint" | "typecheck" | "tsc"
            ) {
                Piece::Build
            } else if matches!(target, "ls" | "list" | "view" | "info" | "outdated") {
                Piece::Orient
            } else {
                Piece::Run
            }
        }
        "npx" => {
            let tool = second;
            if ["jest", "vitest", "mocha", "playwright"].contains(&tool) {
                Piece::Test
            } else if ["tsc", "eslint", "prettier", "next"].contains(&tool) && has("build")
                || tool == "tsc"
            {
                Piece::Build
            } else {
                Piece::Run
            }
        }
        "cmake" | "meson" | "gcc" | "g++" | "cc" | "c++" | "clang" | "clang++" | "rustc"
        | "javac" | "tsc" | "coqc" | "coq_makefile" | "ghc" | "nasm" | "as" | "ld" | "dune"
        | "zig" | "swiftc" | "apt" | "apt-get" | "apk" | "brew" | "gem" | "cabal" | "stack"
        | "opam" | "conda" | "mamba" | "micromamba" | "pip" | "pip3" | "uv" | "poetry" | "pipx"
        | "rustup" | "mvn" | "gradle" | "sbt" | "dotnet" | "lake" | "elan" | "composer"
        | "configure" | "autoreconf" | "bundle" => {
            if matches!(program.as_str(), "pip" | "pip3" | "conda" | "uv")
                && matches!(second, "list" | "show" | "freeze" | "search")
            {
                return Piece::Orient;
            }
            if matches!(
                program.as_str(),
                "mvn" | "gradle" | "dotnet" | "sbt" | "lake" | "dune"
            ) && words.iter().skip(1).any(|w| w.contains("test"))
            {
                return Piece::Test;
            }
            Piece::Build
        }
        "python" | "python3" | "python3.11" | "python3.12" | "python3.10" | "pypy3" | "uvx" => {
            if second == "-m" {
                let module = words.get(2).map(String::as_str).unwrap_or_default();
                return match module {
                    "pytest" | "unittest" | "doctest" | "tox" => Piece::Test,
                    "pip" | "venv" | "py_compile" | "compileall" | "build" | "ensurepip" => {
                        Piece::Build
                    }
                    "json.tool" => Piece::Read(String::new()),
                    _ => Piece::Run,
                };
            }
            if second == "-c" && (text.contains("__version__") || text.contains("sys.version")) {
                return Piece::Orient;
            }
            match words.iter().skip(1).find(|w| !w.starts_with('-')) {
                Some(script) if script.ends_with(".py") => Piece::Script(script.clone()),
                _ => Piece::Run,
            }
        }
        "node" | "ruby" | "php" | "Rscript" | "julia" | "lua" | "java" | "dotnet-script"
        | "tsx" | "ts-node" | "bash" | "sh" | "zsh" | "perl" | "swift" | "kotlin" | "scala"
        | "elixir" | "deno" => {
            if program == "node" && (second == "--check" || second == "-c") {
                return Piece::Build;
            }
            match words.iter().skip(1).find(|w| !w.starts_with('-')) {
                Some(script) if script.contains('.') || script.contains('/') => {
                    Piece::Script(script.clone())
                }
                _ => Piece::Run,
            }
        }
        _ if words[0].starts_with("./") || words[0].starts_with('/') => {
            Piece::Script(words[0].clone())
        }
        _ => Piece::Unknown,
    }
}

/// What a whole command does, by the rules: a phase, or none, and the
/// files it writes, and whether it runs something.
struct Placed {
    phase: Option<Phase>,
    writes: Vec<String>,
    executes: bool,
}

fn place_command(command: &str, edited: &HashSet<String>, edits_before: usize) -> Placed {
    let (texts, bodies) = split(command);
    let pieces: Vec<Piece> = texts.iter().map(|text| piece(text)).collect();
    let mut writes: Vec<String> = Vec::new();
    let mut scratch_writes: Vec<String> = Vec::new();
    for piece in &pieces {
        if let Piece::Write(path) = piece {
            if scratch(path) || path.is_empty() {
                scratch_writes.push(path.clone());
            } else {
                writes.push(path.clone());
            }
        }
    }
    // An inline script that writes a file is an edit too.
    for (opener, body) in &bodies {
        let inline =
            opener.contains("python") || opener.contains("node") || opener.contains("perl");
        if inline && let Some(path) = python_write(body) {
            if scratch(&path) {
                scratch_writes.push(path);
            } else {
                writes.push(path);
            }
        }
    }
    let has = |want: fn(&Piece) -> bool| pieces.iter().any(want);
    // A script the agent wrote outside the task's files, or one named like
    // a test, is the agent's own probe: running it is testing.
    let mut in_scratch = false;
    let mut probe_run = false;
    for (text, piece) in texts.iter().zip(&pieces) {
        let words = words(text);
        if words.first().map(String::as_str) == Some("cd") {
            in_scratch = words.get(1).is_some_and(|dir| scratch(dir));
        }
        if let Piece::Script(path) = piece
            && (testish(path)
                || scratch(path)
                || (in_scratch && !path.starts_with('/'))
                || scratch_writes
                    .iter()
                    .any(|written| same_file(written, path)))
        {
            probe_run = true;
        }
    }
    let probe_written = scratch_writes.iter().any(|path| script_file(path));
    let test = has(|p| matches!(p, Piece::Test)) || probe_run || probe_written;
    let run = has(|p| matches!(p, Piece::Run | Piece::Script(_)));
    let executes = test || run;
    let solution_writes: Vec<String> = writes.iter().filter(|p| !testish(p)).cloned().collect();
    let phase = if !solution_writes.is_empty() {
        Some(Phase::Edit)
    } else if !writes.is_empty() && !run {
        // Writing a test file of the task's own.
        Some(Phase::Test)
    } else if test {
        Some(Phase::Test)
    } else if run || has(|p| matches!(p, Piece::Unknown)) {
        None
    } else if has(|p| matches!(p, Piece::Build)) {
        Some(Phase::Build)
    } else if !scratch_writes.is_empty() {
        // Notes outside the task's files are a plan; anything else is left.
        scratch_writes
            .iter()
            .all(|path| notes(path))
            .then_some(Phase::Plan)
    } else if has(|p| matches!(p, Piece::Diff)) {
        Some(if edits_before > 0 {
            Phase::Verify
        } else {
            Phase::Read
        })
    } else if pieces.iter().any(|p| matches!(p, Piece::Read(_))) {
        let checks_edit = edits_before > 0
            && pieces.iter().any(|p| {
                matches!(p, Piece::Read(path) if !path.is_empty() && edited.iter().any(|e| same_file(e, path)))
            });
        Some(if checks_edit {
            Phase::Verify
        } else {
            Phase::Read
        })
    } else if has(|p| matches!(p, Piece::Orient)) {
        Some(Phase::Orient)
    } else if has(|p| matches!(p, Piece::Cleanup)) {
        Some(Phase::Finish)
    } else {
        None
    };
    let writes = if phase == Some(Phase::Edit) {
        solution_writes
    } else {
        writes
    };
    Placed {
        phase,
        writes,
        executes,
    }
}

/// The path an inline script opens for writing, if any.
fn python_write(body: &str) -> Option<String> {
    for line in body.lines() {
        let writes = (line.contains("open(")
            && (line.contains("'w'") || line.contains("\"w\"") || line.contains("'a'")))
            || line.contains("write_text(")
            || line.contains("writeFileSync(");
        if writes {
            let quoted = line
                .split(['\'', '"'])
                .skip(1)
                .step_by(2)
                .find(|text| text.contains('/') || text.contains('.'))?;
            return Some(quoted.to_owned());
        }
    }
    None
}

fn same_file(a: &str, b: &str) -> bool {
    let name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_owned();
    a == b || (!name(a).is_empty() && name(a) == name(b) && (a.ends_with(b) || b.ends_with(a)))
}

// ---------------------------------------------------------------------------
// Steps
// ---------------------------------------------------------------------------

/// Jev's judgments on one step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepAnswer {
    /// The digest of the state and the question set.
    pub key: String,
    pub phase: String,
    pub confidence: f64,
    pub probabilities: BTreeMap<String, f64>,
    pub checks_assumption: f64,
    pub uses_evidence: f64,
    pub retry: f64,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
}

/// One step of a trajectory, placed.
#[derive(Clone, Debug, Serialize)]
pub struct Step {
    /// 1-based, in the order the actions happened.
    pub n: usize,
    /// The replay event it came from, 0-based.
    pub event: usize,
    pub elapsed_ms: u64,
    pub tool: String,
    /// The command, path, or pattern, clipped.
    pub input: String,
    pub failed: Option<bool>,
    /// The rules' phase, when they place it.
    pub rule: Option<Phase>,
    /// The phase: the rules', or Jev's for a step they leave.
    pub phase: Option<Phase>,
    /// `rule`, `jev`, or `none`.
    pub by: &'static str,
    /// Files it changed in the task's workspace.
    pub writes: Vec<String>,
    /// Whether it runs a program: a test, a script, or an example.
    pub executes: bool,
    /// The same command as an earlier failed one, or as the one right
    /// before it.
    pub repeats_failed: bool,
    pub jev: Option<StepAnswer>,
    #[serde(skip)]
    pub output: String,
    #[serde(skip)]
    pub kind: ActionKind,
}

fn normalized(command: &str) -> String {
    command
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(300)
        .collect()
}

/// The steps in a replay, numbered and placed by the rules.
#[must_use]
pub fn steps(replay: &Replay) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut by_id: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, event) in replay.events.iter().enumerate() {
        let Some(extracted) = replay.extracted.get(index) else {
            continue;
        };
        for (id, output, failed) in &extracted.results {
            for &at in by_id.get(id).into_iter().flatten() {
                steps[at].output.clone_from(output);
                steps[at].failed = *failed;
            }
        }
        for action in &extracted.actions {
            if let Some(id) = &action.id {
                by_id.entry(id.clone()).or_default().push(steps.len());
            }
            steps.push(Step {
                n: steps.len() + 1,
                event: index,
                elapsed_ms: event.elapsed_ms,
                tool: action.tool.clone(),
                input: action.input.clone(),
                failed: action.failed,
                rule: None,
                phase: None,
                by: "none",
                writes: Vec::new(),
                executes: false,
                repeats_failed: false,
                jev: None,
                output: action.output.clone(),
                kind: action.kind.clone(),
            });
        }
    }
    place(&mut steps);
    steps
}

/// Places each step by the rules.
fn place(steps: &mut [Step]) {
    let mut edited: HashSet<String> = HashSet::new();
    let mut edits = 0usize;
    let mut failed_commands: HashSet<String> = HashSet::new();
    let mut previous: Option<String> = None;
    for step in steps.iter_mut() {
        let (phase, writes, executes) = match &step.kind {
            ActionKind::Command => {
                let placed = place_command(&step.input, &edited, edits);
                (placed.phase, placed.writes, placed.executes)
            }
            ActionKind::Read(path) => (
                Some(if edits > 0 && edited.iter().any(|e| same_file(e, path)) {
                    Phase::Verify
                } else {
                    Phase::Read
                }),
                Vec::new(),
                false,
            ),
            ActionKind::Search => (Some(Phase::Read), Vec::new(), false),
            ActionKind::List => (Some(Phase::Orient), Vec::new(), false),
            ActionKind::Edit(paths) => {
                let (scratch_paths, work): (Vec<&String>, Vec<&String>) =
                    paths.iter().partition(|path| scratch(path));
                let (tests, solution): (Vec<&String>, Vec<&String>) =
                    work.into_iter().partition(|path| testish(path));
                if !solution.is_empty() {
                    (
                        Some(Phase::Edit),
                        solution.into_iter().cloned().collect(),
                        false,
                    )
                } else if !tests.is_empty()
                    || scratch_paths.iter().any(|p| testish(p) || script_file(p))
                {
                    (Some(Phase::Test), Vec::new(), false)
                } else if !scratch_paths.is_empty() && scratch_paths.iter().all(|p| notes(p)) {
                    (Some(Phase::Plan), Vec::new(), false)
                } else {
                    (None, Vec::new(), false)
                }
            }
            ActionKind::Plan => (Some(Phase::Plan), Vec::new(), false),
            ActionKind::Finish => (Some(Phase::Finish), Vec::new(), false),
            ActionKind::Check => (Some(Phase::Verify), Vec::new(), true),
            ActionKind::Other => (None, Vec::new(), false),
        };
        step.rule = phase;
        step.phase = phase;
        step.by = if phase.is_some() { "rule" } else { "none" };
        step.executes = executes;
        if matches!(step.kind, ActionKind::Command) {
            let text = normalized(&step.input);
            step.repeats_failed =
                failed_commands.contains(&text) || previous.as_ref().is_some_and(|p| *p == text);
            if step.failed == Some(true) {
                failed_commands.insert(text.clone());
            }
            previous = Some(text);
        }
        if phase == Some(Phase::Edit) {
            edits += 1;
            edited.extend(writes.iter().cloned());
        }
        step.writes = writes;
        step.input = clip_ends(&step.input, 2_000);
    }
}

// ---------------------------------------------------------------------------
// Jev
// ---------------------------------------------------------------------------

const CHOICE_ASK: &str = "Which phase of a coding agent's work is the step in `step`? `task` is what the agent was asked to do. `before` lists the steps just before it, with the phase code's rules gave each, or `unplaced`. `step.edits_before` counts the agent's changes to the task's files before this step.";

const ASSUMPTION_ASK: &str = "Does the step in `step` check an assumption the agent is making, for example whether a file or tool exists, how a function behaves, what a data field holds, or whether an earlier change took effect, rather than doing new work?";
const ASSUMPTION_YES: &str = "The step mainly confirms or refutes something the agent believes about the code, the data, or the environment.";
const ASSUMPTION_NO: &str = "The step does new work, such as changing files, installing, or running a full test suite, or it reads broadly without testing a specific belief.";

const EVIDENCE_ASK: &str = "Does the step in `step` act on evidence an earlier step in `before` revealed, such as a path, an error message, a value, or a failing test shown in that step's `output`?";
const EVIDENCE_YES: &str =
    "The step's command, path, or target follows from something an earlier step's output showed.";
const EVIDENCE_NO: &str =
    "The step doesn't depend on what earlier outputs showed, or `before` is empty.";

const RETRY_ASK: &str = "Is the step in `step` a retry of an earlier step in `before` that failed: the same or nearly the same command or edit, run again after an error?";
const RETRY_YES: &str =
    "An earlier step failed, and this step runs the same thing again, possibly with a small fix.";
const RETRY_NO: &str =
    "No earlier step failed, or this step does something different from the failed one.";

/// The question set each unplaced step is asked.
#[must_use]
pub fn questions() -> Questions {
    let mut choice = Choice::new(CHOICE_ASK, indexmap::IndexMap::new());
    for phase in Phase::ALL {
        choice = choice.option(phase.name(), phase.meaning());
    }
    Questions::new()
        .with("phase", choice)
        .with(
            "checks_assumption",
            Noul::with_criteria(
                ASSUMPTION_ASK,
                NoulCriteria::new()
                    .when_true(ASSUMPTION_YES)
                    .when_false(ASSUMPTION_NO),
            ),
        )
        .with(
            "uses_evidence",
            Noul::with_criteria(
                EVIDENCE_ASK,
                NoulCriteria::new()
                    .when_true(EVIDENCE_YES)
                    .when_false(EVIDENCE_NO),
            ),
        )
        .with(
            "retry",
            Noul::with_criteria(
                RETRY_ASK,
                NoulCriteria::new()
                    .when_true(RETRY_YES)
                    .when_false(RETRY_NO),
            ),
        )
}

/// The question set's digest.
#[must_use]
pub fn questions_digest() -> String {
    atif::digest(&json!({ "set": QUESTION_SET, "questions": questions() }))
}

/// The state Jev reads for step `index`: the task, the step, and the three
/// steps before it with their rule phases. It depends only on the
/// trajectory and the rules, so the same step always has the same state.
#[must_use]
pub fn state(steps: &[Step], index: usize, task: &str) -> Value {
    let step = &steps[index];
    let edits_before = steps[..index]
        .iter()
        .filter(|s| s.rule == Some(Phase::Edit))
        .count();
    let before: Vec<Value> = steps[index.saturating_sub(3)..index]
        .iter()
        .map(|s| {
            json!({
                "number": s.n,
                "phase": s.rule.map_or("unplaced", Phase::name),
                "tool": s.tool,
                "input": clip_ends(&s.input, 240),
                "output": clip_ends(&s.output, 240),
                "failed": s.failed,
            })
        })
        .collect();
    json!({
        "task": clip_ends(&task.split_whitespace().collect::<Vec<_>>().join(" "), 700),
        "step": {
            "number": step.n,
            "of": steps.len(),
            "tool": step.tool,
            "input": clip_ends(&step.input, 900),
            "output": clip_ends(&step.output, 600),
            "failed": step.failed,
            "edits_before": edits_before,
        },
        "before": before,
    })
}

/// The answer key for a step's state.
#[must_use]
pub fn key(state: &Value) -> String {
    atif::digest(&json!({ "set": QUESTION_SET, "state": state, "questions": questions() }))
}

/// Where step answers are kept: `~/.openagents/gym/fingerprints`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/fingerprints"))
}

/// Jev's answers by step key, appended to `step-answers.jsonl`.
#[derive(Clone, Debug, Default)]
pub struct StepStore {
    pub dir: Option<PathBuf>,
    answers: HashMap<String, StepAnswer>,
}

const STEP_FILE: &str = "step-answers.jsonl";

impl StepStore {
    /// Reads the store under `dir`; a missing file is an empty store.
    #[must_use]
    pub fn open(dir: Option<PathBuf>) -> Self {
        let mut store = StepStore {
            dir,
            answers: HashMap::new(),
        };
        if let Some(dir) = &store.dir
            && let Ok(text) = std::fs::read_to_string(dir.join(STEP_FILE))
        {
            for line in text.lines() {
                if let Ok(answer) = serde_json::from_str::<StepAnswer>(line) {
                    store.answers.insert(answer.key.clone(), answer);
                }
            }
        }
        store
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&StepAnswer> {
        self.answers.get(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.answers.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }

    /// Keeps `answer`, appending it to disk when the store has a directory.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn insert(&mut self, answer: StepAnswer) -> Result<(), String> {
        if let Some(dir) = &self.dir {
            std::fs::create_dir_all(dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
            let path = dir.join(STEP_FILE);
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
            let line = serde_json::to_string(&answer).map_err(|e| e.to_string())?;
            writeln!(file, "{line}")
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        }
        self.answers.insert(answer.key.clone(), answer);
        Ok(())
    }
}

/// The unplaced steps' states and keys.
#[must_use]
pub fn pending(steps: &[Step], task: &str) -> Vec<(String, Value)> {
    steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.rule.is_none())
        .map(|(index, _)| {
            let state = state(steps, index, task);
            (key(&state), state)
        })
        .collect()
}

/// Gives each unplaced step Jev's answer, when the store has one.
pub fn apply(steps: &mut [Step], task: &str, store: &StepStore) {
    for index in 0..steps.len() {
        if steps[index].rule.is_some() {
            continue;
        }
        let key = key(&state(steps, index, task));
        if let Some(answer) = store.get(&key) {
            steps[index].phase = Phase::parse(&answer.phase);
            steps[index].by = "jev";
            steps[index].jev = Some(answer.clone());
        }
    }
}

/// What one labeling pass did.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Labeling {
    /// Unplaced steps considered.
    pub considered: usize,
    /// Steps whose answer was already stored.
    pub cached: usize,
    /// Requests sent.
    pub asked: usize,
    pub answered: usize,
    pub failed: usize,
    pub input_tokens: u64,
    /// What the answered requests cost at the learning rate.
    pub cost_usd: f64,
    pub errors: Vec<String>,
}

/// Asks Jev about every pending step without a stored answer, keeping the
/// answers in `store`. `limit` bounds the requests.
pub async fn label(
    pending: Vec<(String, Value)>,
    store: &mut StepStore,
    judge: &Judge,
    limit: Option<usize>,
) -> Labeling {
    let mut report = Labeling::default();
    let mut seen = HashSet::new();
    let mut todo: std::collections::VecDeque<(String, Value)> = std::collections::VecDeque::new();
    for (key, state) in pending {
        if !seen.insert(key.clone()) {
            continue;
        }
        report.considered += 1;
        if store.get(&key).is_some() {
            report.cached += 1;
        } else {
            todo.push_back((key, state));
        }
    }
    if let Some(limit) = limit {
        todo.truncate(limit);
    }
    let Judge::Live(client) = judge else {
        if !todo.is_empty() {
            report.failed = todo.len();
            report.errors.push(format!(
                "{} steps wait for Jev: {}",
                todo.len(),
                judge
                    .unavailable()
                    .unwrap_or("recorded answers don't cover steps")
            ));
        }
        return report;
    };
    let mut set = tokio::task::JoinSet::new();
    loop {
        while set.len() < CONCURRENCY
            && let Some((key, state)) = todo.pop_front()
        {
            let client = client.clone();
            report.asked += 1;
            set.spawn(async move {
                let started = Instant::now();
                let result = client
                    .system_one(SystemOneRequest::new(Entry::from(state), questions()))
                    .await;
                (key, result, started.elapsed())
            });
        }
        let Some(joined) = set.join_next().await else {
            break;
        };
        let Ok((key, result, _elapsed)) = joined else {
            report.failed += 1;
            continue;
        };
        let answer = result.map_err(|e| e.to_string()).and_then(|response| {
            let choice = response.choice("phase").map_err(|e| e.to_string())?;
            let noul = |id: &str| response.noul(id).map(|a| a.noul).map_err(|e| e.to_string());
            Ok(StepAnswer {
                key: key.clone(),
                phase: choice.choice.clone(),
                confidence: choice.confidence,
                probabilities: choice
                    .probabilities
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect(),
                checks_assumption: noul("checks_assumption")?,
                uses_evidence: noul("uses_evidence")?,
                retry: noul("retry")?,
                input_tokens: response.usage.input_tokens,
                model: Some(response.model.clone()),
            })
        });
        match answer {
            Ok(answer) => {
                report.input_tokens += answer.input_tokens.unwrap_or(0);
                match store.insert(answer) {
                    Ok(()) => report.answered += 1,
                    Err(error) => {
                        report.failed += 1;
                        report.errors.push(error);
                    }
                }
            }
            Err(error) => {
                report.failed += 1;
                if report.errors.len() < 5 {
                    report.errors.push(
                        error
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .chars()
                            .take(200)
                            .collect(),
                    );
                }
            }
        }
    }
    report.cost_usd = report.input_tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0;
    report
}

/// Each event's phases, in words, for replay: `test`, or `read, edit`,
/// with `(Jev)` when Jev placed any of them.
#[must_use]
pub fn event_labels(replay: &Replay, task: &str, store: &StepStore) -> Vec<String> {
    let mut steps = steps(replay);
    apply(&mut steps, task, store);
    let mut labels = vec![String::new(); replay.events.len()];
    let mut by_event: BTreeMap<usize, Vec<&Step>> = BTreeMap::new();
    for step in &steps {
        by_event.entry(step.event).or_default().push(step);
    }
    for (event, list) in by_event {
        let mut words: Vec<&str> = Vec::new();
        let mut jev = false;
        for step in list {
            let word = step.phase.map_or("unplaced", Phase::name);
            jev |= step.by == "jev";
            if !words.contains(&word) {
                words.push(word);
            }
        }
        if let Some(label) = labels.get_mut(event) {
            *label = format!("{}{}", words.join(", "), if jev { " (Jev)" } else { "" });
        }
    }
    labels
}

/// The task a replay's steps are judged against: its first plain user
/// message, or `fallback`.
#[must_use]
pub fn task_of(replay: &Replay, fallback: Option<String>) -> String {
    replay
        .extracted
        .iter()
        .find_map(|extracted| extracted.user.clone())
        .or(fallback)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(command: &str) -> Option<Phase> {
        place_command(command, &HashSet::new(), 0).phase
    }

    #[test]
    fn commands_fall_into_phases_by_their_programs() {
        assert_eq!(placed("ls -la /app"), Some(Phase::Orient));
        assert_eq!(placed("cd /app && cat -n src/main.py"), Some(Phase::Read));
        assert_eq!(placed("python3 -m pytest -q tests"), Some(Phase::Test));
        assert_eq!(
            placed("cargo build --release 2>&1 | tail -5"),
            Some(Phase::Build)
        );
        assert_eq!(placed("pip install numpy"), Some(Phase::Build));
        assert_eq!(placed("sed -i 's/a/b/' /app/x.py"), Some(Phase::Edit));
        assert_eq!(
            placed("cat > /app/fix.py <<'EOF'\nprint(1)\nEOF"),
            Some(Phase::Edit)
        );
        assert_eq!(placed("python /tmp/repro.py"), Some(Phase::Test));
        assert_eq!(placed("rm -rf /tmp/work"), Some(Phase::Finish));
        assert_eq!(placed("git status"), Some(Phase::Orient));
        assert_eq!(placed("make -C /app repro"), Some(Phase::Test));
        assert_eq!(
            place_command("sed -i 's/a;b/c/' src/lib.py", &HashSet::new(), 0).writes,
            vec!["src/lib.py".to_owned()]
        );
        // A program the rules can't name is left for Jev.
        assert_eq!(placed("python solve.py --input data.csv"), None);
        assert_eq!(placed("python3 - <<'PY'\nimport json\nPY"), None);
    }

    #[test]
    fn a_heredoc_body_is_not_read_as_commands() {
        let command = "cat > /app/run.sh <<'EOF'\npytest -q\nrm -rf /\nEOF";
        let placed = place_command(command, &HashSet::new(), 0);
        assert_eq!(placed.phase, Some(Phase::Edit));
        assert_eq!(placed.writes, vec!["/app/run.sh".to_owned()]);
    }

    #[test]
    fn reading_an_edited_file_after_an_edit_is_verification() {
        let edited: HashSet<String> = ["/app/out.txt".to_owned()].into();
        assert_eq!(
            place_command("cat /app/out.txt", &edited, 1).phase,
            Some(Phase::Verify)
        );
        assert_eq!(
            place_command("git diff", &edited, 1).phase,
            Some(Phase::Verify)
        );
        assert_eq!(
            place_command("git diff", &edited, 0).phase,
            Some(Phase::Read)
        );
    }

    #[test]
    fn a_codex_script_yields_each_command_and_patch() {
        let record = json!({
            "source": "agent",
            "tool_calls": [{
                "tool_call_id": "c1",
                "function_name": "exec",
                "arguments": {"input": "const patch = \"*** Begin Patch\\n*** Update File: /app/a.py\\n*** End Patch\";\nconst r = await tools.apply_patch(patch);"}
            }, {
                "tool_call_id": "c2",
                "function_name": "exec",
                "arguments": {"input": "const r = await tools.exec_command({cmd:\"ls -la /app\",workdir:\"/app\"}); text(r.output);"}
            }],
            "observation": {"results": [{"source_call_id": "c2", "content": "Process exited with code 0\nOutput:\nx"}]}
        });
        let extracted = extract(&record);
        assert_eq!(extracted.actions.len(), 2);
        assert_eq!(
            extracted.actions[0].kind,
            ActionKind::Edit(vec!["/app/a.py".to_owned()])
        );
        assert_eq!(extracted.actions[1].input, "ls -la /app");
        assert_eq!(extracted.actions[1].failed, Some(false));
    }

    #[test]
    fn claude_stream_results_pair_with_their_calls() {
        let call = json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "pytest -q"}}
        ]}});
        let result = json!({"type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": "t1", "content": "1 failed", "is_error": true}
        ]}});
        let replay = Replay {
            events: vec![event(0), event(1)],
            extracted: vec![extract(&call), extract(&result)],
            ..Replay::default()
        };
        let steps = steps(&replay);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].phase, Some(Phase::Test));
        assert_eq!(steps[0].failed, Some(true));
    }

    fn event(at: u64) -> crate::runs_replay::Event {
        crate::runs_replay::Event {
            elapsed_ms: at,
            timing: "step timestamp",
            title: String::new(),
            text: String::new(),
            parts: Vec::new(),
            record: String::new(),
        }
    }

    #[test]
    fn a_step_state_is_stable_and_names_the_rule_phases_before_it() {
        let record = json!({"source": "agent", "tool_calls": [
            {"tool_call_id": "a", "function_name": "Bash", "arguments": {"command": "ls /app"}},
            {"tool_call_id": "b", "function_name": "Bash", "arguments": {"command": "python solve.py"}}
        ]});
        let replay = Replay {
            events: vec![event(0)],
            extracted: vec![extract(&record)],
            ..Replay::default()
        };
        let steps = steps(&replay);
        let waiting = pending(&steps, "Solve it.");
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].1["before"][0]["phase"], "orient");
        assert_eq!(waiting[0].0, key(&state(&steps, 1, "Solve it.")));
        let mut store = StepStore::open(None);
        store
            .insert(StepAnswer {
                key: waiting[0].0.clone(),
                phase: "test".to_owned(),
                confidence: 0.8,
                probabilities: BTreeMap::new(),
                checks_assumption: 0.2,
                uses_evidence: 0.7,
                retry: 0.1,
                input_tokens: Some(900),
                model: None,
            })
            .unwrap();
        let mut steps = steps;
        apply(&mut steps, "Solve it.", &store);
        assert_eq!(steps[1].phase, Some(Phase::Test));
        assert_eq!(steps[1].by, "jev");
        let labels = event_labels(&replay, "Solve it.", &store);
        assert_eq!(labels[0], "orient, test (Jev)");
    }

    #[test]
    fn the_question_set_asks_one_choice_and_three_nouls() {
        let body = serde_json::to_value(questions()).unwrap();
        assert_eq!(body["phase"]["type"], "choice");
        assert_eq!(body["phase"]["criteria"].as_object().unwrap().len(), 8);
        for id in ["checks_assumption", "uses_evidence", "retry"] {
            assert_eq!(body[id]["type"], "noul", "{id}");
        }
    }

    #[test]
    fn a_repeated_failed_command_is_marked() {
        let record = json!({"source": "agent", "tool_calls": [
            {"tool_call_id": "a", "function_name": "Bash", "arguments": {"command": "make"}},
            {"tool_call_id": "b", "function_name": "Bash", "arguments": {"command": "make"}}
        ], "observation": {"results": [
            {"source_call_id": "a", "content": "error", "extra": {"tool_result_is_error": true}},
            {"source_call_id": "b", "content": "ok"}
        ]}});
        let replay = Replay {
            events: vec![event(0)],
            extracted: vec![extract(&record)],
            ..Replay::default()
        };
        let steps = steps(&replay);
        assert!(!steps[0].repeats_failed);
        assert!(steps[1].repeats_failed);
    }
}
