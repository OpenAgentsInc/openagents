//! The briefed agent on Codex (#11308), on the Coder SDK's Codex backend
//! (#11530): the same briefing, checks, summary and hooks as the Claude
//! session in `main.rs`, run on `codex app-server` (`coder_sdk::codex`).
//!
//! - **System prompt**: the briefing replaces Codex's instructions
//!   (`model_instructions_file`), as it replaces Claude Code's.
//! - **Tools**: Codex's shell and `apply_patch`, and the briefing's check
//!   tools (`verify`, `finish`, ...) as Codex dynamic tools in this process
//!   (the SDK's one tool server, `oa`).
//! - **Hooks**: the registry `main.rs` composes for Claude Code (the worktree
//!   guard over the offered tools, the floors held, the shared-checkout
//!   fence, squeeze, the Stop gate). The thread runs under Codex's
//!   `untrusted` approvals, never its bypass flags, and the registry answers
//!   each command and patch before it runs; a denied one is declined and the
//!   agent told why in the same turn.
//! - **Login**: Codex's own (`CODEX_HOME`), never `OPENAI_API_KEY`.
//! - **Misses**: the paths of every patch the briefing did not list. Shell
//!   reads are not visible as paths, so misses count edits only.
//!
//! Events are kept one JSON line each as the backend reported them; the
//! summary has the shape `main.rs` writes, plus `"engine": "codex"`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use coder_sdk::codex::{Codex, Launch, Open, Output};
use serde_json::{Value, json};

use crate::verify::Tally;

/// What one Codex session observed.
#[derive(Default, Debug, PartialEq)]
pub struct Observed {
    pub tool_calls: BTreeMap<String, u64>,
    pub files_written: BTreeSet<String>,
    pub misses: Vec<Value>,
    pub denied: Vec<Value>,
    pub check_runs: u64,
}

impl Observed {
    /// Reads one backend output: a finished tool call is counted (a
    /// patch's files written, a briefing miss when unlisted), and a call
    /// the hooks refused is kept with why.
    pub fn see(&mut self, output: &Output, briefed: &BTreeSet<String>, worktree: &Path) {
        match output {
            Output::Tool {
                name,
                input,
                running: false,
                ..
            } => {
                let tool = match name.as_str() {
                    "Run" => "Bash",
                    other => other,
                };
                *self.tool_calls.entry(tool.to_owned()).or_insert(0) += 1;
                if tool == "verify" || tool == "finish" {
                    self.check_runs += 1;
                }
                for change in input["changes"].as_array().into_iter().flatten() {
                    let Some(path) = change["path"].as_str() else {
                        continue;
                    };
                    let path = Path::new(path)
                        .strip_prefix(worktree)
                        .unwrap_or(Path::new(path))
                        .to_string_lossy()
                        .trim_start_matches("./")
                        .to_owned();
                    if !briefed.contains(&path) && !self.files_written.contains(&path) {
                        self.misses
                            .push(json!({"path": path, "via": "file_change"}));
                    }
                    self.files_written.insert(path);
                }
            }
            Output::Gated(applied) => {
                if let coder_sdk::Permission::Deny { reason }
                | coder_sdk::Permission::Ask { step: reason } = &applied.permission
                {
                    self.denied.push(json!({
                        "tool": applied.tool,
                        "input": applied.input,
                        "why": reason,
                        "by": applied.decided_by,
                    }));
                }
            }
            _ => {}
        }
    }
}

/// One backend output as an event-log line's `event`.
fn event_of(output: &Output) -> Value {
    match output {
        Output::Thread(thread) => json!({"thread": thread}),
        Output::Model(model) => json!({"model": model}),
        Output::Text(text) => json!({"text": text}),
        Output::Thinking(thinking) => json!({"thinking": thinking}),
        Output::Tool {
            name,
            input,
            output,
            running,
        } => json!({"tool": name, "input": input, "output": output, "running": running}),
        Output::Gated(applied) => json!({"gated": applied}),
        Output::Usage(usage) => json!({"usage": usage}),
        Output::RateLimits(limits) => json!({"rate_limits": limits}),
    }
}

/// The thread's token usage (`tokenUsage.total`) in the shape `codex exec`
/// reported and the bench reads: `input_tokens`, `cached_input_tokens`,
/// `output_tokens`.
fn usage(reported: &Value) -> Value {
    let total = &reported["total"];
    json!({
        "input_tokens": total["inputTokens"],
        "cached_input_tokens": total["cachedInputTokens"],
        "output_tokens": total["outputTokens"],
        "reasoning_output_tokens": total["reasoningOutputTokens"],
    })
}

/// How `codex app-server` starts for `config`: the briefing as the whole
/// instructions, no web search, the squeezer's variables, and Codex's own
/// login rather than an API key.
pub fn launch(
    config: &Value,
    worktree: &Path,
    env: impl IntoIterator<Item = (String, String)>,
) -> Launch {
    let toml = |value: &Value| value.to_string();
    let mut launch = Launch::new(config["codex_bin"].as_str().unwrap_or("codex"));
    if let Some(path) = config["system_prompt_path"].as_str() {
        launch
            .config
            .push(format!("model_instructions_file={}", toml(&json!(path))));
    }
    launch.config.push("web_search=\"disabled\"".to_owned());
    launch.env = env.into_iter().collect();
    launch.env_remove = vec!["OPENAI_API_KEY".to_owned()];
    launch.cwd = Some(worktree.to_path_buf());
    launch
}

/// Runs the briefed agent on Codex and writes the event log and summary.
pub async fn run(config: &Value, _config_path: &str) -> Result<(), String> {
    let worktree = PathBuf::from(crate::text(config, "worktree")?);
    let worktree = worktree.canonicalize().unwrap_or(worktree);
    let prompt = std::fs::read_to_string(crate::text(config, "prompt_path")?)
        .map_err(|error| format!("reading the prompt: {error}"))?;
    let events_path = crate::text(config, "events_path")?;
    let summary_path = crate::text(config, "summary_path")?;
    let briefed: BTreeSet<String> = crate::strings(&config["briefed_files"])
        .into_iter()
        .collect();
    let timeout = Duration::from_secs(config["timeout_secs"].as_u64().unwrap_or(1200));

    let custom = crate::strings(&config["custom"]);
    let finished = Arc::new(AtomicBool::new(false));
    let tally: Arc<Mutex<Tally>> = Arc::default();
    let tools =
        crate::tools::tool_server(&worktree, config, &custom, finished.clone(), tally.clone());
    let mut offered = vec![
        coder_sdk::codex::COMMAND_TOOL.to_owned(),
        coder_sdk::codex::PATCH_TOOL.to_owned(),
    ];
    if let Some(tools) = &tools {
        offered.push(format!("mcp__{}", tools.name));
    }
    let last_text: crate::Shared<String> = Arc::default();
    let composed = crate::compose(&crate::Setup {
        config,
        worktree: &worktree,
        tools: &offered,
        allowed: &offered,
        summary_path: &summary_path,
        finished: &finished,
        last_text: &last_text,
        timeout,
    })?;
    let hooks = coder_sdk::codex::hooks(composed.registry, coder_sdk::Budget::default())
        .map_err(|refused| format!("Codex can't run these hooks: {refused:?}"))?;

    let started = Instant::now();
    let mut observed = Observed::default();
    let mut events = String::new();
    let mut timed_out = false;
    let mut ended = None;
    let mut error = None;
    let open = Open {
        cwd: worktree.clone(),
        model: config["model"]
            .as_str()
            .filter(|model| !model.is_empty())
            .map(str::to_owned),
        effort: config["effort"].as_str().map(str::to_owned),
        developer_instructions: None,
        resume: None,
        tools,
    };
    let mut emit = |output: Output| {
        if let Output::Text(text) = &output {
            crate::lock(&last_text).push_str(text);
        }
        if let Output::Tool { running: true, .. } = &output {
            crate::lock(&last_text).clear();
        }
        observed.see(&output, &briefed, &worktree);
        events.push_str(
            &json!({"type": "codex", "t_ms": started.elapsed().as_millis(), "event": event_of(&output)})
                .to_string(),
        );
        events.push('\n');
    };
    let session = async {
        let mut codex = Codex::spawn(&launch(config, &worktree, composed.env), hooks).await?;
        codex.open(&open, &mut emit).await?;
        let turn = codex.turn(&prompt, None, None, &mut emit).await;
        codex.close().await;
        turn
    };
    match tokio::time::timeout(timeout, session).await {
        Err(_) => timed_out = true,
        Ok(Ok(done)) => ended = Some(done),
        Ok(Err(failure)) => error = Some(failure),
    }
    let wall_ms = started.elapsed().as_millis();
    if let Some(done) = &ended
        && error.is_none()
    {
        error.clone_from(&done.error);
    }

    std::fs::write(&events_path, events).map_err(|error| format!("{events_path}: {error}"))?;
    let summary = json!({
        "engine": "codex",
        "model": config["model"].as_str().filter(|model| !model.is_empty()),
        "result": ended.as_ref().map(|done| json!({
            "thread_id": done.thread,
            "num_turns": 1 + done.continued,
            "status": done.status,
            "model": done.model,
            "usage": usage(&done.usage),
            "result": done.text,
        })),
        "error": error,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "tool_calls": observed.tool_calls,
        "files_read": [],
        "files_written": observed.files_written,
        "misses": observed.misses,
        "denied": observed.denied,
        "check_runs": observed.check_runs,
        "verify": crate::lock(&tally).to_json(),
        "finished": finished.load(Ordering::SeqCst),
        "stop_gate": composed.gate.map(|gate| gate.tally.lock().map(|t| t.clone()).unwrap_or_default()),
    });
    std::fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).unwrap_or_default(),
    )
    .map_err(|error| format!("{summary_path}: {error}"))?;
    Ok(())
}

/// The `verify` tally from the log the serving process appended to, one
/// JSON object per line with a `status` (or `verdict.status`).
pub fn tally_from_log(path: &str) -> Tally {
    let mut tally = Tally::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return tally;
    };
    for line in text.lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(status) = row["status"]
            .as_str()
            .or_else(|| row.pointer("/verdict/status").and_then(Value::as_str))
        {
            tally.record(status);
        }
    }
    tally
}

/// `briefed-agent --serve CONFIG.json`: the briefing's check tools as an
/// MCP server over stdio, for an engine that cannot host them in-process:
/// the Coder SDK's one tool server, served on stdio.
pub async fn serve(config: &Value) -> Result<(), String> {
    let worktree = std::path::PathBuf::from(crate::text(config, "worktree")?);
    let worktree = worktree.canonicalize().unwrap_or(worktree);
    let custom = crate::strings(&config["custom"]);
    let finished = Arc::new(AtomicBool::new(false));
    let tally: Arc<Mutex<Tally>> = Arc::default();
    let server = crate::tools::tool_server(&worktree, config, &custom, finished, tally)
        .ok_or("the config names no check tools to serve")?;
    coder_sdk::serve_stdio(&server).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in `codex app-server`: answers the handshake, the thread and
    /// the turn, asks to run a merge, a `git stash`, a patch outside the
    /// worktree and `cargo test`, keeping each answer, then ends the turn.
    const STAND_IN: &str = r#"#!/bin/sh
[ "$1" = app-server ] || exit 2
printf '%s\n' "$*" > "$REC/args"
read l; echo '{"jsonrpc":"2.0","id":1,"result":{"userAgent":"codex_cli_rs/0.162.1 (stand-in)"}}'
read l
read l; printf '%s\n' "$l" > "$REC/thread"; echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thr-1"},"model":"stand-in"}}'
read l; echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-1"}}}'
ask() {
  echo "$1"
  while read l; do
    case "$l" in *'"result":{"decision"'*) printf '%s\n' "$l" >> "$REC/decisions"; return;; esac
  done
}
command() {
  ask '{"jsonrpc":"2.0","id":'"$1"',"method":"item/commandExecution/requestApproval","params":{"threadId":"thr-1","turnId":"turn-1","itemId":"c'"$1"'","command":"/bin/zsh -lc '"'$2'"'","cwd":"'"$PWD"'"}}'
}
command 100 'gh pr merge 5 --squash'
command 101 'git stash'
echo '{"jsonrpc":"2.0","method":"item/started","params":{"item":{"type":"fileChange","id":"p1","status":"inProgress","changes":[{"path":"/etc/briefed-outside","kind":{"type":"add"},"diff":"x"}]}}}'
ask '{"jsonrpc":"2.0","id":102,"method":"item/fileChange/requestApproval","params":{"threadId":"thr-1","turnId":"turn-1","itemId":"p1"}}'
command 103 'cargo test'
echo '{"jsonrpc":"2.0","method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"Done.","phase":"final_answer"}}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thr-1","turn":{"id":"turn-1","status":"completed","error":null}}}'
cat > /dev/null
"#;

    /// The briefed agent's Codex engine, started through `run` on a
    /// stand-in app-server, runs the composition the Claude engine runs:
    /// the floors hold a merge, the shared-checkout fence refuses a
    /// `git stash`, the worktree guard refuses a patch outside the
    /// worktree, and `cargo test` runs; the thread is under Codex's
    /// approvals, never its bypass flags.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_codex_session_meets_the_floors_the_fence_and_the_worktree_guard() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (worktree, rec) = (root.join("tree"), root.join("rec"));
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&rec).unwrap();
        let program = root.join("codex");
        std::fs::write(&program, STAND_IN).unwrap();
        let mut permissions = std::fs::metadata(&program).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&program, permissions).unwrap();
        std::fs::write(root.join("prompt.md"), "Fix it.").unwrap();
        std::fs::write(root.join("system.md"), "The briefing.").unwrap();
        // The stand-in reads where to record from its environment.
        // SAFETY: this test alone reads REC.
        unsafe { std::env::set_var("REC", &rec) };
        let at = |name: &str| root.join(name).display().to_string();
        let config = json!({
            "engine": "codex", "codex_bin": at("codex"), "worktree": worktree,
            "prompt_path": at("prompt.md"), "system_prompt_path": at("system.md"),
            "events_path": at("events.jsonl"), "summary_path": at("summary.json"),
            "timeout_secs": 30,
        });
        run(&config, "agent.json").await.unwrap();

        let decisions = std::fs::read_to_string(rec.join("decisions")).unwrap();
        let decisions: Vec<Value> = decisions
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["result"]["decision"].clone())
            .collect();
        assert_eq!(decisions, ["decline", "decline", "decline", "accept"]);
        let summary: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("summary.json")).unwrap())
                .unwrap();
        let why: Vec<&str> = summary["denied"]
            .as_array()
            .unwrap()
            .iter()
            .map(|denied| denied["why"].as_str().unwrap())
            .collect();
        assert_eq!(why.len(), 3, "{summary}");
        assert!(why[0].starts_with("Held for the person"), "{why:?}");
        assert!(why[1].contains("git stash"), "{why:?}");
        assert_eq!(why[2], coder_sdk::worktree::OUTSIDE);
        assert_eq!(summary["result"]["result"], "Done.");
        assert_eq!(summary["error"], Value::Null);
        let args = std::fs::read_to_string(rec.join("args")).unwrap();
        assert!(args.contains("model_instructions_file"), "{args}");
        assert!(!args.contains("bypass"), "{args}");
        let thread: Value =
            serde_json::from_str(&std::fs::read_to_string(rec.join("thread")).unwrap()).unwrap();
        assert_eq!(thread["params"]["approvalPolicy"], "untrusted");
    }
}
