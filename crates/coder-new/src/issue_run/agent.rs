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
        } else if line.starts_with("test result:") || line.starts_with("failures:") {
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
    let result = tokio::process::Command::new(program)
        .args(args)
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .output()
        .await;
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
fn result_text(block: &Value) -> String {
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

/// Runs the briefed agent to the end, drawing its work as it goes.
pub async fn work(
    options: &super::Options,
    base: &Base,
    briefing: &Briefing,
    folder: &Path,
    transcript: &mut Transcript,
) -> Work {
    let root = base
        .worktree
        .canonicalize()
        .unwrap_or_else(|_| base.worktree.clone());
    let checks = Arc::new(briefing.checks.clone());
    let verdicts: Arc<Mutex<Vec<(String, bool)>>> = Arc::default();
    let server = {
        let (checks, root, verdicts) = (checks.clone(), root.clone(), verdicts.clone());
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
                async move {
                    let id = args["check"].as_str().unwrap_or_default().to_owned();
                    let (text, ok) = run_check(&checks, &root, &id, args["filter"].as_str()).await;
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
    let mut allowed = server.allowed_tool_names();
    allowed.extend(TOOLS.iter().map(|t| (*t).to_owned()));
    let mut query_options = QueryOptions::new()
        .cwd(&root)
        .model(options.model.clone())
        .permission_mode(PermissionMode::Default)
        .no_session_persistence()
        .sdk_mcp_server(server);
    query_options.tools = Some(ToolsConfig::Names(
        TOOLS.iter().map(|t| (*t).to_owned()).collect(),
    ));
    query_options.allowed_tools = Some(allowed.clone());
    query_options.system_prompt = Some(SystemPromptConfig::Custom(briefing.system.clone()));
    query_options.setting_sources = Some(Vec::new());
    query_options.strict_mcp_config = true;
    query_options.env_remove = REMOVED_ENV.iter().map(|n| (*n).to_owned()).collect();

    let card = json!({
        "kind": "agent",
        "title": "Briefed agent",
        "detail": "Claude Agent SDK session on this computer's Claude Code login",
        "rows": [
            {"label": "model", "text": options.model},
            {"label": "tools", "text": "Read, Edit, Write, Grep, Glob (held to the worktree) and run_check"},
            {"label": "checks", "text": briefing.checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>().join(", ")},
            {"label": "briefing", "text": format!("{} files, {} characters in the system prompt", briefing.files.len(), briefing.system.chars().count())},
        ],
    });
    let card_index = transcript.card(card.clone());

    let briefed: BTreeSet<String> = briefing.files.iter().cloned().collect();
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
        briefing.prompt.clone(),
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
        let left = options.timeout.saturating_sub(started.elapsed());
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
    let _ = std::fs::write(folder.join("agent-events.jsonl"), events);
    work.wall_ms = started.elapsed().as_millis() as u64;
    work.misses = lock(&misses).clone();
    work.checks = lock(&verdicts).clone();
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
    let mut verdicts = Vec::new();
    for check in briefing
        .checks
        .iter()
        .filter(|c| c.id.starts_with("check:"))
    {
        if transcript.cancelled() {
            break;
        }
        let input = json!({"command": check.argv.join(" "), "check": check.id});
        let index = transcript.push(Entry::Tool {
            name: "Run".into(),
            input: input.clone(),
            output: Value::Null,
            running: true,
        });
        let (text, ok) = run_check(&briefing.checks, &base.worktree, &check.id, None).await;
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
        verdicts.push((check.id.clone(), ok));
    }
    verdicts
}
