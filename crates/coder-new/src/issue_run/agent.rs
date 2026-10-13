//! The briefed agent (#11211) on the Claude Agent SDK, drawn with Coder's
//! own tool widgets.
//!
//! The session matches the A/B bench's arm B (`crates/briefed-agent`): the
//! briefing is the whole system prompt; the built-in tools are Read, Edit,
//! Write, Grep and Glob, held to the run's worktree; no settings files or
//! MCP servers load; and the CLI runs on the owner's Claude Code login
//! (`ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` are removed from its
//! environment). The one difference is the check tool: here `run_check` is
//! an SDK-hosted MCP tool that runs in this process (#11213) and runs the
//! briefing's checks in the worktree, on this computer.
//!
//! Each SDK message becomes the transcript item Coder's own tools make:
//! a `tool_use` starts a running `Read`/`Edit`/`Write`/`Grep`/`Glob`/`Run`
//! entry, and its `tool_result` finishes it with the output those widgets
//! read (an Edit's diff from the CLI's structured patch, a Read's line
//! count, a Grep's matches, a check's verdict and trimmed output).

use std::{
    collections::{BTreeSet, HashMap},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use acp_client::{PermissionAnswer, PermissionRequest, StopReason, Update};
use claude_agent_sdk::{
    PermissionMode, PermissionResult, QueryOptions, SdkMcpServer, SdkMessage, SystemPromptConfig,
    ToolResult, ToolsConfig,
};
use futures::StreamExt;
use serde_json::{Value, json};

use super::{
    Transcript,
    decide::{Briefing, Check},
    setup::Base,
};
use crate::live::Entry;

/// Credentials the CLI must not inherit, so it uses the Claude Code login.
const REMOVED_ENV: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
/// The built-in tools the agent has.
const TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Grep", "Glob"];
/// The file tools whose paths must stay inside the worktree.
const FILE_TOOLS: [&str; 6] = ["Read", "Edit", "MultiEdit", "Write", "Glob", "Grep"];
/// The longest check output the agent reads.
const CHECK_LIMIT: usize = 7000;

/// What the agent did and what it cost.
#[derive(Clone, Debug, Default)]
pub struct Work {
    pub wall_ms: u64,
    pub turns: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub cost_usd: Option<f64>,
    /// Files the agent opened that the briefing did not list.
    pub misses: Vec<String>,
    /// Each check the agent ran, with its last verdict.
    pub checks: Vec<(String, bool)>,
    pub error: Option<String>,
    pub reply: Option<String>,
}

fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// `path` made absolute against `root` and normalized lexically.
fn resolve(root: &Path, path: &str) -> PathBuf {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        root.join(path)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The worktree-relative path a tool input names: `Some(None)` when it
/// names none, `None` when the path leaves the worktree.
#[must_use]
pub fn inside(root: &Path, input: &Value) -> Option<Option<String>> {
    let Some(path) = ["file_path", "path", "notebook_path"]
        .iter()
        .find_map(|key| input[*key].as_str())
    else {
        return Some(None);
    };
    let resolved = resolve(root, path);
    let relative = resolved.strip_prefix(root).ok()?;
    Some(Some(relative.to_string_lossy().into_owned()))
}

/// `path` as an absolute, resolved name: relative to `base`, `.` and `..`
/// taken lexically, and the longest part that exists resolved through its
/// symbolic links, so `/tmp` and `/private/tmp` compare equal. Carried
/// over from `crates/microcoder`'s Devin boundary and reimplemented here.
#[must_use]
pub fn resolved(base: &Path, path: &str) -> PathBuf {
    let mut lexical = PathBuf::new();
    for component in base.join(path).components() {
        match component {
            Component::ParentDir => {
                lexical.pop();
            }
            Component::CurDir => {}
            other => lexical.push(other),
        }
    }
    let mut existing = lexical.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return lexical,
        }
    }
}

/// The checks' argv with a test filter's words appended.
#[must_use]
pub fn check_argv(check: &Check, filter: Option<&str>) -> Vec<String> {
    let mut argv = check.argv.clone();
    // `cargo test` rebuilds and runs every doc test on each call, whatever
    // the filter: minutes per check on a big crate. Unit and integration
    // tests are what a change is checked against.
    if argv.get(..2) == Some(&["cargo".to_owned(), "test".to_owned()][..])
        && !argv.iter().any(|a| a == "--doc" || a == "--tests")
    {
        argv.insert(2, "--tests".into());
    }
    if check.filter
        && let Some(filter) = filter
    {
        let words: Vec<String> = filter
            .split_whitespace()
            .filter(|word| {
                word.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
            })
            .take(8)
            .map(str::to_owned)
            .collect();
        if !words.is_empty() {
            argv.push("--".into());
            argv.extend(words);
        }
    }
    argv
}

/// A check's output cut to what the agent needs: the compiler's errors and
/// failing tests when it failed, the test totals when it passed. Ported
/// from the bench's `checks_mcp.py`.
#[must_use]
pub fn trimmed(text: &str, ok: bool) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if ok {
        let kept: Vec<&str> = lines
            .iter()
            .filter(|line| line.starts_with("test result:") || line.starts_with("warning: unused"))
            // A test binary the filter left empty says nothing.
            .filter(|line| !line.starts_with("test result: ok. 0 passed; 0 failed"))
            .copied()
            .collect();
        let tail = kept.len().saturating_sub(12);
        let summary = if kept.is_empty() {
            lines.last().copied().unwrap_or_default().to_owned()
        } else {
            kept[tail..].join("\n")
        };
        return format!("ok\n{summary}");
    }
    let mut out = Vec::new();
    let mut block = false;
    for line in &lines {
        if line.starts_with("error")
            || line.starts_with("warning: unused")
            || line.starts_with("---- ")
            || line.contains("panicked at")
        {
            block = true;
        } else if line.starts_with("test result:")
            || line.starts_with("failures:")
            || (line.starts_with("test ") && line.ends_with("... FAILED"))
        {
            out.push(*line);
            block = false;
            continue;
        } else if block && line.trim().is_empty() {
            out.push("");
            block = false;
            continue;
        }
        if block {
            out.push(line);
        }
    }
    let mut body = out.join("\n").trim().to_owned();
    if body.is_empty() {
        body = lines[lines.len().saturating_sub(60)..].join("\n");
    }
    if body.len() > CHECK_LIMIT {
        let mut cut = CHECK_LIMIT;
        while !body.is_char_boundary(cut) {
            cut -= 1;
        }
        body.truncate(cut);
        body.push_str("\n... (cut; fix the first errors and run again)");
    }
    body
}

/// Runs one of the briefing's checks in `worktree`.
pub async fn run_check(
    checks: &[Check],
    worktree: &Path,
    id: &str,
    filter: Option<&str>,
) -> (String, bool) {
    let Some(check) = checks.iter().find(|check| check.id == id) else {
        return (
            format!(
                "Unknown check. Available: {}",
                checks
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            false,
        );
    };
    let argv = check_argv(check, filter);
    let Some((program, args)) = argv.split_first() else {
        return ("This check has no command.".into(), false);
    };
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(worktree)
        .stdin(std::process::Stdio::null());
    // Every run's worktree is new, so its own target folder would build the
    // crates from scratch each time.
    shared_target(&mut command);
    let result = command.output().await;
    let Ok(out) = result else {
        return (format!("Cannot run {program}."), false);
    };
    let ok = out.status.success();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (
        format!(
            "{}: {}\n{}",
            argv.join(" "),
            if ok { "passed" } else { "FAILED" },
            trimmed(&text, ok)
        ),
        ok,
    )
}

/// The Coder tool name and input for an SDK tool call.
#[must_use]
pub fn coder_call(name: &str, input: &Value, root: &Path, checks: &[Check]) -> (String, Value) {
    let path = || match inside(root, input) {
        Some(Some(relative)) => relative,
        _ => ["file_path", "path"]
            .iter()
            .find_map(|key| input[*key].as_str())
            .unwrap_or_default()
            .to_owned(),
    };
    match name {
        "Read" => ("Read".into(), json!({"path": path()})),
        "Edit" | "MultiEdit" => ("Edit".into(), json!({"path": path()})),
        "Write" => ("Write".into(), json!({"path": path()})),
        "Grep" | "Glob" => (
            name.into(),
            json!({"pattern": input["pattern"], "path": input["path"]}),
        ),
        _ if name.ends_with("__run_check") => {
            let id = input["check"].as_str().unwrap_or_default();
            let command = checks.iter().find(|check| check.id == id).map_or_else(
                || id.to_owned(),
                |check| check_argv(check, input["filter"].as_str()).join(" "),
            );
            ("Run".into(), json!({"command": command, "check": id}))
        }
        _ => (name.into(), input.clone()),
    }
}

/// The text of a `tool_result` block.
pub(crate) fn result_text(block: &Value) -> String {
    match &block["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// A structured patch (`[{oldStart, oldLines, newStart, newLines, lines}]`)
/// as unified `@@` hunks, with its added and removed line counts.
#[must_use]
pub fn patch_text(patch: &Value) -> (String, usize, usize) {
    let mut text = String::new();
    let (mut added, mut removed) = (0, 0);
    for hunk in patch.as_array().into_iter().flatten() {
        text.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            hunk["oldStart"].as_u64().unwrap_or(0),
            hunk["oldLines"].as_u64().unwrap_or(0),
            hunk["newStart"].as_u64().unwrap_or(0),
            hunk["newLines"].as_u64().unwrap_or(0),
        ));
        for line in hunk["lines"].as_array().into_iter().flatten() {
            let line = line.as_str().unwrap_or_default();
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
            text.push_str(line);
            text.push('\n');
        }
    }
    (text, added, removed)
}

/// The output Coder's widget for `coder_name` reads, from an SDK tool
/// result: its text, the CLI's structured result when it sent one, and
/// whether it failed.
#[must_use]
pub fn coder_output(
    coder_name: &str,
    sdk_input: &Value,
    coder_input: &Value,
    text: &str,
    structured: Option<&Value>,
    is_error: bool,
) -> Value {
    let path = coder_input["path"].clone();
    if coder_name == "Run" {
        let (head, body) = text.split_once('\n').unwrap_or((text, ""));
        return if is_error || head.ends_with("FAILED") {
            json!({"output": body.trim_end(), "error": "The check failed."})
        } else {
            json!({"output": body.trim_end(), "result": "passed"})
        };
    }
    if is_error {
        let first = text
            .trim()
            .trim_start_matches("<tool_use_error>")
            .lines()
            .next()
            .unwrap_or("The tool failed.")
            .trim_end_matches("</tool_use_error>")
            .to_owned();
        return json!({"error": first});
    }
    match coder_name {
        "Read" => {
            let file = structured.map(|s| &s["file"]);
            let total = file
                .and_then(|f| f["totalLines"].as_u64().or(f["numLines"].as_u64()))
                .unwrap_or_else(|| text.lines().count() as u64);
            json!({"path": path, "total_lines": total})
        }
        "Edit" | "Write" => {
            let patch = structured.map(|s| &s["structuredPatch"]);
            let (diff, added, removed) = match patch {
                Some(patch) if patch.as_array().is_some_and(|p| !p.is_empty()) => patch_text(patch),
                _ => {
                    let (old, new) = if coder_name == "Write" {
                        ("", sdk_input["content"].as_str().unwrap_or_default())
                    } else {
                        (
                            sdk_input["old_string"].as_str().unwrap_or_default(),
                            sdk_input["new_string"].as_str().unwrap_or_default(),
                        )
                    };
                    let diff = crate::file_tools::unified(old, new);
                    (diff.text, diff.added, diff.removed)
                }
            };
            json!({"path": path, "diff": diff, "added": added, "removed": removed})
        }
        "Grep" => {
            let s = structured.cloned().unwrap_or(Value::Null);
            let files = s["numFiles"].as_u64();
            let names: Vec<&str> = s["filenames"]
                .as_array()
                .map(|n| n.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let found = |text: &str| !text.trim().is_empty() && !text.trim().starts_with("No ");
            let (matches, count) = match s["content"].as_str().filter(|c| found(c)) {
                Some(content) => (
                    content.to_owned(),
                    s["numLines"]
                        .as_u64()
                        .unwrap_or(content.lines().count() as u64),
                ),
                None if !names.is_empty() => (names.join("\n"), names.len() as u64),
                None if s.is_object() || !found(text) => (String::new(), 0),
                None => (text.to_owned(), text.lines().count() as u64),
            };
            // Content mode reports no file count; count the files its
            // `path:line:` rows name.
            let distinct = {
                let mut seen: Vec<&str> = matches
                    .lines()
                    .filter_map(|row| row.split_once(':').map(|(file, _)| file))
                    .filter(|file| file.contains('.') || file.contains('/'))
                    .collect();
                seen.sort_unstable();
                seen.dedup();
                seen.len() as u64
            };
            let files = files.filter(|n| *n > 0).unwrap_or(if names.is_empty() {
                distinct
            } else {
                names.len() as u64
            });
            // Rows without a file name come from a search of one file.
            let files = if files == 0 && count > 0 { 1 } else { files };
            json!({"matches": matches, "count": count, "files": files})
        }
        "Glob" => {
            let names: Vec<&str> = structured
                .and_then(|s| s["filenames"].as_array())
                .map(|n| n.iter().filter_map(Value::as_str).collect())
                .unwrap_or_else(|| text.lines().collect());
            json!({"files": names.join("\n"), "count": names.len()})
        }
        _ => json!({"output": text}),
    }
}

/// What the stream loop keeps about each running tool call.
struct Call {
    index: usize,
    coder_name: String,
    coder_input: Value,
    sdk_input: Value,
}

/// The agent runtime a session runs on.
#[derive(Clone, Debug)]
pub enum Engine {
    /// A `claude_agent_sdk` session on this computer's Claude Code login.
    Claude {
        /// The built-in tools; file tools among them are held to the
        /// worktree.
        tools: Vec<String>,
        /// An in-process tool server of the hand's own, beside `run_check`.
        server: Option<SdkMcpServer>,
    },
    /// An ACP session (`devin acp`), opened as [`AcpOptions`] describes.
    Acp(AcpOptions),
}

/// How an ACP session opens: the agent's binary, arguments, mode, session
/// `_meta` and whole environment.
#[derive(Clone, Debug)]
pub struct AcpOptions {
    /// The agent binary; `None` when no installed agent was found, which
    /// surfaces as the session's start error.
    pub program: Option<PathBuf>,
    /// The binary's arguments: `devin --sandbox acp [--model NAME]` for a
    /// solver hand.
    pub arguments: Vec<String>,
    /// The session's mode, by the agent's own id (`accept-edits`). A
    /// solver hand never runs `bypass`.
    pub mode: Option<String>,
    /// The session's `_meta` (the engine mark).
    pub meta: Option<Value>,
    /// The agent's whole environment, credentials scrubbed.
    pub environment: Vec<(String, String)>,
    /// Whether the hand may change files at all: the Coder may, the other
    /// hands may not, so a writing tool is refused even inside the
    /// worktree.
    pub writes: bool,
}

/// The engine mark a solver hand's ACP session opens with, the same mark
/// Coder's other engine sessions carry.
const ENGINE_MARK: &str = "openagents-coder-engine";

impl AcpOptions {
    /// A solver hand's `devin acp` session on `model`: `--sandbox`,
    /// `accept-edits` mode, the engine mark in its `_meta`, and the
    /// credential-scrubbed environment with the shared build folder.
    /// `writes` says whether the hand may change files.
    #[must_use]
    pub fn devin(model: &str, writes: bool) -> Self {
        let mut arguments = acp_client::devin::arguments(model);
        arguments.insert(0, "--sandbox".into());
        Self {
            program: acp_client::devin::binary(&|name| std::env::var_os(name)),
            arguments,
            mode: Some(
                acp_client::devin::Permission::AcceptEdits
                    .mode_id()
                    .to_owned(),
            ),
            meta: Some(acp_client::devin::engine_meta(ENGINE_MARK)),
            environment: acp_environment(),
            writes,
        }
    }
}

/// The ACP agent's whole environment: this process's, less credential
/// variables, plus the child environment a fleet worker set, and the
/// shared build folder so a `cargo` run in the worktree builds beside the
/// other runs instead of cold-building a `target/` of its own.
#[must_use]
pub fn acp_environment() -> Vec<(String, String)> {
    let mut environment: Vec<(String, String)> = std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(key, _)| !acp_client::process::is_credential_name(key))
        .collect();
    for (name, value) in crate::bundled_runtime::child_env() {
        let name = name.to_string_lossy().into_owned();
        environment.retain(|(existing, _)| existing != &name);
        environment.push((name, value.to_string_lossy().into_owned()));
    }
    if !environment.iter().any(|(key, _)| key == "CARGO_TARGET_DIR")
        && let Some(home) = std::env::var_os("HOME")
    {
        environment.push((
            "CARGO_TARGET_DIR".into(),
            Path::new(&home)
                .join(".openagents")
                .join(crate::home::DIR)
                .join("target")
                .display()
                .to_string(),
        ));
    }
    environment
}

/// One constrained agent session: its model, its engine, the checks it may
/// run, and what it is told. `issue_run` runs one, the briefed agent;
/// `solver` runs four, each with the tools its hand needs.
pub struct Session {
    /// The agent card's title and detail line.
    pub title: String,
    pub detail: String,
    pub model: String,
    pub timeout: Duration,
    /// The agent runtime: a `claude_agent_sdk` session holding only the
    /// tools its hand needs, or a `devin acp` session held to the
    /// worktree.
    pub engine: Engine,
    /// The checks a `run_check` tool offers; none means no such tool. An
    /// ACP session cannot host an in-process tool, so a Devin hand lists
    /// the check commands in its prompt instead.
    pub checks: Vec<Check>,
    /// The session's system prompt on the Claude engine; on an ACP session
    /// it is folded into the prompt, which is the only text channel.
    pub system: String,
    pub prompt: String,
    /// The files the briefing listed; a `Read` of another is a miss.
    pub briefed: BTreeSet<String>,
    /// The file the session's raw messages are written to, in the run's
    /// folder.
    pub events_file: String,
    /// What a failed test is compared against: the repository and the
    /// commit the run started from. A test that already fails there is not
    /// the change's doing, so `run_check` doesn't fail on it. Only the
    /// Claude engine consumes it.
    pub baseline: Option<Baseline>,
}

/// The commit a run's tests are compared against, and a checkout to run
/// them in.
#[derive(Clone, Debug)]
pub struct Baseline {
    pub repo: PathBuf,
    pub commit: String,
    /// Where the baseline checkout goes (a sibling of the run's worktree).
    pub worktree: PathBuf,
}

impl Baseline {
    #[must_use]
    pub fn of(base: &Base) -> Self {
        Self {
            repo: base.repo.clone(),
            commit: base.base.clone(),
            worktree: base
                .worktree
                .parent()
                .map_or_else(|| base.worktree.join("baseline"), |p| p.join("baseline")),
        }
    }

    /// The baseline checkout, made on first use.
    async fn checkout(&self) -> Option<&Path> {
        if !self.worktree.join(".git").exists() {
            let status = tokio::process::Command::new("git")
                .args(["worktree", "add", "--detach", "--force"])
                .arg(&self.worktree)
                .arg(&self.commit)
                .current_dir(&self.repo)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .ok()?;
            if !status.success() {
                return None;
            }
        }
        Some(&self.worktree)
    }

    /// Removes the baseline checkout once the run has ended. It is a full
    /// checkout of the repository (about 2 GB here), and every run used to
    /// leave one behind beside its working copy.
    pub async fn remove(&self) {
        if !self.worktree.join(".git").exists() {
            return;
        }
        let _ = tokio::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&self.worktree)
            .current_dir(&self.repo)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        let _ = std::fs::remove_dir_all(&self.worktree);
    }
}

/// The tests a `cargo test` output reports as failed, in order.
#[must_use]
pub fn failing_tests(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(name) = line
            .trim()
            .strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
            && !names.iter().any(|seen| seen == name)
        {
            names.push(name.to_owned());
        }
    }
    names
}

/// Which of `names` also fail at the baseline: `None` when the baseline
/// could not be checked out or built, so nothing is excused.
pub async fn preexisting_failures(
    baseline: &Baseline,
    check: &Check,
    names: &[String],
) -> Option<Vec<String>> {
    let worktree = baseline.checkout().await?;
    let mut argv = check_argv(check, None);
    argv.push("--".into());
    argv.push("--exact".into());
    argv.extend(names.iter().cloned());
    let (program, args) = argv.split_first()?;
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .current_dir(worktree)
        .stdin(std::process::Stdio::null());
    shared_target(&mut command);
    let out = command.output().await.ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if text.contains("error[E") || text.contains("error: could not compile") {
        return None;
    }
    let failed = failing_tests(&text);
    Some(
        names
            .iter()
            .filter(|name| failed.contains(name))
            .cloned()
            .collect(),
    )
}

/// `run_check`, then for a failed test check, the baseline's say: when
/// every failed test already fails there the check passes with a note, and
/// otherwise the note names the failures that were there before.
pub async fn run_check_against(
    checks: &[Check],
    worktree: &Path,
    baseline: Option<&Baseline>,
    id: &str,
    filter: Option<&str>,
) -> (String, bool) {
    let (text, ok) = run_check(checks, worktree, id, filter).await;
    let Some(baseline) = baseline else {
        return (text, ok);
    };
    let Some(check) = checks.iter().find(|check| check.id == id) else {
        return (text, ok);
    };
    if ok || !check.id.starts_with("test:") {
        return (text, ok);
    }
    let names = failing_tests(&text);
    if names.is_empty() {
        return (text, ok);
    }
    let Some(before) = preexisting_failures(baseline, check, &names).await else {
        return (text, ok);
    };
    if before.is_empty() {
        return (text, ok);
    }
    let short: Vec<&str> = before.iter().map(String::as_str).collect();
    if before.len() == names.len() {
        let text = text.replacen(": FAILED\n", ": passed\n", 1);
        (
            format!(
                "{text}

PASSED anyway: every failed test ({}) already fails at the base commit {}, before this change.",
                short.join(", "),
                &baseline.commit[..baseline.commit.len().min(10)]
            ),
            true,
        )
    } else {
        (
            format!(
                "{text}

Already failing at the base commit before this change, not yours to fix: {}. The other failures are the change's.",
                short.join(", ")
            ),
            false,
        )
    }
}

/// Runs share one build folder, unless the person set their own.
fn shared_target(command: &mut tokio::process::Command) {
    if std::env::var_os("CARGO_TARGET_DIR").is_none()
        && let Some(home) = std::env::var_os("HOME")
    {
        command.env(
            "CARGO_TARGET_DIR",
            std::path::Path::new(&home)
                .join(".openagents")
                .join(crate::home::DIR)
                .join("target"),
        );
    }
}

/// Free space below which a finished run sweeps the shared build folder.
const SWEEP_BELOW_BYTES: u64 = 60 * 1024 * 1024 * 1024;
/// Build outputs untouched this long are swept.
const SWEEP_AGE: std::time::Duration = std::time::Duration::from_secs(2 * 60 * 60);

/// Sweeps stale outputs from the shared build folder when the disk is low.
/// Every run builds its workspace crates again (their build hashes include
/// the worktree's path), so each run leaves ~20 GB in the folder that no
/// later run reuses; three runs took the Mac from 45 GB free to 6 GB.
/// Outputs older than [`SWEEP_AGE`] go; cargo rebuilds any it still needs.
/// A build folder the person chose (`CARGO_TARGET_DIR`) is never touched.
pub async fn sweep_shared_target() {
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let target = std::path::Path::new(&home)
        .join(".openagents")
        .join(crate::home::DIR)
        .join("target");
    if !target.is_dir()
        || free_bytes(&target)
            .await
            .is_none_or(|free| free >= SWEEP_BELOW_BYTES)
    {
        return;
    }
    let _ = tokio::task::spawn_blocking(move || sweep(&target, SWEEP_AGE)).await;
}

/// The free bytes on the disk holding `path`, from `df`.
async fn free_bytes(path: &Path) -> Option<u64> {
    let out = tokio::process::Command::new("df")
        .arg("-Pk")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let kib: u64 = text
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    Some(kib * 1024)
}

/// Removes the files under each profile's `deps`, `build`, `.fingerprint`
/// and `incremental` folders in `target` last changed more than `age` ago,
/// and the folders that leaves empty. How many files went.
pub fn sweep(target: &Path, age: std::time::Duration) -> usize {
    fn walk(dir: &Path, cutoff: std::time::SystemTime, removed: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                walk(&path, cutoff, removed);
                let _ = std::fs::remove_dir(&path);
            } else if meta.modified().is_ok_and(|at| at < cutoff)
                && std::fs::remove_file(&path).is_ok()
            {
                *removed += 1;
            }
        }
    }
    let Some(cutoff) = std::time::SystemTime::now().checked_sub(age) else {
        return 0;
    };
    let mut removed = 0;
    let Ok(profiles) = std::fs::read_dir(target) else {
        return 0;
    };
    for profile in profiles.flatten() {
        let Ok(entries) = std::fs::read_dir(profile.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(name.as_ref(), "deps" | "build" | ".fingerprint")
                || name.starts_with("incremental")
            {
                walk(&entry.path(), cutoff, &mut removed);
            }
        }
    }
    removed
}

impl Session {
    /// The briefed agent of `coder issue-run`: Read, Edit, Write, Grep,
    /// Glob and `run_check` over the briefing's checks.
    #[must_use]
    pub fn briefed(options: &super::Options, briefing: &Briefing) -> Self {
        Self {
            title: "Briefed agent".into(),
            detail: "Claude Agent SDK session on this computer's Claude Code login".into(),
            model: options.model.clone(),
            timeout: options.timeout,
            engine: Engine::Claude {
                tools: TOOLS.iter().map(|t| (*t).to_owned()).collect(),
                server: None,
            },
            checks: briefing.checks.clone(),
            system: briefing.system.clone(),
            prompt: briefing.prompt.clone(),
            briefed: briefing.files.iter().cloned().collect(),
            events_file: "agent-events.jsonl".into(),
            baseline: None,
        }
    }

    /// What the card's `tools` row says.
    #[must_use]
    pub fn tools_text(&self) -> String {
        match &self.engine {
            Engine::Claude { tools, server } => {
                let mut names = tools.clone();
                if !self.checks.is_empty() {
                    names.push("run_check".into());
                }
                if let Some(server) = server {
                    names.extend(server.allowed_tool_names());
                }
                names.join(", ")
            }
            Engine::Acp(_) => "the agent's own tools".into(),
        }
    }
}

/// Runs the briefed agent to the end, drawing its work as it goes.
pub async fn work(
    options: &super::Options,
    base: &Base,
    briefing: &Briefing,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    run(
        Session::briefed(options, briefing),
        &base.worktree,
        folder,
        transcript,
    )
    .await
}

/// The `run_check` tool server over `checks`, and the verdicts it records.
fn checks_server(
    checks: Arc<Vec<Check>>,
    root: PathBuf,
    baseline: Option<Baseline>,
) -> (SdkMcpServer, Arc<Mutex<Vec<(String, bool)>>>) {
    let verdicts: Arc<Mutex<Vec<(String, bool)>>> = Arc::default();
    let server = {
        let verdicts = verdicts.clone();
        SdkMcpServer::new("checks", "1.0.0").tool(
            "run_check",
            format!(
                "Run one of the briefing's checks against your working copy. Checks: {}",
                checks
                    .iter()
                    .map(|c| format!("{} ({})", c.id, c.what))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            json!({
                "type": "object",
                "properties": {
                    "check": {"type": "string", "enum": checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>()},
                    "filter": {"type": "string", "description": "test-name filter words (test checks only)"}
                },
                "required": ["check"]
            }),
            move |args| {
                let (checks, root, verdicts) = (checks.clone(), root.clone(), verdicts.clone());
                let baseline = baseline.clone();
                async move {
                    let id = args["check"].as_str().unwrap_or_default().to_owned();
                    let (text, ok) = run_check_against(
                        &checks,
                        &root,
                        baseline.as_ref(),
                        &id,
                        args["filter"].as_str(),
                    )
                    .await;
                    let mut verdicts = lock(&verdicts);
                    verdicts.retain(|(seen, _)| *seen != id);
                    verdicts.push((id, ok));
                    Ok(if ok {
                        ToolResult::text(text)
                    } else {
                        ToolResult::error(text)
                    })
                }
            },
        )
    };
    (server, verdicts)
}

/// Runs one session to the end in `worktree`, drawing its work as it goes.
/// The session's engine picks the driver: the Claude Agent SDK, or an ACP
/// agent ([`run_acp`]).
pub async fn run(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    if matches!(session.engine, Engine::Acp(_)) {
        return run_acp(session, worktree, folder, transcript).await;
    }
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let checks = Arc::new(session.checks.clone());
    let tools_text = session.tools_text();
    let Engine::Claude { tools, server } = session.engine else {
        unreachable!("the ACP arm returned");
    };
    let mut allowed: Vec<String> = tools.clone();
    let mut query_options = QueryOptions::new()
        .cwd(&root)
        .model(session.model.clone())
        .permission_mode(PermissionMode::Default)
        .no_session_persistence();
    let mut verdicts = None;
    if !checks.is_empty() {
        let (check_server, seen) =
            checks_server(checks.clone(), root.clone(), session.baseline.clone());
        allowed.extend(check_server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(check_server);
        verdicts = Some(seen);
    }
    if let Some(server) = server {
        allowed.extend(server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(server);
    }
    // Token savers (#11494): once the session has Bash, its noisy commands
    // run through the squeezer (#11486). The full output is a file outside
    // the worktree, which the agent reads with Bash.
    if tools.iter().any(|tool| tool == "Bash") {
        query_options =
            crate::token_savers::install(query_options, crate::token_savers::Lever::Squeeze);
    }
    query_options.tools = Some(ToolsConfig::Names(tools));
    query_options.allowed_tools = Some(allowed.clone());
    query_options.system_prompt = Some(SystemPromptConfig::Custom(session.system.clone()));
    query_options.setting_sources = Some(Vec::new());
    query_options.strict_mcp_config = true;
    query_options.env_remove = REMOVED_ENV.iter().map(|n| (*n).to_owned()).collect();

    let card = json!({
        "kind": "agent",
        "title": session.title,
        "detail": session.detail,
        "rows": [
            {"label": "model", "text": session.model},
            {"label": "tools", "text": tools_text},
            {"label": "checks", "text": checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>().join(", ")},
            {"label": "briefing", "text": format!("{} files, {} characters in the system prompt", session.briefed.len(), session.system.chars().count())},
        ],
    });
    let card_index = transcript.card(card.clone());

    let briefed = session.briefed;
    let misses: Arc<Mutex<Vec<String>>> = Arc::default();
    let handler = {
        let (root, allowed) = (root.clone(), allowed.clone());
        claude_agent_sdk::permission_handler(move |request| {
            let named = allowed.iter().any(|name| *name == request.tool_name);
            let decision = if FILE_TOOLS.contains(&request.tool_name.as_str()) {
                if named && inside(&root, &request.input).is_some() {
                    PermissionResult::allow(request.input.clone())
                } else {
                    PermissionResult::deny(
                        "That path is outside the task's worktree. Work only inside the worktree.",
                    )
                }
            } else if named {
                PermissionResult::allow(request.input.clone())
            } else {
                PermissionResult::deny("That tool is not available in this task.")
            };
            async move { Ok::<_, claude_agent_sdk::Error>(decision) }
        })
    };

    let started = Instant::now();
    let mut work = Work::default();
    let mut events = String::new();
    let mut query = match claude_agent_sdk::query_with_permissions(
        session.prompt,
        query_options,
        handler,
    )
    .await
    {
        Ok(query) => query,
        Err(error) => {
            let why = format!("Claude Code did not start: {error}");
            transcript.finish(card_index, card, json!({"error": why.clone()}));
            work.error = Some(why);
            return work;
        }
    };
    transcript.finish(card_index, card, json!({"ok": true}));
    let mut calls: HashMap<String, Call> = HashMap::new();
    let mut opened: BTreeSet<String> = BTreeSet::new();
    loop {
        if transcript.cancelled() {
            work.error = Some("The run was stopped.".into());
            break;
        }
        let left = session.timeout.saturating_sub(started.elapsed());
        if left.is_zero() {
            work.error = Some("The agent ran out of its time.".into());
            break;
        }
        let next =
            match tokio::time::timeout(left.min(Duration::from_millis(500)), query.next()).await {
                Ok(next) => next,
                Err(_) => continue,
            };
        let message = match next {
            None => break,
            Some(Err(claude_agent_sdk::Error::UnrecognizedMessage { .. })) => continue,
            Some(Err(error)) => {
                work.error = Some(error.to_string());
                break;
            }
            Some(Ok(message)) => message,
        };
        let t_ms = started.elapsed().as_millis() as u64;
        match message {
            SdkMessage::Assistant(assistant) => {
                events.push_str(
                    &json!({"type": "assistant", "t_ms": t_ms, "message": assistant.message})
                        .to_string(),
                );
                events.push('\n');
                if assistant.parent_tool_use_id.is_some() {
                    continue;
                }
                let model = assistant.message["model"].as_str().map(str::to_owned);
                for block in assistant.message["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or_default().trim();
                            if !text.is_empty() {
                                transcript.push(Entry::Assistant {
                                    text: text.to_owned(),
                                    model: model.clone(),
                                    elapsed_ms: Some(t_ms),
                                });
                            }
                        }
                        Some("tool_use") => {
                            let name = block["name"].as_str().unwrap_or("?");
                            let input = block["input"].clone();
                            if name == "Read"
                                && let Some(Some(path)) = inside(&root, &input)
                                && !briefed.contains(&path)
                                && opened.insert(path.clone())
                            {
                                lock(&misses).push(path);
                            }
                            let (coder_name, coder_input) =
                                coder_call(name, &input, &root, &checks);
                            let index = transcript.push(Entry::Tool {
                                name: coder_name.clone(),
                                input: coder_input.clone(),
                                output: Value::Null,
                                running: true,
                            });
                            if let Some(id) = block["id"].as_str() {
                                calls.insert(
                                    id.to_owned(),
                                    Call {
                                        index,
                                        coder_name,
                                        coder_input,
                                        sdk_input: input,
                                    },
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            SdkMessage::User(user) => {
                events.push_str(&json!({"type": "user", "t_ms": t_ms, "message": user.message, "tool_use_result": user.tool_use_result}).to_string());
                events.push('\n');
                let blocks: Vec<&Value> = user.message["content"]
                    .as_array()
                    .map(|b| b.iter().filter(|b| b["type"] == "tool_result").collect())
                    .unwrap_or_default();
                let single = blocks.len() == 1;
                for block in blocks {
                    let Some(call) = block["tool_use_id"]
                        .as_str()
                        .and_then(|id| calls.remove(id))
                    else {
                        continue;
                    };
                    let structured = if single {
                        user.tool_use_result.as_ref().filter(|v| v.is_object())
                    } else {
                        None
                    };
                    let output = coder_output(
                        &call.coder_name,
                        &call.sdk_input,
                        &call.coder_input,
                        &result_text(block),
                        structured,
                        block["is_error"].as_bool().unwrap_or(false),
                    );
                    transcript.set(
                        call.index,
                        Entry::Tool {
                            name: call.coder_name,
                            input: call.coder_input,
                            output,
                            running: false,
                        },
                    );
                }
            }
            SdkMessage::Result(done) => {
                let value = serde_json::to_value(&done).unwrap_or(Value::Null);
                events.push_str(
                    &json!({"type": "result", "t_ms": t_ms, "result": value}).to_string(),
                );
                events.push('\n');
                read_result(&value, &mut work);
                break;
            }
            SdkMessage::Unknown { type_name, raw } if type_name == "result" => {
                read_result(&raw, &mut work);
                break;
            }
            _ => {}
        }
    }
    let _ = query.kill().await;
    for (_, call) in calls {
        transcript.set(
            call.index,
            Entry::Tool {
                name: call.coder_name,
                input: call.coder_input,
                output: json!({"error": "The agent stopped before this tool returned."}),
                running: false,
            },
        );
    }
    let _ = std::fs::write(folder.join(&session.events_file), events);
    work.wall_ms = started.elapsed().as_millis() as u64;
    work.misses = lock(&misses).clone();
    work.checks = verdicts.map(|v| lock(&v).clone()).unwrap_or_default();
    work
}

fn read_result(value: &Value, work: &mut Work) {
    let usage = &value["usage"];
    work.turns = value["num_turns"].as_u64();
    work.cost_usd = value["total_cost_usd"].as_f64();
    work.input_tokens = usage["input_tokens"].as_u64();
    work.output_tokens = usage["output_tokens"].as_u64();
    work.cache_read = usage["cache_read_input_tokens"].as_u64();
    work.cache_write = usage["cache_creation_input_tokens"].as_u64();
    work.reply = value["result"].as_str().map(str::to_owned);
    if value["is_error"].as_bool() == Some(true) {
        work.error = Some(
            value["errors"]
                .as_array()
                .and_then(|errors| errors.first())
                .and_then(Value::as_str)
                .or(value["result"].as_str())
                .unwrap_or("The agent's session ended on an error.")
                .to_owned(),
        );
    }
}

/// After the agent: each `check:` the briefing names, run by the harness
/// and shown as a Run.
pub async fn final_checks(
    base: &Base,
    briefing: &Briefing,
    transcript: &mut Transcript,
) -> Vec<(String, bool)> {
    final_check_outputs(base, briefing, &[], transcript)
        .await
        .into_iter()
        .map(|(id, ok, _)| (id, ok))
        .collect()
}

/// Whether the harness runs `check` after the agent: compile checks
/// (`check:`, and `wasm:` for a browser build) and any check the briefing
/// marks required (a docs link check, a script's tests). A required check
/// the harness never ran would fail the run with no result.
#[must_use]
pub fn harness_runs(check: &Check) -> bool {
    check.id.starts_with("check:") || check.id.starts_with("wasm:") || check.required
}

/// The Coder tool name and input an ACP tool call draws as: its `kind` and
/// `rawInput` onto the same `Read`/`Edit`/`Write`/`Grep`/`Run` names the
/// Claude session's calls produce ([`coder_call`]). A call whose kind has
/// no file or command shape draws a `Run` showing its own title.
#[must_use]
pub fn acp_call(kind: &str, title: &str, raw_input: &Value, root: &Path) -> (String, Value) {
    let path = |input: &Value| match inside(root, input) {
        Some(Some(relative)) => relative,
        _ => [
            "file_path",
            "path",
            "filePath",
            "target_file",
            "source_path",
        ]
        .iter()
        .find_map(|key| input[*key].as_str())
        .unwrap_or_default()
        .to_owned(),
    };
    match kind {
        "read" => ("Read".into(), json!({"path": path(raw_input)})),
        "edit" | "move" => ("Edit".into(), json!({"path": path(raw_input)})),
        "write" | "create" => ("Write".into(), json!({"path": path(raw_input)})),
        "search" => {
            let pattern = ["pattern", "query", "regex", "text"]
                .iter()
                .find_map(|key| raw_input[*key].as_str())
                .unwrap_or(title);
            (
                "Grep".into(),
                json!({"pattern": pattern, "path": raw_input["path"]}),
            )
        }
        "execute" => (
            "Run".into(),
            json!({"command": raw_input["command"].as_str().unwrap_or(title)}),
        ),
        _ => ("Run".into(), json!({"command": title})),
    }
}

/// The output a finished ACP tool call's widget reads, in the same shape
/// [`coder_output`] gives the Claude session's results.
#[must_use]
pub fn acp_output(name: &str, input: &Value, text: &str, status: &str) -> Value {
    match status {
        "failed" => {
            let detail = text.trim();
            json!({"error": if detail.is_empty() { "The tool failed.".to_owned() } else { detail.to_owned() }})
        }
        "cancelled" => json!({"error": "The agent stopped before this tool returned."}),
        _ => match name {
            "Run" => json!({"output": text}),
            "Read" => json!({"path": input["path"], "total_lines": text.lines().count()}),
            "Grep" => json!({"matches": text, "count": text.lines().count()}),
            "Glob" => json!({"files": text, "count": text.lines().count()}),
            _ => json!({"path": input["path"], "output": text}),
        },
    }
}

/// The answer to a `session/request_permission` for a hand held to `root`:
/// when `writes` is false, every writing tool (`edit`, `delete`, `move`)
/// is refused with the agent's own `reject` option; when it is true, only
/// a written path that resolves outside `root` is refused. Everything else
/// is allowed once, so the agent asks again for its next call. A command's
/// writes carry no paths — `devin --sandbox` bounds those. Carried over
/// from `crates/microcoder`'s `Answering::Contained` and reimplemented
/// here.
#[must_use]
pub fn boundary_answer(
    request: &PermissionRequest,
    root: &Path,
    writes: bool,
) -> (PermissionAnswer, Option<String>) {
    let pick = |option: Option<&str>| {
        option.map_or(PermissionAnswer::Cancelled, |option| {
            PermissionAnswer::Selected(option.to_owned())
        })
    };
    if request.tool_call.writes() {
        if !writes {
            return (
                pick(request.reject()),
                Some("this hand may not change files".to_owned()),
            );
        }
        if let Some(path) = request
            .tool_call
            .written_paths()
            .into_iter()
            .map(|path| resolved(root, &path))
            .find(|path| !path.starts_with(root))
        {
            return (
                pick(request.reject()),
                Some(format!(
                    "a write to {} would leave the worktree",
                    path.display()
                )),
            );
        }
    }
    (pick(request.allow_once()), None)
}

/// How long a cancelled ACP prompt gets to end as `cancelled`.
const ACP_CANCEL_GRACE: Duration = Duration::from_secs(10);
/// How long the agent gets to exit before its group is killed.
const ACP_STOP_GRACE: Duration = Duration::from_secs(5);

/// An ACP tool call in flight: its transcript row and its output so far.
struct AcpCall {
    index: usize,
    coder_name: String,
    coder_input: Value,
    output: String,
}

/// One `session/update` as a line for the hand's events file.
fn update_json(update: &Update) -> Value {
    match update {
        Update::AgentText(text) => json!({"kind": "agent_text", "text": text}),
        Update::Thought(text) => json!({"kind": "thought", "text": text}),
        Update::UserText(text) => json!({"kind": "user_text", "text": text}),
        Update::ToolCall {
            id,
            title,
            kind,
            status,
            raw_input,
            tool,
        } => {
            json!({"kind": "tool_call", "id": id, "title": title, "tool_kind": kind, "status": status, "input": raw_input, "tool": tool})
        }
        Update::ToolCallUpdate {
            id,
            status,
            title,
            text,
        } => {
            json!({"kind": "tool_call_update", "id": id, "status": status, "title": title, "text": text})
        }
        Update::Plan(entries) => json!({"kind": "plan", "entries": entries}),
        Update::Usage(usage) => {
            json!({"kind": "usage", "input_tokens": usage.input_tokens, "output_tokens": usage.output_tokens, "used": usage.used, "size": usage.size, "cost_usd": usage.cost_usd, "subagent": usage.subagent})
        }
        Update::CurrentMode(mode) => json!({"kind": "current_mode", "mode": mode}),
        Update::Title(title) => json!({"kind": "title", "title": title}),
        Update::Other => json!({"kind": "other"}),
    }
}

/// The handler that turns an ACP agent's stream into the same
/// `Entry::Assistant` and `Entry::Tool` items the Claude session makes,
/// and answers its permission asks against the worktree boundary.
struct AcpHandler<'a> {
    transcript: &'a mut Transcript,
    root: PathBuf,
    briefed: BTreeSet<String>,
    writes: bool,
    model: String,
    started: Instant,
    events: String,
    /// The reply text so far, and the part not yet shown as an entry.
    reply: String,
    text: String,
    calls: HashMap<String, AcpCall>,
    opened: BTreeSet<String>,
    misses: Vec<String>,
    input_tokens: u64,
    output_tokens: u64,
    cost_usd: Option<f64>,
    refused: Option<String>,
}

impl AcpHandler<'_> {
    /// Append one events-file line.
    fn record(&mut self, body: Value) {
        let t_ms = self.started.elapsed().as_millis() as u64;
        self.events
            .push_str(&json!({"t_ms": t_ms, "body": body}).to_string());
        self.events.push('\n');
    }

    /// Show the reply text gathered so far as an assistant entry.
    fn flush_text(&mut self) {
        if self.text.trim().is_empty() {
            self.text.clear();
            return;
        }
        let text = std::mem::take(&mut self.text);
        self.transcript.push(Entry::Assistant {
            text,
            model: Some(self.model.clone()),
            elapsed_ms: Some(self.started.elapsed().as_millis() as u64),
        });
    }

    /// Finish one tool call's transcript row.
    fn finish_call(&mut self, id: &str, status: &str) {
        let Some(call) = self.calls.remove(id) else {
            return;
        };
        self.flush_text();
        let output = acp_output(&call.coder_name, &call.coder_input, &call.output, status);
        self.transcript.set(
            call.index,
            Entry::Tool {
                name: call.coder_name,
                input: call.coder_input,
                output,
                running: false,
            },
        );
    }

    /// Flush what is still gathered and finish the calls still open as
    /// cancelled, once the prompt has ended.
    fn close(&mut self) {
        self.flush_text();
        let pending: Vec<String> = self.calls.keys().cloned().collect();
        for id in pending {
            self.finish_call(&id, "cancelled");
        }
    }
}

impl acp_client::Handler for AcpHandler<'_> {
    fn update(&mut self, update: Update) {
        self.record(update_json(&update));
        match update {
            Update::AgentText(piece) => {
                self.reply.push_str(&piece);
                self.text.push_str(&piece);
            }
            Update::ToolCall {
                id,
                title,
                kind,
                status,
                raw_input,
                tool: _,
            } => {
                self.flush_text();
                if kind == "read"
                    && let Some(Some(path)) = inside(&self.root, &raw_input)
                    && !self.briefed.contains(&path)
                    && self.opened.insert(path.clone())
                {
                    self.misses.push(path);
                }
                let (name, input) = acp_call(&kind, &title, &raw_input, &self.root);
                let running = !matches!(status.as_str(), "completed" | "failed" | "cancelled");
                let index = self.transcript.push(Entry::Tool {
                    name: name.clone(),
                    input: input.clone(),
                    output: Value::Null,
                    running,
                });
                self.calls.insert(
                    id.clone(),
                    AcpCall {
                        index,
                        coder_name: name,
                        coder_input: input,
                        output: String::new(),
                    },
                );
                if !running {
                    self.finish_call(&id, &status);
                }
            }
            Update::ToolCallUpdate {
                id, status, text, ..
            } => {
                if let Some(call) = self.calls.get_mut(&id)
                    && let Some(text) = text
                {
                    // The agent sends the tool's whole output so far in
                    // each update; the latest replaces the earlier.
                    call.output = text;
                }
                if let Some(status) = status
                    && matches!(status.as_str(), "completed" | "failed" | "cancelled")
                {
                    self.finish_call(&id, &status);
                }
            }
            Update::Plan(entries) => {
                self.flush_text();
                let plan = entries
                    .iter()
                    .map(|entry| {
                        entry
                            .status
                            .as_deref()
                            .map_or(entry.content.clone(), |status| {
                                format!("{} ({})", entry.content, status)
                            })
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !plan.trim().is_empty() {
                    self.transcript.push(Entry::Assistant {
                        text: plan,
                        model: Some(self.model.clone()),
                        elapsed_ms: Some(self.started.elapsed().as_millis() as u64),
                    });
                }
            }
            Update::Usage(usage) if !usage.subagent => {
                self.input_tokens = self
                    .input_tokens
                    .saturating_add(usage.input_tokens.unwrap_or_default());
                self.output_tokens = self
                    .output_tokens
                    .saturating_add(usage.output_tokens.unwrap_or_default());
                if usage.cost_usd.is_some() {
                    self.cost_usd = usage.cost_usd;
                }
            }
            _ => {}
        }
    }

    fn notification(&mut self, method: &str, params: &Value) {
        self.record(json!({"kind": "notification", "method": method, "params": params}));
    }

    fn permission(&mut self, request: &PermissionRequest) -> PermissionAnswer {
        let (answer, why) = boundary_answer(request, &self.root, self.writes);
        self.record(json!({
            "kind": "permission",
            "tool_kind": request.tool_call.kind,
            "title": request.tool_call.title,
            "refused": why,
        }));
        if let Some(why) = why {
            if self.refused.is_none() {
                let tool = request
                    .tool_call
                    .title
                    .clone()
                    .or_else(|| request.tool_call.kind.clone())
                    .unwrap_or_else(|| "a tool".to_owned());
                self.refused = Some(format!("{tool}: {why}"));
            }
        }
        answer
    }
}

/// Runs one ACP session (`devin acp`) to the end in `worktree`, drawing
/// its work as it goes: the same cards, entries and `Work` the Claude
/// driver produces. The session runs `accept-edits` under `--sandbox`,
/// and the boundary answerer holds its writes to the worktree.
pub async fn run_acp(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let started = Instant::now();
    let mut work = Work::default();
    let tools_text = session.tools_text();
    let card = json!({
        "kind": "agent",
        "title": session.title,
        "detail": session.detail,
        "rows": [
            {"label": "engine", "text": "devin"},
            {"label": "model", "text": session.model},
            {"label": "tools", "text": tools_text},
            {"label": "checks", "text": session.checks.iter().map(|check| check.id.clone()).collect::<Vec<_>>().join(", ")},
            {"label": "briefing", "text": format!("{} files, {} characters in the prompt", session.briefed.len(), session.system.chars().count() + session.prompt.chars().count())},
        ],
    });
    let card_index = transcript.card(card.clone());
    let Engine::Acp(acp) = session.engine else {
        unreachable!("run_acp takes an ACP session")
    };

    let Some(program) = acp.program.clone() else {
        let why =
            "Devin did not start: no devin binary in DEVIN_BIN, PATH, or ~/.local/bin.".to_owned();
        transcript.finish(card_index, card, json!({"error": why.clone()}));
        work.error = Some(why);
        return work;
    };
    let opening = acp_client::session::Opening {
        spec: acp_client::process::Spec {
            program,
            arguments: acp.arguments,
            cwd: root.clone(),
            environment: acp.environment,
        },
        resume: None,
        meta: acp.meta,
        mode: acp.mode,
        authenticate: None,
    };
    let cancel = transcript.cancel.clone();
    let cancelled = move || cancel.load(std::sync::atomic::Ordering::Relaxed);
    let mut acp_session = match acp_client::Session::open(&opening, &cancelled).await {
        Ok(acp_session) => acp_session,
        Err(failure) => {
            let why = format!("Devin did not start: {failure}");
            transcript.finish(card_index, card, json!({"error": why.clone()}));
            work.error = Some(why);
            return work;
        }
    };
    let reported = acp_session.opened.model().map(str::to_owned);
    if !acp_client::devin::admits(&session.model, reported.as_deref()) {
        let why = format!(
            "Devin reported a different model ({}) than the hand asked for ({}).",
            reported.as_deref().unwrap_or("none"),
            session.model
        );
        transcript.finish(card_index, card, json!({"error": why.clone()}));
        work.error = Some(why);
        acp_session.close(ACP_STOP_GRACE).await;
        return work;
    }
    transcript.finish(card_index, card, json!({"ok": true}));

    // An ACP session has no system prompt channel (`session/new` carries
    // only cwd, mcpServers and _meta), so the hand's system text leads its
    // prompt.
    let prompt = if session.system.trim().is_empty() {
        session.prompt.clone()
    } else {
        format!("{}\n\n{}", session.system.trim_end(), session.prompt)
    };
    // The prompt's bound is the hand's timeout: the cancel flag stops it
    // as `session/cancel`, the same stop the transcript's own cancel sends.
    let cancel_flag = {
        let cancel = transcript.cancel.clone();
        let timeout = session.timeout;
        move || cancel.load(std::sync::atomic::Ordering::Relaxed) || started.elapsed() > timeout
    };
    let timed_out = |started: Instant| started.elapsed() > session.timeout;
    let mut handler = AcpHandler {
        transcript,
        root,
        briefed: session.briefed,
        writes: acp.writes,
        model: reported.unwrap_or_else(|| session.model.clone()),
        started,
        events: String::new(),
        reply: String::new(),
        text: String::new(),
        calls: HashMap::new(),
        opened: BTreeSet::new(),
        misses: Vec::new(),
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: None,
        refused: None,
    };
    let reported_model = handler.model.clone();
    handler.record(json!({
        "kind": "prompt",
        "session": acp_session.id(),
        "model": reported_model,
        "prompt": prompt,
    }));
    let prompted = acp_session
        .prompt(
            &prompt,
            session.timeout,
            &cancel_flag,
            ACP_CANCEL_GRACE,
            &mut handler,
        )
        .await;
    handler.close();
    work.misses = std::mem::take(&mut handler.misses);
    work.input_tokens = (handler.input_tokens > 0).then_some(handler.input_tokens);
    work.output_tokens = (handler.output_tokens > 0).then_some(handler.output_tokens);
    work.cost_usd = handler.cost_usd;
    work.reply = (!handler.reply.trim().is_empty()).then(|| std::mem::take(&mut handler.reply));
    let events = std::mem::take(&mut handler.events);
    let refused = handler.refused.take();
    let group_clear = acp_session.close(ACP_STOP_GRACE).await;
    let _ = std::fs::write(folder.join(&session.events_file), events);
    if let Some(refused) = refused {
        transcript.notice(format!(
            "The agent asked for a tool the run refused: {refused}"
        ));
    }
    if !group_clear {
        transcript.notice("The agent did not stop cleanly.");
    }
    work.wall_ms = started.elapsed().as_millis() as u64;
    match prompted {
        Ok(reply) => {
            work.cache_read = reply.usage.and_then(|usage| usage.cached_read_tokens);
            if work.input_tokens.is_none()
                && work.output_tokens.is_none()
                && let Some(usage) = reply.usage
            {
                work.input_tokens = usage.input_tokens;
                work.output_tokens = usage.output_tokens;
            }
            match reply.stop_reason {
                StopReason::Cancelled => {
                    work.error = Some(if transcript.cancelled() {
                        "The run was stopped.".to_owned()
                    } else if timed_out(started) {
                        "The agent ran out of its time.".to_owned()
                    } else {
                        "The agent ended the turn as cancelled.".to_owned()
                    });
                }
                StopReason::EndTurn => {}
                other => {
                    work.error = Some(format!(
                        "The agent ended its turn early ({}).",
                        other.as_str()
                    ));
                }
            }
        }
        Err(error) => {
            work.error = Some(if matches!(error, acp_client::ClientError::Cancelled) {
                if transcript.cancelled() {
                    "The run was stopped.".to_owned()
                } else {
                    "The agent ran out of its time.".to_owned()
                }
            } else {
                format!("Devin's turn failed: {error}")
            });
        }
    }
    work
}

/// [`final_checks`] with each check's output, so a hand-back can carry the
/// real errors. A check in `passed` already passed on this same working copy
/// (the Tester ran it and cannot edit), so it isn't built again.
pub async fn final_check_outputs(
    base: &Base,
    briefing: &Briefing,
    passed: &[String],
    transcript: &mut Transcript,
) -> Vec<(String, bool, String)> {
    let mut verdicts = Vec::new();
    for check in briefing.checks.iter().filter(|c| harness_runs(c)) {
        if transcript.cancelled() {
            break;
        }
        if passed.contains(&check.id) {
            verdicts.push((check.id.clone(), true, "passed in the Tester's run".into()));
            continue;
        }
        let input = json!({"command": check.argv.join(" "), "check": check.id});
        let index = transcript.push(Entry::Tool {
            name: "Run".into(),
            input: input.clone(),
            output: Value::Null,
            running: true,
        });
        let (text, ok) = run_check_against(
            &briefing.checks,
            &base.worktree,
            Some(&Baseline::of(base)),
            &check.id,
            None,
        )
        .await;
        let output = coder_output("Run", &Value::Null, &input, &text, None, !ok);
        transcript.set(
            index,
            Entry::Tool {
                name: "Run".into(),
                input,
                output,
                running: false,
            },
        );
        verdicts.push((check.id.clone(), ok, text));
    }
    verdicts
}
