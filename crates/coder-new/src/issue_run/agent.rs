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

/// One constrained Claude Agent SDK session: its model, its built-in tools,
/// its in-process tools, and what it is told. `issue_run` runs one, the
/// briefed agent; `solver` runs four, each with the tools its hand needs.
pub struct Session {
    /// The agent card's title and detail line.
    pub title: String,
    pub detail: String,
    pub model: String,
    pub timeout: Duration,
    /// The built-in tools; file tools among them are held to the worktree.
    pub tools: Vec<String>,
    /// The checks a `run_check` tool offers; none means no such tool.
    pub checks: Vec<Check>,
    /// An in-process tool server of the hand's own, beside `run_check`.
    pub server: Option<SdkMcpServer>,
    pub system: String,
    pub prompt: String,
    /// The files the briefing listed; a `Read` of another is a miss.
    pub briefed: BTreeSet<String>,
    /// The file the session's raw messages are written to, in the run's
    /// folder.
    pub events_file: String,
    /// What a failed test is compared against: the repository and the
    /// commit the run started from. A test that already fails there is not
    /// the change's doing, so `run_check` doesn't fail on it.
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
            std::path::Path::new(&home).join(".openagents/coder-new/target"),
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
    let target = std::path::Path::new(&home).join(".openagents/coder-new/target");
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
            tools: TOOLS.iter().map(|t| (*t).to_owned()).collect(),
            checks: briefing.checks.clone(),
            server: None,
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
        let mut names = self.tools.clone();
        if !self.checks.is_empty() {
            names.push("run_check".into());
        }
        if let Some(server) = &self.server {
            names.extend(server.allowed_tool_names());
        }
        names.join(", ")
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
pub async fn run(
    session: Session,
    worktree: &Path,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    let root = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let checks = Arc::new(session.checks.clone());
    let tools_text = session.tools_text();
    let mut allowed: Vec<String> = session.tools.clone();
    let mut query_options = QueryOptions::new()
        .cwd(&root)
        .model(session.model.clone())
        .permission_mode(PermissionMode::Default)
        .no_session_persistence();
    let mut verdicts = None;
    if !checks.is_empty() {
        let (server, seen) = checks_server(checks.clone(), root.clone(), session.baseline.clone());
        allowed.extend(server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(server);
        verdicts = Some(seen);
    }
    if let Some(server) = session.server {
        allowed.extend(server.allowed_tool_names());
        query_options = query_options.sdk_mcp_server(server);
    }
    query_options.tools = Some(ToolsConfig::Names(session.tools.clone()));
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
