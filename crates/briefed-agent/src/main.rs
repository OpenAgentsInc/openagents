//! The briefed agent of the briefed-agent A/B bench (#11211).
//!
//! One Claude Agent SDK session turns one issue into a change, working from
//! a briefing that `scripts/bench/briefed-ab/briefing.py` wrote:
//!
//! - **System prompt**: the harness writes it from the briefing; this
//!   binary passes it as Claude Code's whole system prompt.
//! - **Tools**: only the built-in tools the config names (by default
//!   `Read`, `Edit`, `Write`, `Grep`, `Glob`) plus the MCP tools of the
//!   config's servers (the bench's `checks` server runs only the briefing's
//!   checks). No Bash.
//! - **Boundary**: a file tool whose path resolves outside the worktree is
//!   denied. No user or project settings load, and no MCP server but the
//!   config's.
//! - **Misses**: a `Read` of a file the briefing did not list is allowed and
//!   recorded as a miss, for the context finder (#11210).
//! - **Login**: the owner's Claude Code login; the CLI does not inherit
//!   `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN`.
//!
//! Usage: `briefed-agent CONFIG.json`. The config names the worktree, the
//! system prompt and first message files, the model, the effort, the time
//! limit, and where to write the event log (one JSON line per message) and
//! the summary (which includes how the `verify` calls went: calls,
//! failures, whether one passed, and the last status).

mod tools;
mod verify;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use claude_agent_sdk::{
    EffortLevel, McpServerConfig, PermissionMode, PermissionResult, QueryOptions, SdkMessage,
    SystemPromptConfig, ToolsConfig,
};
use futures::StreamExt;
use serde_json::{Value, json};

/// Credentials the CLI must not inherit, so it uses the Claude Code login.
const REMOVED_ENV: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
/// The file tools whose paths must stay inside the worktree.
const FILE_TOOLS: [&str; 6] = ["Read", "Edit", "MultiEdit", "Write", "Glob", "Grep"];
/// The default built-in tools.
const DEFAULT_TOOLS: [&str; 5] = ["Read", "Edit", "Write", "Grep", "Glob"];

type Shared<T> = Arc<Mutex<T>>;

fn lock<T>(shared: &Shared<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("briefed-agent: {error}");
            2
        }
    };
    std::process::exit(code);
}

fn text(config: &Value, key: &str) -> Result<String, String> {
    config[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("config: `{key}` is missing"))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
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

/// The worktree-relative path of a tool input's path, or `None` when it
/// lies outside the worktree. A file tool without a path works in the
/// worktree.
fn inside(root: &Path, input: &Value) -> Option<Option<String>> {
    let path = ["file_path", "path", "notebook_path"]
        .iter()
        .find_map(|key| input[*key].as_str());
    let Some(path) = path else {
        return Some(None);
    };
    let resolved = resolve(root, path);
    let relative = resolved.strip_prefix(root).ok()?;
    Some(Some(relative.to_string_lossy().into_owned()))
}

#[derive(Default)]
struct Ledger {
    tool_calls: BTreeMap<String, u64>,
    files_read: BTreeSet<String>,
    files_written: BTreeSet<String>,
    misses: Vec<Value>,
    denied: Vec<Value>,
    check_runs: u64,
}

async fn run() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: briefed-agent CONFIG.json")?;
    let config: Value = serde_json::from_slice(
        &std::fs::read(&path).map_err(|error| format!("reading {path}: {error}"))?,
    )
    .map_err(|error| format!("parsing {path}: {error}"))?;

    let worktree = PathBuf::from(text(&config, "worktree")?);
    let worktree = worktree.canonicalize().unwrap_or(worktree);
    let system_prompt = std::fs::read_to_string(text(&config, "system_prompt_path")?)
        .map_err(|error| format!("reading the system prompt: {error}"))?;
    let prompt = std::fs::read_to_string(text(&config, "prompt_path")?)
        .map_err(|error| format!("reading the prompt: {error}"))?;
    let events_path = text(&config, "events_path")?;
    let summary_path = text(&config, "summary_path")?;
    let briefed: BTreeSet<String> = strings(&config["briefed_files"]).into_iter().collect();
    let timeout = Duration::from_secs(config["timeout_secs"].as_u64().unwrap_or(1200));
    let mut tools = strings(&config["tools"]);
    if tools.is_empty() {
        tools = DEFAULT_TOOLS.iter().map(|&tool| tool.to_owned()).collect();
    }

    let mut options = QueryOptions::new()
        .cwd(&worktree)
        .permission_mode(PermissionMode::Default)
        .no_session_persistence();
    if let Some(model) = config["model"].as_str() {
        options = options.model(model);
    }
    if let Some(turns) = config["max_turns"].as_u64() {
        options = options.max_turns(u32::try_from(turns).unwrap_or(u32::MAX));
    }
    options.effort = match config["effort"].as_str() {
        Some("low") => Some(EffortLevel::Low),
        Some("medium") => Some(EffortLevel::Medium),
        Some("high") => Some(EffortLevel::High),
        Some("xhigh") => Some(EffortLevel::Xhigh),
        Some("max") => Some(EffortLevel::Max),
        _ => None,
    };
    options.tools = Some(ToolsConfig::Names(tools.clone()));
    options.system_prompt = Some(SystemPromptConfig::Custom(system_prompt));
    options.setting_sources = Some(Vec::new());
    options.strict_mcp_config = true;
    let mut allowed = tools.clone();
    if let Some(servers) = config["mcp"].as_object() {
        for (name, server) in servers {
            let env: HashMap<String, String> = server["env"]
                .as_object()
                .map(|env| {
                    env.iter()
                        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default();
            options = options.mcp_server(
                name,
                McpServerConfig::Stdio {
                    command: text(server, "command")?,
                    args: Some(strings(&server["args"])),
                    env: Some(env),
                },
            );
            allowed.push(format!("mcp__{name}"));
        }
    }
    let custom = strings(&config["custom"]);
    let finished = Arc::new(AtomicBool::new(false));
    let tally: Shared<verify::Tally> = Arc::default();
    if let Some(server) =
        tools::server(&worktree, &config, &custom, finished.clone(), tally.clone())
    {
        allowed.extend(server.allowed_tool_names());
        allowed.push("mcp__oa".to_owned());
        options = options.sdk_mcp_server(server);
    }
    options.env_remove = REMOVED_ENV.iter().map(|&name| name.to_owned()).collect();

    let ledger: Shared<Ledger> = Arc::default();
    let handler = {
        let ledger = ledger.clone();
        let root = worktree.clone();
        let allowed = allowed.clone();
        claude_agent_sdk::permission_handler(move |request| {
            let decision = decide(&root, &allowed, &ledger, &request.tool_name, &request.input);
            async move { Ok::<_, claude_agent_sdk::Error>(decision) }
        })
    };

    let mut events = String::new();
    let started = Instant::now();
    let mut result: Option<Value> = None;
    let mut error: Option<String> = None;
    let mut timed_out = false;
    let mut interrupted = false;
    let mut query = claude_agent_sdk::query_with_permissions(prompt, options, handler)
        .await
        .map_err(|error| format!("Claude Code didn't start: {error}"))?;
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        let next = match tokio::time::timeout(remaining, query.next()).await {
            Ok(next) => next,
            Err(_) => {
                timed_out = true;
                break;
            }
        };
        let message = match next {
            None => break,
            Some(Err(claude_agent_sdk::Error::UnrecognizedMessage { .. })) => continue,
            Some(Err(failure)) => {
                error = Some(failure.to_string());
                break;
            }
            Some(Ok(message)) => message,
        };
        let elapsed_ms = started.elapsed().as_millis();
        match message {
            SdkMessage::Assistant(assistant) => {
                observe(&worktree, &briefed, &ledger, &assistant.message);
                events.push_str(
                    &json!({"type": "assistant", "t_ms": elapsed_ms, "message": assistant.message})
                        .to_string(),
                );
                events.push('\n');
            }
            SdkMessage::User(user) => {
                if !interrupted && finished.load(Ordering::SeqCst) {
                    // `finish` passed: end the run without another turn.
                    interrupted = true;
                    let _ = query.interrupt().await;
                }
                events.push_str(
                    &json!({"type": "user", "t_ms": elapsed_ms, "message": user.message})
                        .to_string(),
                );
                events.push('\n');
            }
            SdkMessage::Result(done) => {
                let value = serde_json::to_value(&done).unwrap_or(Value::Null);
                events.push_str(
                    &json!({"type": "result", "t_ms": elapsed_ms, "result": value}).to_string(),
                );
                events.push('\n');
                result = Some(value);
                break;
            }
            SdkMessage::Unknown { type_name, raw } if type_name == "result" => {
                result = Some(raw);
                break;
            }
            _ => {}
        }
    }
    let _ = query.kill().await;
    let wall_ms = started.elapsed().as_millis();

    std::fs::write(&events_path, events).map_err(|error| format!("{events_path}: {error}"))?;
    let ledger = lock(&ledger);
    let summary = json!({
        "result": result,
        "error": error,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "tool_calls": ledger.tool_calls,
        "files_read": ledger.files_read,
        "files_written": ledger.files_written,
        "misses": ledger.misses,
        "denied": ledger.denied,
        "check_runs": ledger.check_runs,
        "verify": lock(&tally).to_json(),
        "finished": finished.load(Ordering::SeqCst),
    });
    std::fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).unwrap_or_default(),
    )
    .map_err(|error| format!("{summary_path}: {error}"))?;
    Ok(())
}

/// Allows a file tool inside the worktree and the config's tools; denies
/// the rest and records it.
fn decide(
    root: &Path,
    allowed: &[String],
    ledger: &Shared<Ledger>,
    tool: &str,
    input: &Value,
) -> PermissionResult {
    let named = allowed
        .iter()
        .any(|name| tool == name || tool.starts_with(&format!("{name}__")));
    if FILE_TOOLS.contains(&tool) {
        if named && inside(root, input).is_some() {
            return PermissionResult::allow(input.clone());
        }
        lock(ledger)
            .denied
            .push(json!({"tool": tool, "input": input, "why": "outside the worktree"}));
        return PermissionResult::deny(
            "That path is outside the task's worktree. Work only inside the worktree.",
        );
    }
    if named {
        return PermissionResult::allow(input.clone());
    }
    lock(ledger)
        .denied
        .push(json!({"tool": tool, "input": input, "why": "not in the tool set"}));
    PermissionResult::deny("That tool is not available in this task.")
}

/// Counts an assistant message's tool calls and the files they read and
/// write; a read of a file the briefing did not list is a miss.
fn observe(root: &Path, briefed: &BTreeSet<String>, ledger: &Shared<Ledger>, message: &Value) {
    let Some(blocks) = message["content"].as_array() else {
        return;
    };
    let mut ledger = lock(ledger);
    for block in blocks {
        if block["type"].as_str() != Some("tool_use") {
            continue;
        }
        let name = block["name"].as_str().unwrap_or("?").to_owned();
        let input = &block["input"];
        *ledger.tool_calls.entry(name.clone()).or_default() += 1;
        if name.starts_with("mcp__checks") || name == "mcp__oa__verify" || name == "mcp__oa__finish"
        {
            ledger.check_runs += 1;
        }
        let Some(Some(relative)) = inside(root, input) else {
            continue;
        };
        match name.as_str() {
            "Read" => {
                if !briefed.contains(&relative) && ledger.files_read.insert(relative.clone()) {
                    ledger
                        .misses
                        .push(json!({"file": relative, "tool": "Read"}));
                } else {
                    ledger.files_read.insert(relative);
                }
            }
            "Edit" | "MultiEdit" | "Write" => {
                let seen = ledger.files_read.contains(&relative);
                if !briefed.contains(&relative)
                    && !seen
                    && ledger.files_written.insert(relative.clone())
                {
                    ledger.misses.push(json!({"file": relative, "tool": name}));
                } else {
                    ledger.files_written.insert(relative);
                }
            }
            _ => {}
        }
    }
}
