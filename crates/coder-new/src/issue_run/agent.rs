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
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use acp_client::{PermissionRequest, StopReason, Update};
use claude_agent_sdk::{
    PermissionMode, QueryOptions, SdkMcpServer, SdkMessage, SystemPromptConfig, ToolResult,
    ToolsConfig,
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
    /// What the Stop gate (#11487) did, when it was on: stops, blocks, and
    /// why each allowed stop went through.
    pub stop_gate: Option<Value>,
}

fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The worktree-relative path a tool input names, read lexically (the
/// Coder SDK's one copy, [`coder_sdk::worktree`]).
pub use coder_sdk::worktree::{relative as inside, resolved};

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
    /// A Coder-loop session on the OpenAgents gateway (#11518): Coder's
    /// own Run and file tools under the same floors and fence as any Coder
    /// chat, held to the worktree.
    Coder {
        /// Whether the hand may change files in the worktree; when not,
        /// the whole disk is read-only for it.
        writes: bool,
        /// The sign-in the gateway turn uses.
        account: Option<openagents_login::Saved>,
    },
    /// An `opencode run` session under Coder's OpenCode plugin (#11533),
    /// on the session's OpenCode model: the floors held, the fence and the
    /// worktree guard over every tool, its shell included.
    OpenCode {
        /// Whether the hand may change files in the worktree; when not,
        /// the whole disk is read-only for it.
        writes: bool,
        /// OpenCode's program; `None` when it isn't installed, which
        /// surfaces as the session's start error.
        program: Option<PathBuf>,
    },
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
        && let Some(home) = user_home::home_os()
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
    /// When set, `run_check` also offers the checks the change calls for at
    /// the moment of each call: every Rust package the working copy touches
    /// since the base, new packages included (#11505). The checks above stay
    /// offered too. Only the Claude engine consumes it.
    pub follow: Option<Follow>,
}

/// How a session's `run_check` follows the change as it grows.
#[derive(Clone, Debug)]
pub struct Follow {
    /// The commit the change is measured from.
    pub base: String,
    /// Only compile checks (`check:`, `wasm:`): the Coder compiles as it
    /// works and leaves the tests to the harness and the Tester.
    pub compile_only: bool,
}

/// A compile check: `check:<package>`, or `wasm:<package>` for a browser
/// build's wasm32 half.
#[must_use]
pub fn is_compile(check: &Check) -> bool {
    check.id.starts_with("check:") || check.id.starts_with("wasm:")
}

/// The checks `run_check` offers right now: `checks`, plus, when the
/// session follows the change, the checks the change in `root` since the
/// base calls for, read again at every call so a crate the session created
/// a minute ago is already checkable.
pub async fn checks_now(checks: &[Check], root: &Path, follow: Option<&Follow>) -> Vec<Check> {
    let mut list = checks.to_vec();
    if let Some(follow) = follow {
        let changed = super::setup::change_checks(root, &follow.base).await;
        with_changed(&mut list, changed, follow.compile_only);
    }
    list
}

/// Adds the `changed` checks `list` lacks (only compile ones when
/// `compile_only`), keeping `list`'s order first.
pub fn with_changed(list: &mut Vec<Check>, changed: Vec<Check>, compile_only: bool) {
    for check in changed {
        if (!compile_only || is_compile(&check)) && !list.iter().any(|have| have.id == check.id) {
            list.push(check);
        }
    }
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
    // Never the run's own build folder: see [`target_dir`].
    if let Some(target) = target_dir(
        std::env::var_os("CARGO_TARGET_DIR"),
        user_home::home_os(),
        true,
    ) {
        command.env("CARGO_TARGET_DIR", target);
    }
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
    if let Some(target) = target_dir(
        std::env::var_os("CARGO_TARGET_DIR"),
        user_home::home_os(),
        false,
    ) {
        command.env("CARGO_TARGET_DIR", target);
    }
}

/// The build folder a check runs with, from the person's `CARGO_TARGET_DIR`
/// (`user`) and `HOME`; `None` leaves the person's own setting alone.
///
/// The baseline checkout builds in a sibling folder (`<target>-baseline`),
/// never the run's. Cargo names a workspace crate's outputs by its path
/// inside the workspace, not by the checkout's own path, and calls them
/// fresh when they are newer than the sources. So a baseline build of
/// `route-contract` landing after the Coder's edit to it was taken as the
/// working copy's own, and `coder-sdk`'s tests then failed to find the
/// function the edit added (#11505).
#[must_use]
pub fn target_dir(
    user: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    baseline: bool,
) -> Option<PathBuf> {
    let shared = match user {
        Some(_) if !baseline => return None,
        Some(user) => PathBuf::from(user),
        None => Path::new(&home?)
            .join(".openagents")
            .join(crate::home::DIR)
            .join("target"),
    };
    if !baseline {
        return Some(shared);
    }
    let mut name = shared.file_name()?.to_os_string();
    name.push("-baseline");
    Some(shared.with_file_name(name))
}

/// Free space below which a finished run sweeps the shared build folder.
const SWEEP_BELOW_BYTES: u64 = 60 * 1024 * 1024 * 1024;
/// Build outputs untouched this long are swept.
const SWEEP_AGE: std::time::Duration = std::time::Duration::from_secs(2 * 60 * 60);

/// Sweeps stale outputs from the shared build folder (and the baseline's
/// beside it) when the disk is low. Every run builds its workspace crates
/// again (a fresh checkout's sources are newer than any earlier output), so
/// each run leaves ~20 GB in the folder that no later run reuses; three
/// runs took the Mac from 45 GB free to 6 GB. Outputs older than
/// [`SWEEP_AGE`] go; cargo rebuilds any it still needs. A build folder the
/// person chose (`CARGO_TARGET_DIR`) is never touched.
pub async fn sweep_shared_target() {
    if std::env::var_os("CARGO_TARGET_DIR").is_some() {
        return;
    }
    for baseline in [false, true] {
        let Some(target) = target_dir(None, user_home::home_os(), baseline) else {
            return;
        };
        if !target.is_dir()
            || free_bytes(&target)
                .await
                .is_none_or(|free| free >= SWEEP_BELOW_BYTES)
        {
            continue;
        }
        let _ = tokio::task::spawn_blocking(move || sweep(&target, SWEEP_AGE)).await;
    }
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
        let (detail, engine) = match options.engine {
            crate::solver::Engine::Coder => (
                "Coder loop on the OpenAgents gateway",
                Engine::Coder {
                    writes: true,
                    account: options.account.clone(),
                },
            ),
            crate::solver::Engine::OpenCode => (
                "OpenCode session under Coder's plugin",
                Engine::OpenCode {
                    writes: true,
                    program: crate::opencode_chat::binary(),
                },
            ),
            crate::solver::Engine::Claude | crate::solver::Engine::Devin => (
                "Claude Agent SDK session on this computer's Claude Code login",
                Engine::Claude {
                    tools: TOOLS.iter().map(|t| (*t).to_owned()).collect(),
                    server: None,
                },
            ),
        };
        Self {
            title: "Briefed agent".into(),
            detail: detail.into(),
            model: options.model.clone(),
            timeout: options.timeout,
            engine,
            checks: briefing.checks.clone(),
            system: briefing.system.clone(),
            prompt: briefing.prompt.clone(),
            briefed: briefing.files.iter().cloned().collect(),
            events_file: "agent-events.jsonl".into(),
            baseline: None,
            follow: None,
        }
    }

    /// What the card's `tools` row says.
    #[must_use]
    pub fn tools_text(&self) -> String {
        match &self.engine {
            Engine::Claude { tools, server } => {
                let mut names = tools.clone();
                if !self.checks.is_empty() || self.follow.is_some() {
                    names.push("run_check".into());
                }
                if let Some(server) = server {
                    names.extend(server.allowed_tool_names());
                }
                names.join(", ")
            }
            Engine::Acp(_) => "the agent's own tools".into(),
            Engine::Coder { writes: true, .. } => {
                "Run, Read, Edit, Write, Grep, Glob, held to the worktree".into()
            }
            Engine::Coder { writes: false, .. } => "Run, Read, Grep, Glob, read-only".into(),
            Engine::OpenCode { writes: true, .. } => {
                "OpenCode's own tools, held to the worktree".into()
            }
            Engine::OpenCode { writes: false, .. } => "OpenCode's own tools, read-only".into(),
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
    run_gated(
        Session::briefed(options, briefing),
        &base.worktree,
        folder,
        transcript,
        true,
    )
    .await
}

/// The issue-run session's Stop gate (#11487): a stop is checked against
/// `finals` (the checks the harness runs after the agent) unless the agent
/// says it is blocked or asks a question. Decisions go to the run folder's
/// `stop_gate.jsonl`.
pub(crate) fn stop_gate(
    finals: Arc<Vec<Check>>,
    root: PathBuf,
    baseline: Option<Baseline>,
    last_text: Arc<Mutex<String>>,
    classify: briefed_agent::stop_gate::Classify,
    folder: &Path,
) -> briefed_agent::stop_gate::StopGate {
    use briefed_agent::stop_gate::{Check as GateCheck, StopGate, Tree};
    let check: GateCheck = {
        let root = root.clone();
        Arc::new(move || {
            let (finals, root, baseline) = (finals.clone(), root.clone(), baseline.clone());
            Box::pin(async move { final_verdict(&finals, &root, baseline.as_ref()).await })
        })
    };
    let tree: Tree = Arc::new(move || {
        let root = root.clone();
        Box::pin(async move { candidate_tree(&root).await })
    });
    let mut gate = StopGate::new(
        Arc::default(),
        last_text,
        classify,
        check,
        tree,
        Some(folder.join("stop_gate.jsonl")),
    );
    // The run folder is named for the run, the key `outcome.jsonl` joins on.
    gate.run_id = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    gate
}

/// Each of `finals` run against `root`: `{"status": "pass"}`, or `"fail"`
/// with each failed check's id and output.
pub async fn final_verdict(finals: &[Check], root: &Path, baseline: Option<&Baseline>) -> Value {
    let mut failed = Vec::new();
    for check in finals {
        let (text, ok) = run_check_against(finals, root, baseline, &check.id, None).await;
        if !ok {
            failed.push(json!({"check": check.id, "output": text}));
        }
    }
    if failed.is_empty() {
        json!({"status": "pass"})
    } else {
        json!({"status": "fail", "failed": failed})
    }
}

/// The working copy's content as a git tree id, staged into a scratch
/// index so the real one is untouched; `None` when git can't say.
pub(crate) async fn candidate_tree(root: &Path) -> Option<String> {
    async fn git(root: &Path, args: &[&str], index: Option<&Path>) -> Option<String> {
        let mut command = tokio::process::Command::new("git");
        command.arg("-C").arg(root).args(args);
        if let Some(index) = index {
            command.env("GIT_INDEX_FILE", index);
        }
        let out = command.output().await.ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    }
    let index = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        None,
    )
    .await?;
    let scratch = tempfile::NamedTempFile::new().ok()?;
    std::fs::copy(&index, scratch.path()).ok()?;
    git(root, &["add", "-A"], Some(scratch.path())).await?;
    git(root, &["write-tree"], Some(scratch.path())).await
}

/// The `run_check` tool server over `checks`, and the verdicts it records.
/// With `follow`, each call also offers the checks the change calls for at
/// that moment ([`checks_now`]).
fn checks_server(
    checks: Arc<Vec<Check>>,
    root: PathBuf,
    baseline: Option<Baseline>,
    follow: Option<Follow>,
) -> (SdkMcpServer, Arc<Mutex<Vec<(String, bool)>>>) {
    let verdicts: Arc<Mutex<Vec<(String, bool)>>> = Arc::default();
    let listed = checks
        .iter()
        .map(|c| format!("{} ({})", c.id, c.what))
        .collect::<Vec<_>>()
        .join("; ");
    let (description, check_field) = match &follow {
        None => (
            format!("Run one of the briefing's checks against your working copy. Checks: {listed}"),
            json!({"type": "string", "enum": checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>()}),
        ),
        Some(follow) => (
            format!(
                "Run a check against your working copy. Checks now: {listed}. Every Rust package your change touches also has `check:<package>`{}, a new package included, as soon as a file in it changes; an unknown name answers with the list.",
                if follow.compile_only {
                    ""
                } else {
                    " and `test:<package>`"
                }
            ),
            json!({"type": "string", "description": "the check's id, e.g. `check:<package>`"}),
        ),
    };
    let server = {
        let verdicts = verdicts.clone();
        SdkMcpServer::new("checks", "1.0.0").tool(
            "run_check",
            description,
            json!({
                "type": "object",
                "properties": {
                    "check": check_field,
                    "filter": {"type": "string", "description": "test-name filter words (test checks only)"}
                },
                "required": ["check"]
            }),
            move |args| {
                let (checks, root, verdicts) = (checks.clone(), root.clone(), verdicts.clone());
                let (baseline, follow) = (baseline.clone(), follow.clone());
                async move {
                    let id = args["check"].as_str().unwrap_or_default().to_owned();
                    let checks = checks_now(&checks, &root, follow.as_ref()).await;
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
    run_gated(session, worktree, folder, transcript, false).await
}

/// [`run`], with the Stop gate (#11487) when `gated` and the Token savers
/// plugin is on. Only the issue-run's own agent is gated: the Solver's
/// hands already hand their work to a Tester.
pub async fn run_gated(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
    gated: bool,
) -> Work {
    if matches!(session.engine, Engine::Acp(_)) {
        return run_acp(session, worktree, folder, transcript).await;
    }
    if matches!(session.engine, Engine::Coder { .. }) {
        return run_coder(session, worktree, folder, transcript, gated).await;
    }
    if matches!(session.engine, Engine::OpenCode { .. }) {
        return run_opencode(session, worktree, folder, transcript).await;
    }
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let checks = Arc::new(session.checks.clone());
    let tools_text = session.tools_text();
    let Engine::Claude { tools, server } = session.engine else {
        unreachable!("the ACP, Coder and OpenCode arms returned");
    };
    let mut allowed: Vec<String> = tools.clone();
    let mut query_options = QueryOptions::new()
        .cwd(&root)
        .model(session.model.clone())
        .permission_mode(PermissionMode::Default)
        .no_session_persistence();
    let mut verdicts = None;
    if !checks.is_empty() || session.follow.is_some() {
        let (check_server, seen) = checks_server(
            checks.clone(),
            root.clone(),
            session.baseline.clone(),
            session.follow.clone(),
        );
        allowed.extend(check_server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(check_server);
        verdicts = Some(seen);
    }
    if let Some(server) = server {
        allowed.extend(server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(server);
    }
    // The session's hooks, on the Coder SDK (#11509): the shared-checkout
    // fence, the floors, the worktree guard over the offered tools, the
    // squeezer and the Stop gate (Token savers levers, #11486, #11487), the
    // plugins' hooks and the monitor, composed by one runner.
    let last_text: Arc<Mutex<String>> = Arc::default();
    let finals: Vec<Check> = checks.iter().filter(|c| harness_runs(c)).cloned().collect();
    let gate = (gated && !finals.is_empty() && crate::token_savers::enabled()).then(|| {
        Arc::new(stop_gate(
            Arc::new(finals),
            root.clone(),
            session.baseline.clone(),
            last_text.clone(),
            briefed_agent::stop_gate::decisions(),
            folder,
        ))
    });
    let backend = crate::sdk_hooks::task_claude(
        &root,
        crate::sdk_hooks::Task {
            guard: coder_sdk::worktree::Guard::new(&root).offering(allowed.clone()),
            stop_gate: gate.clone().map(|gate| (gate, session.timeout)),
        },
    );
    query_options = backend.install(query_options, coder_sdk::claude::PreTool::Permission);
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
    let handler = backend.permission_handler();

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
                                text.clone_into(&mut lock(&last_text));
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
    work.stop_gate = gate.map(|gate| lock(&gate.tally).clone());
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
            ..
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
    /// The session's hooks, which answer every ask (#11509).
    hooks: coder_sdk::Hooks,
    /// What the hooks refused, each as the agent should read it.
    refusals: Arc<Mutex<Vec<String>>>,
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

    /// Every ask is answered by the session's hooks (#11509): the fence,
    /// the floors, the worktree guard and the rest, as on Claude Code.
    fn permission_later(
        &mut self,
        request: &PermissionRequest,
    ) -> Option<acp_client::PendingAnswer> {
        let call = coder_sdk::acp::call(request, None, &self.root);
        self.record(json!({
            "kind": "permission",
            "tool_kind": request.tool_call.kind,
            "title": request.tool_call.title,
        }));
        let (hooks, refusals) = (self.hooks.clone(), self.refusals.clone());
        let request = request.clone();
        Some(Box::pin(async move {
            let applied = hooks.fire(call).await;
            if let Some(reason) = coder_sdk::acp::refusal(&applied) {
                let tool = request
                    .tool_call
                    .title
                    .clone()
                    .or_else(|| request.tool_call.kind.clone())
                    .unwrap_or_else(|| "a tool".to_owned());
                lock(&refusals).push(format!("{tool}: {reason}"));
            }
            coder_sdk::acp::answer(&applied.permission, &request)
        }))
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
    // The session's hooks (#11509): the same as any ACP agent's in Coder
    // (the fence, the floors, the plugins' hooks, the monitor), the
    // worktree guard over what the hand may change, and Devin's own hooks
    // running Coder's bridge before every tool.
    let mut guard = coder_sdk::worktree::Guard::new(&root);
    if !acp.writes {
        guard = guard.read_only();
    }
    let (hooks, _refused) = crate::sdk_hooks::task_acp(
        &root,
        crate::sdk_hooks::Task {
            guard,
            stop_gate: None,
        },
        "devin-cli",
        "Devin",
        &transcript.cancel,
    );
    let (mut arguments, mut environment) = (acp.arguments, acp.environment);
    let agent = crate::bundled_runtime::AcpAgent {
        id: "devin-cli".into(),
        name: "Devin CLI".into(),
        program: program.clone(),
        transport: crate::bundled_runtime::AgentTransport::default(),
        arguments: Vec::new(),
        mode: None,
        enabled: true,
    };
    let _gate =
        match crate::bundled_runtime::acp_gate(&agent, &hooks, &mut arguments, &mut environment) {
            Ok(gate) => gate,
            Err(why) => {
                transcript.finish(card_index, card, json!({"error": why.clone()}));
                work.error = Some(why);
                return work;
            }
        };
    let opening = acp_client::session::Opening {
        spec: acp_client::process::Spec {
            program,
            arguments,
            cwd: root.clone(),
            environment,
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
        hooks,
        refusals: Arc::default(),
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
    let refused = lock(&handler.refusals).first().cloned();
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

/// The longest prompt a Coder-loop session is given, in bytes; the rest of
/// a longer one (a very large diff) is cut with a note.
const CODER_PROMPT_MAX: usize = 60 * 1024;

/// The settings of a Coder-loop session in `root` (#11518): Coder's Run
/// and file tools and nothing else, under the floors and the fence. With
/// `writes`, everything outside `root` is read-only for it; without, the
/// whole disk is, and only reading commands run.
#[must_use]
pub fn coder_settings(root: &Path, writes: bool) -> crate::plugin_tools::ExecutionSettings {
    let mut execution = crate::plugins::Plugins::default().execution_settings(root.to_path_buf());
    execution.microcoder = false;
    execution.cli = false;
    execution.acp = false;
    execution.claude = false;
    execution.solver = false;
    execution.comms = false;
    execution.swarm = false;
    execution.channel = None;
    execution.boat.enabled = false;
    execution.gce.enabled = false;
    execution.brainstorm = None;
    execution.memory = None;
    execution.routing = None;
    execution.fleet = None;
    execution.agents = Vec::new();
    execution.connections = None;
    execution.shell = true;
    execution.harness = Some(crate::harness::Run::choose(
        None,
        crate::harness::DecidedBy::Fallback,
        0,
    ));
    execution.boundary = Some(task_fence(root, writes));
    execution
}

/// A task session's fence: the whole disk as the shared side, so a write
/// lands in the session's own folder or is refused. A read-only hand's own
/// folder is one that never exists, so nothing it writes lands anywhere.
#[must_use]
pub fn task_fence(root: &Path, writes: bool) -> crate::checkout_guard::Boundary {
    crate::checkout_guard::Boundary {
        worktree: if writes {
            root.to_path_buf()
        } else {
            root.join(".coder-read-only-hand")
        },
        shared: PathBuf::from("/"),
        branch: String::new(),
    }
}

/// One session on OpenCode (#11533): the hand's system text and prompt as
/// the opening message of a new OpenCode session, its tool calls drawn as
/// they run, bounded by the hand's timeout and the run's stop. Coder's
/// OpenCode plugin asks the session's hooks before every tool
/// ([`crate::sdk_hooks::task_opencode`]).
pub async fn run_opencode(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    use std::sync::atomic::{AtomicBool, Ordering};
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let started = Instant::now();
    let mut work = Work::default();
    let card = json!({
        "kind": "agent",
        "title": session.title,
        "detail": session.detail,
        "rows": [
            {"label": "engine", "text": "OpenCode"},
            {"label": "model", "text": session.model},
            {"label": "tools", "text": session.tools_text()},
            {"label": "checks", "text": session.checks.iter().map(|check| check.id.clone()).collect::<Vec<_>>().join(", ")},
            {"label": "briefing", "text": format!("{} files, {} characters in the prompt", session.briefed.len(), session.system.chars().count() + session.prompt.chars().count())},
        ],
    });
    let card_index = transcript.card(card.clone());
    let Engine::OpenCode { writes, program } = session.engine else {
        unreachable!("run_opencode takes an OpenCode session")
    };
    let Some(program) = program else {
        let why = "OpenCode is not installed on this computer.".to_owned();
        transcript.finish(card_index, card, json!({"error": why.clone()}));
        work.error = Some(why);
        return work;
    };
    transcript.finish(card_index, card, json!({"ok": true}));
    let mut guard = coder_sdk::worktree::Guard::new(&root);
    if !writes {
        guard = guard.read_only();
    }
    let registry = crate::sdk_hooks::task_opencode(&root, guard, task_fence(&root, writes));
    // An OpenCode session has a system prompt only through its agents'
    // configuration, so the hand's system text leads its prompt.
    let prompt = if session.system.trim().is_empty() {
        session.prompt.clone()
    } else {
        format!("{}\n\n{}", session.system.trim_end(), session.prompt)
    };
    let turn = crate::opencode_chat::Turn {
        messages: vec![openrouter::Message {
            role: "user".into(),
            content: prompt.clone(),
        }],
        cwd: root.clone(),
        session: None,
        last_model: None,
        briefing: None,
        model: session.model.clone(),
        tools: false,
    };
    let mut events = String::new();
    events
        .push_str(&json!({"type": "prompt", "model": session.model, "prompt": prompt}).to_string());
    events.push('\n');
    // The hand's stop: the run's own, or its time running out.
    let stop = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (stop, run_stop, timeout) = (stop.clone(), transcript.cancel.clone(), session.timeout);
        async move {
            while !run_stop.load(Ordering::Relaxed) && started.elapsed() <= timeout {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            stop.store(true, Ordering::Relaxed);
            std::future::pending::<()>().await;
        }
    };
    let briefed = session.briefed;
    let mut opened: BTreeSet<String> = BTreeSet::new();
    let mut misses = Vec::new();
    let mut on_session = |_: &str| {};
    let result = {
        let mut on_event = |event: crate::bundled_runtime::RuntimeEvent| {
            let crate::bundled_runtime::RuntimeEvent::Tool {
                name,
                input,
                output,
                running: false,
            } = event
            else {
                return;
            };
            events.push_str(&json!({"type": "tool", "t_ms": started.elapsed().as_millis() as u64, "name": name, "input": input, "output": output}).to_string());
            events.push('\n');
            if name == "Read"
                && let Some(Some(path)) = inside(&root, &input)
                && !briefed.contains(&path)
                && opened.insert(path.clone())
            {
                misses.push(path);
            }
            transcript.push(Entry::Tool {
                name,
                input,
                output,
                running: false,
            });
        };
        let turn = Box::pin(crate::opencode_chat::run_with(
            &program,
            turn,
            registry,
            &stop,
            &mut on_event,
            &mut on_session,
        ));
        tokio::select! {
            result = turn => result,
            () = watcher => unreachable!("the watcher never finishes"),
        }
    };
    let t_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(reply) => {
            let text = reply.text.trim().to_owned();
            if !text.is_empty() {
                transcript.push(Entry::Assistant {
                    text: text.clone(),
                    model: Some(reply.model.clone()).filter(|model| !model.is_empty()),
                    elapsed_ms: Some(t_ms),
                });
                work.reply = Some(text);
            }
            events.push_str(&json!({"type": "result", "t_ms": t_ms, "model": reply.model, "tokens": reply.usage.total_tokens}).to_string());
            events.push('\n');
            work.input_tokens =
                (reply.usage.prompt_tokens > 0).then_some(reply.usage.prompt_tokens);
            work.output_tokens =
                (reply.usage.completion_tokens > 0).then_some(reply.usage.completion_tokens);
        }
        Err(why) => {
            work.error = Some(if transcript.cancelled() {
                "The run was stopped.".to_owned()
            } else if started.elapsed() > session.timeout {
                "The agent ran out of its time.".to_owned()
            } else {
                why
            });
        }
    }
    let _ = std::fs::write(folder.join(&session.events_file), events);
    work.wall_ms = t_ms;
    work.misses = misses;
    work
}

/// One session on the Coder loop through the OpenAgents gateway (#11518):
/// the hand's system text and prompt as one turn, its tool calls drawn as
/// they run, bounded by the hand's timeout and the run's stop.
pub async fn run_coder(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
    gated: bool,
) -> Work {
    use std::sync::atomic::{AtomicBool, Ordering};
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let started = Instant::now();
    let mut work = Work::default();
    let card = json!({
        "kind": "agent",
        "title": session.title,
        "detail": session.detail,
        "rows": [
            {"label": "engine", "text": "Coder loop on the OpenAgents gateway"},
            {"label": "model", "text": session.model},
            {"label": "tools", "text": session.tools_text()},
            {"label": "checks", "text": session.checks.iter().map(|check| check.id.clone()).collect::<Vec<_>>().join(", ")},
            {"label": "briefing", "text": format!("{} files, {} characters in the prompt", session.briefed.len(), session.system.chars().count() + session.prompt.chars().count())},
        ],
    });
    let card_index = transcript.card(card.clone());
    let Engine::Coder { writes, account } = session.engine else {
        unreachable!("run_coder takes a Coder session")
    };
    let Some(account) = account else {
        let why = "The Coder loop needs your OpenAgents sign-in: run coder login.".to_owned();
        transcript.finish(card_index, card, json!({"error": why.clone()}));
        work.error = Some(why);
        return work;
    };
    let provider = match crate::provider::Provider::gateway(&account) {
        Ok(provider) => provider,
        Err(why) => {
            transcript.finish(card_index, card, json!({"error": why.clone()}));
            work.error = Some(why);
            return work;
        }
    };
    transcript.finish(card_index, card, json!({"ok": true}));
    let mut execution = coder_settings(&root, writes);
    execution
        .redaction_keys
        .push(model_access::ApiKey::new(account.token()));
    let mut prompt = session.prompt.clone();
    if prompt.len() > CODER_PROMPT_MAX {
        let mut cut = CODER_PROMPT_MAX;
        while !prompt.is_char_boundary(cut) {
            cut -= 1;
        }
        prompt.truncate(cut);
        prompt.push_str("\n\n[The rest of this prompt was cut to fit.]");
    }
    let mut messages: Vec<openrouter::Message> = Vec::new();
    if !session.system.trim().is_empty() {
        messages.push(openrouter::Message {
            role: "system".into(),
            content: session.system.clone(),
        });
    }
    messages.push(openrouter::Message {
        role: "user".into(),
        content: prompt.clone(),
    });
    let mut events = String::new();
    events
        .push_str(&json!({"type": "prompt", "model": session.model, "prompt": prompt}).to_string());
    events.push('\n');

    // The Stop gate (#11487) on the Coder loop (#11520): a stop is checked
    // against the final checks, as on Claude Code, and a blocked stop is
    // the next turn's message.
    let last_text: Arc<Mutex<String>> = Arc::default();
    let finals: Vec<Check> = session
        .checks
        .iter()
        .filter(|c| harness_runs(c))
        .cloned()
        .collect();
    let gate = (gated && !finals.is_empty() && crate::token_savers::enabled()).then(|| {
        Arc::new(stop_gate(
            Arc::new(finals),
            root.clone(),
            session.baseline.clone(),
            last_text.clone(),
            briefed_agent::stop_gate::decisions(),
            folder,
        ))
    });
    // The hand's stop: the run's own, or its time running out.
    let stop = Arc::new(AtomicBool::new(false));
    let model_seen: Arc<Mutex<Option<String>>> = Arc::default();
    let briefed = session.briefed;
    let mut opened: BTreeSet<String> = BTreeSet::new();
    let mut misses = Vec::new();
    let mut running: Vec<(usize, String, Value)> = Vec::new();
    let mut sent_back = 0;
    let mut earlier = (0, 0);
    let result = loop {
        let watcher = {
            let (stop, run_stop, timeout) =
                (stop.clone(), transcript.cancel.clone(), session.timeout);
            async move {
                while !run_stop.load(Ordering::Relaxed) && started.elapsed() <= timeout {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                stop.store(true, Ordering::Relaxed);
                std::future::pending::<()>().await;
            }
        };
        let result = {
            let mut on_text = |_: &str| {};
            let mut on_model = {
                let model_seen = model_seen.clone();
                move |model: &str| *lock(&model_seen) = Some(model.to_owned())
            };
            let mut on_event = |event: crate::bundled_runtime::RuntimeEvent| {
                let crate::bundled_runtime::RuntimeEvent::Tool {
                    name,
                    input,
                    output,
                    running: still,
                } = event
                else {
                    return;
                };
                events.push_str(&json!({"type": "tool", "t_ms": started.elapsed().as_millis() as u64, "name": name, "input": input, "output": output, "running": still}).to_string());
                events.push('\n');
                if name == "Read"
                    && let Some(Some(path)) = inside(&root, &input)
                    && !briefed.contains(&path)
                    && opened.insert(path.clone())
                {
                    misses.push(path);
                }
                let entry = Entry::Tool {
                    name: name.clone(),
                    input: input.clone(),
                    output,
                    running: still,
                };
                // A running call's progress and its end update its own entry.
                match running
                    .iter()
                    .rposition(|(_, open, open_input)| *open == name && *open_input == input)
                {
                    Some(at) => {
                        transcript.set(running[at].0, entry);
                        if !still {
                            running.remove(at);
                        }
                    }
                    None => {
                        let index = transcript.push(entry);
                        if still {
                            running.push((index, name, input));
                        }
                    }
                }
            };
            let options = crate::models::GenerationOptions::default();
            // The turn's state lives on the heap: a solver hand runs on a plain
            // thread's stack, which a debug build's turn would overflow.
            let turn = Box::pin(provider.chat_with_plugins(
                crate::models::GATEWAY_AUTO,
                &options,
                messages.clone(),
                &execution,
                &mut on_text,
                &mut on_model,
                &mut on_event,
                &stop,
            ));
            tokio::select! {
                result = turn => result,
                () = watcher => unreachable!("the watcher never finishes"),
            }
        };
        let (Ok(reply), Some(gate)) = (&result, &gate) else {
            break result;
        };
        if sent_back >= STOP_GATE_ROUNDS || stop.load(Ordering::Relaxed) {
            break result;
        }
        reply.text.trim().clone_into(&mut lock(&last_text));
        let briefed_agent::stop_gate::Outcome::Block(reason) = gate.decide().await else {
            break result;
        };
        // Sent back: the stop's words stay in the transcript, and the gate's
        // verdict is the next turn's message.
        sent_back += 1;
        earlier.0 += reply.usage.prompt_tokens;
        earlier.1 += reply.usage.completion_tokens;
        if !reply.text.trim().is_empty() {
            transcript.push(Entry::Assistant {
                text: reply.text.trim().to_owned(),
                model: Some(reply.model.clone()).filter(|model| !model.is_empty()),
                elapsed_ms: Some(started.elapsed().as_millis() as u64),
            });
            messages.push(openrouter::Message {
                role: "assistant".into(),
                content: reply.text.trim().to_owned(),
            });
        }
        transcript.notice(
            "The agent stopped before the final checks pass; it was sent back with their verdict.",
        );
        events.push_str(&json!({"type": "stop_gate", "t_ms": started.elapsed().as_millis() as u64, "reason": reason}).to_string());
        events.push('\n');
        messages.push(openrouter::Message {
            role: "user".into(),
            content: reason,
        });
    };
    for (index, name, input) in running {
        transcript.set(
            index,
            Entry::Tool {
                name,
                input,
                output: json!({"error": "The agent stopped before this tool returned."}),
                running: false,
            },
        );
    }
    let t_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(reply) => {
            let model = Some(reply.model.clone())
                .filter(|model| !model.is_empty())
                .or_else(|| lock(&model_seen).clone());
            let text = reply.text.trim().to_owned();
            if !text.is_empty() {
                transcript.push(Entry::Assistant {
                    text: text.clone(),
                    model: model.clone(),
                    elapsed_ms: Some(t_ms),
                });
                work.reply = Some(text);
            }
            events.push_str(&json!({"type": "result", "t_ms": t_ms, "model": model, "tokens": reply.usage.total_tokens, "cost": reply.usage.cost}).to_string());
            events.push('\n');
            let (input, output) = (
                earlier.0 + reply.usage.prompt_tokens,
                earlier.1 + reply.usage.completion_tokens,
            );
            work.input_tokens = (input > 0).then_some(input);
            work.output_tokens = (output > 0).then_some(output);
            work.cost_usd = reply.usage.cost;
        }
        Err(why) => {
            work.error = Some(if transcript.cancelled() {
                "The run was stopped.".to_owned()
            } else if started.elapsed() > session.timeout {
                "The agent ran out of its time.".to_owned()
            } else {
                why
            });
        }
    }
    let _ = std::fs::write(folder.join(&session.events_file), events);
    work.wall_ms = t_ms;
    work.misses = misses;
    work.stop_gate = gate.map(|gate| lock(&gate.tally).clone());
    work
}

/// How many times the Stop gate sends a Coder-loop session back before
/// its last stop stands.
const STOP_GATE_ROUNDS: usize = 3;

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
