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
//! - **Login**: the Claude Code login in the environment; the CLI does not
//!   inherit `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN`, unless the
//!   config sets `inherit_api_key` (the person's own API key is the run's
//!   credential).
//!
//! - **Squeeze** (#11486): with `squeeze: true` and `Bash` in `tools`, a
//!   noisy build or test command runs through this binary's `--squeeze`
//!   mode, so its output enters the history small; the full output goes to
//!   the run's `squeeze` directory (`squeeze_dir`), which `Read` may open,
//!   and each decision to its `decisions.jsonl`.
//!
//! - **Stop gate** (#11487): with `stop_gate: true`, an agent that ends
//!   its turn without `finish` passing is sent back with the final checks'
//!   verdict, unless it says it is blocked or asks a question
//!   (`stop_gate.rs`); decisions go to the run's `stop_gate.jsonl`.
//!
//! Other engines run the same briefing, checks and hooks behind the same
//! summary (#11308, #11530): `engine: "codex"` (Codex on the Coder SDK's
//! app-server backend, the check tools in process, `codex.rs`), `engine:
//! "opencode"` and `engine: "gemini"` (OpenCode, Gemini on Vertex through
//! OpenCode, `opencode.rs`, the check tools over MCP from this binary's
//! `--serve CONFIG.json` mode and the hooks through Coder's OpenCode plugin
//! and this binary's `coder-hook`).
//!
//! Usage: `briefed-agent CONFIG.json`. The config names the worktree, the
//! system prompt and first message files, the model, the effort, the time
//! limit, and where to write the event log (one JSON line per message) and
//! the summary (which includes how the `verify` calls went: calls,
//! failures, whether one passed, and the last status).

mod tools;
mod verify;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use briefed_agent::stop_gate;
use claude_agent_sdk::{
    EffortLevel, McpServerConfig, PermissionMode, QueryOptions, SdkMessage, SystemPromptConfig,
    ToolsConfig,
};
use futures::StreamExt;
use serde_json::{Value, json};

mod codex;
mod opencode;

/// Credentials the CLI must not inherit, so it uses the Claude Code login.
const REMOVED_ENV: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
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

/// What a session's hooks are composed from.
struct Setup<'a> {
    config: &'a Value,
    worktree: &'a Path,
    tools: &'a [String],
    allowed: &'a [String],
    summary_path: &'a str,
    finished: &'a Arc<AtomicBool>,
    last_text: &'a Shared<String>,
    timeout: Duration,
}

/// A session's hooks: the registry, the variables the squeezer reads, and
/// the Stop gate when it is on.
struct Composed {
    registry: coder_sdk::Registry,
    env: HashMap<String, String>,
    gate: Option<Arc<stop_gate::StopGate>>,
}

/// The briefed agent's hooks on the Coder SDK (#11509): the worktree guard
/// (the offered tools, file paths inside the worktree, and a squeezed
/// command's full output readable), the floors held for the person, since
/// nobody answers a bench run from inside, and with their config switches
/// the squeezer (#11486) and the Stop gate (#11487).
fn compose(setup: &Setup<'_>) -> Result<Composed, String> {
    let config = setup.config;
    let mut registry = coder_sdk::Registry::new();
    let mut env = HashMap::new();
    let mut guard =
        coder_sdk::worktree::Guard::new(setup.worktree).offering(setup.allowed.to_vec());
    registry.register(coder_sdk::floors::held_hook())?;
    // The shared-checkout fence (#11532): no git command that throws away
    // uncommitted work, and no write to the checkout a Coder worktree came
    // from.
    registry.register(coder_sdk::checkout_guard::hook(setup.worktree.to_path_buf()))?;

    // System One hook 1 (#11486): noisy Bash output squeezed.
    if config["squeeze"].as_bool() == Some(true)
        && setup.tools.iter().any(|tool| tool == "Bash")
        && let Ok(this) = std::env::current_exe()
    {
        let dir = config["squeeze_dir"].as_str().map_or_else(
            || {
                Path::new(setup.summary_path)
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("squeeze")
            },
            PathBuf::from,
        );
        std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        let dir = dir.canonicalize().unwrap_or(dir);
        env.insert(squeeze::DIR_VAR.to_owned(), dir.display().to_string());
        env.insert(
            squeeze::LOG_VAR.to_owned(),
            dir.join("decisions.jsonl").display().to_string(),
        );
        registry.register(squeeze::sdk_hook(squeeze::invoker(&this, &["--squeeze"])))?;
        // A squeezed command's full output may be read.
        guard = guard.reading(dir);
    }
    registry.register(guard.hook())?;

    // System One hook 2 (#11487): no done without an observation.
    let mut gate = None;
    if config["stop_gate"].as_bool() == Some(true)
        && let Some(verify) = verify::Verify::from_config(setup.worktree, config)
    {
        let verify = Arc::new(verify);
        let (for_check, for_tree) = (verify.clone(), verify);
        let check: stop_gate::Check = Arc::new(move || {
            let verify = for_check.clone();
            Box::pin(async move { verify.final_check().await })
        });
        let tree: stop_gate::Tree = Arc::new(move || {
            let verify = for_tree.clone();
            Box::pin(async move { verify.tree().await })
        });
        let log = Path::new(setup.summary_path)
            .parent()
            .map(|dir| dir.join("stop_gate.jsonl"));
        let built = Arc::new(stop_gate::StopGate::new(
            setup.finished.clone(),
            setup.last_text.clone(),
            stop_gate::decisions(),
            check,
            tree,
            log,
        ));
        registry.register(built.clone().sdk_hook(setup.timeout))?;
        gate = Some(built);
    }
    Ok(Composed {
        registry,
        env,
        gate,
    })
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, command] = args.as_slice()
        && flag == "--squeeze"
    {
        std::process::exit(squeeze::main(std::slice::from_ref(command)));
    }
    // The hook bridge Codex's and OpenCode's hooks run (#11530): asks the
    // run's host, or with none holds the floors alone.
    if args.first().map(String::as_str) == Some(coder_sdk::bridge::COMMAND) {
        std::process::exit(coder_sdk::bridge::main(
            &args[1..],
            coder_sdk::floors::held_decision,
        ));
    }
    let (serve, path) = match args.as_slice() {
        [flag, path] if flag == "--serve" => (true, path.clone()),
        [path] => (false, path.clone()),
        _ => return Err("usage: briefed-agent [--serve] CONFIG.json".to_owned()),
    };
    let config: Value = serde_json::from_slice(
        &std::fs::read(&path).map_err(|error| format!("reading {path}: {error}"))?,
    )
    .map_err(|error| format!("parsing {path}: {error}"))?;
    if serve {
        return codex::serve(&config).await;
    }
    match config["engine"].as_str() {
        None | Some("claude") => {}
        Some("codex") => return codex::run(&config, &path).await,
        Some(engine @ ("opencode" | "gemini")) => {
            return opencode::run(&config, &path, engine).await;
        }
        Some(other) => {
            return Err(format!(
                "config: engine `{other}` is not claude, codex, opencode, or gemini"
            ));
        }
    }

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
    // `inherit_api_key`: the run's own credential is an API key (the
    // person's own, `openagents work`), so the CLI keeps it.
    let keep_key = config["inherit_api_key"].as_bool().unwrap_or(false);
    options.env_remove = REMOVED_ENV
        .iter()
        .filter(|&&name| !(keep_key && name == "ANTHROPIC_API_KEY"))
        .map(|&name| name.to_owned())
        .collect();

    // The session's hooks, one registry on the Coder SDK's Claude backend
    // (#11509): the worktree guard, the floors, the squeezer and the Stop
    // gate, composed by the SDK's rules.
    let last_text: Shared<String> = Arc::default();
    let composed = compose(&Setup {
        config: &config,
        worktree: &worktree,
        tools: &tools,
        allowed: &allowed,
        summary_path: &summary_path,
        finished: &finished,
        last_text: &last_text,
        timeout,
    })?;
    if !composed.env.is_empty() {
        options
            .env
            .get_or_insert_with(HashMap::new)
            .extend(composed.env.clone());
    }
    let backend = coder_sdk::claude::Claude::new(composed.registry, coder_sdk::Budget::default())
        .map_err(|refused| format!("{refused:?}"))?;
    options = backend.install(options, coder_sdk::claude::PreTool::Permission);
    let gate = composed.gate;

    let ledger: Shared<Ledger> = Arc::default();
    let handler = {
        let ledger = ledger.clone();
        let backend = backend.clone();
        claude_agent_sdk::permission_handler(move |request| {
            let (ledger, backend) = (ledger.clone(), backend.clone());
            async move {
                let applied = backend
                    .fire(coder_sdk::Call::tool(
                        coder_sdk::Event::PreToolUse,
                        &request.tool_name,
                        request.input.clone(),
                    ))
                    .await;
                if let coder_sdk::Permission::Deny { reason } = &applied.permission {
                    lock(&ledger).denied.push(json!({
                        "tool": request.tool_name,
                        "input": request.input,
                        "why": reason,
                        "by": applied.decided_by,
                    }));
                }
                Ok::<_, claude_agent_sdk::Error>(coder_sdk::claude::permission_result(&applied))
            }
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
                if let Some(text) = last_text_of(&assistant.message) {
                    *lock(&last_text) = text;
                }
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
        "stop_gate": gate.map(|gate| gate.tally.lock().map(|t| t.clone()).unwrap_or_default()),
    });
    std::fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).unwrap_or_default(),
    )
    .map_err(|error| format!("{summary_path}: {error}"))?;
    Ok(())
}

/// The text of an assistant message, when it has any.
fn last_text_of(message: &Value) -> Option<String> {
    let text: Vec<&str> = message["content"]
        .as_array()?
        .iter()
        .filter(|block| block["type"].as_str() == Some("text"))
        .filter_map(|block| block["text"].as_str())
        .collect();
    (!text.is_empty()).then(|| text.join("\n"))
}

/// Counts an assistant message's tool calls and the files they read and
/// write; a read or edit of a file the briefing did not list is a miss,
/// but writing a file that does not exist yet is not.
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
        let Some(Some(relative)) = coder_sdk::worktree::relative(root, input) else {
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
                // A Write that creates a file no finder could have listed.
                let created = name == "Write" && !root.join(&relative).exists();
                if !briefed.contains(&relative)
                    && !seen
                    && !created
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

#[cfg(test)]
mod compose_tests {
    use super::*;

    /// The briefed agent's session, composed as `run` composes it, replays
    /// a scripted session: the floors hold a merge, the guard refuses a
    /// path through a symbolic link out of the worktree and a tool the
    /// session wasn't offered, and an edit inside runs.
    #[cfg(unix)]
    #[test]
    fn a_scripted_session_meets_the_floors_and_the_worktree_guard() {
        let worktree = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "s").unwrap();
        let root = worktree.path().canonicalize().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("link")).unwrap();
        let tools: Vec<String> = ["Read", "Edit", "Bash"].map(str::to_owned).to_vec();
        let composed = compose(&Setup {
            config: &json!({}),
            worktree: &root,
            tools: &tools,
            allowed: &tools,
            summary_path: &root.join("summary.json").display().to_string(),
            finished: &Arc::default(),
            last_text: &Arc::default(),
            timeout: Duration::from_secs(60),
        })
        .unwrap();
        let step =
            |tool: &str, input: Value| json!({"event": "PreToolUse", "tool": tool, "input": input});
        let recorded = json!({
            "schema": coder_sdk::RECORDED_SESSION_SCHEMA,
            "events": [
                step("Bash", json!({"command": "gh pr merge 5 --squash"})),
                step("Read", json!({"file_path": root.join("link/secret").display().to_string()})),
                step("Write", json!({"file_path": "a.rs"})),
                step("Edit", json!({"file_path": "src/lib.rs"})),
                step("Bash", json!({"command": "git stash"})),
            ],
        });
        let session = coder_sdk::ScriptedSession::from_json(&recorded.to_string()).unwrap();
        let mut runner = coder_sdk::Runner::new(composed.registry, coder_sdk::Budget::default());
        let replay = session.replay(&mut runner).unwrap();
        let permissions: Vec<_> = replay
            .applied
            .iter()
            .map(|a| a.permission.clone())
            .collect();
        assert!(
            matches!(&permissions[0], coder_sdk::Permission::Deny { reason } if reason.starts_with("Held for the person")),
            "{permissions:?}"
        );
        assert_eq!(
            permissions[1],
            coder_sdk::Permission::deny(coder_sdk::worktree::OUTSIDE)
        );
        assert_eq!(
            permissions[2],
            coder_sdk::Permission::deny(coder_sdk::worktree::NOT_OFFERED)
        );
        assert_eq!(permissions[3], coder_sdk::Permission::NoOpinion);
        // The shared-checkout fence (#11532).
        assert!(
            matches!(&permissions[4], coder_sdk::Permission::Deny { reason } if reason.contains("git stash")),
            "{permissions:?}"
        );
    }
}

#[cfg(test)]
mod observe_tests {
    use super::*;

    fn write(root: &Path, tool: &str, file: &str) -> Ledger {
        let ledger: Shared<Ledger> = Arc::default();
        let message = json!({"content": [{
            "type": "tool_use",
            "name": tool,
            "input": {"file_path": root.join(file).to_string_lossy()},
        }]});
        observe(root, &BTreeSet::new(), &ledger, &message);
        std::mem::take(&mut *lock(&ledger))
    }

    #[test]
    fn writing_a_new_file_is_not_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ledger = write(&root, "Write", "src/new_module.rs");
        assert!(ledger.files_written.contains("src/new_module.rs"));
        assert!(ledger.misses.is_empty(), "{:?}", ledger.misses);
    }

    #[test]
    fn editing_an_unlisted_existing_file_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("old.rs"), "").unwrap();
        for tool in ["Edit", "Write"] {
            let ledger = write(&root, tool, "old.rs");
            assert!(ledger.files_written.contains("old.rs"));
            assert_eq!(ledger.misses, vec![json!({"file": "old.rs", "tool": tool})]);
        }
    }
}
