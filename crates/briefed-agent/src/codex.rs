//! The briefed agent on Codex CLI (#11308): the same briefing, checks, and
//! summary as the Claude session in `main.rs`, run as `codex exec --json`.
//!
//! - **System prompt**: the briefing replaces Codex's instructions
//!   (`model_instructions_file`), as it replaces Claude Code's.
//! - **Tools**: Codex has no file tools of its own, only its shell and
//!   `apply_patch`. The briefing's check tools (`verify`, `finish`, ...)
//!   reach it as the MCP server `oa`, which is this same binary in
//!   `--serve CONFIG.json` mode over stdio ([`serve`]). Codex's sandbox
//!   refuses MCP calls under `approval_policy = never`, so the run bypasses
//!   its approvals and sandbox, as `coder-delegate` does; the worktree is
//!   the working directory, and `verify` only reads the worktree.
//! - **Login**: Codex's own (`CODEX_HOME`); user config and rules do not
//!   load, and the session is not kept.
//! - **Misses**: the paths of every `file_change` the briefing did not list.
//!   Shell reads are not visible as paths, so misses count edits only.
//!
//! Events are kept one JSON line each as Codex printed them; the summary
//! has the shape `main.rs` writes, plus `"engine": "codex"`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::AsyncBufReadExt;

use crate::verify::Tally;

/// What one run of `codex exec --json` observed.
#[derive(Default, Debug, PartialEq)]
pub struct Observed {
    pub tool_calls: BTreeMap<String, u64>,
    pub files_written: BTreeSet<String>,
    pub misses: Vec<Value>,
    pub check_runs: u64,
    pub turns: u64,
    pub usage: BTreeMap<String, u64>,
    pub last_message: Option<String>,
    pub thread_id: Option<String>,
    pub error: Option<String>,
}

impl Observed {
    /// Reads one event line, with `file_change` paths made relative to
    /// `worktree`.
    pub fn see_in(&mut self, event: &Value, briefed: &BTreeSet<String>, worktree: &Path) {
        match event["type"].as_str() {
            Some("thread.started") => {
                self.thread_id = event["thread_id"].as_str().map(str::to_owned);
            }
            Some("turn.completed") => {
                self.turns += 1;
                if let Some(usage) = event["usage"].as_object() {
                    for (key, value) in usage {
                        if let Some(n) = value.as_u64() {
                            *self.usage.entry(key.clone()).or_insert(0) += n;
                        }
                    }
                }
            }
            Some("turn.failed") => {
                let message = event
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("the turn failed");
                self.error = Some(message.to_owned());
            }
            Some("error") => {
                if let Some(message) = event["message"].as_str() {
                    self.error.get_or_insert_with(|| message.to_owned());
                }
            }
            Some("item.completed") => {
                let item = &event["item"];
                match item["type"].as_str() {
                    Some("agent_message") => {
                        self.last_message = item["text"].as_str().map(str::to_owned);
                    }
                    Some("command_execution") => self.count("Bash"),
                    Some("file_change") => {
                        self.count("Edit");
                        for change in item["changes"].as_array().into_iter().flatten() {
                            if let Some(path) = change["path"].as_str() {
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
                    }
                    Some("mcp_tool_call") => {
                        let tool = item["tool"].as_str().unwrap_or("mcp");
                        self.count(tool);
                        if tool == "verify" || tool == "finish" {
                            self.check_runs += 1;
                        }
                    }
                    Some("web_search") => self.count("WebSearch"),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn count(&mut self, tool: &str) {
        *self.tool_calls.entry(tool.to_owned()).or_insert(0) += 1;
    }
}

/// The arguments of `codex exec` for `config`: JSON output, the briefing as
/// the whole instructions, the `oa` MCP server as this binary serving
/// `config_path`, no user config, no session, no approval prompts, and the
/// prompt from stdin. Without `model`, Codex picks its
/// login's default model. `env_names` are the variables Codex hands the
/// `oa` server: Codex starts MCP servers with a short default environment,
/// and `verify`'s checker needs the run's own (the bench's `AB_*`, the
/// target dir).
pub fn argv(
    config: &Value,
    config_path: &str,
    self_exe: &str,
    worktree: &Path,
    env_names: &[String],
) -> Vec<String> {
    let toml = |value: &Value| value.to_string();
    let mut args: Vec<String> = [
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--ephemeral",
        "--ignore-user-config",
        "--ignore-rules",
        "-C",
    ]
    .map(str::to_owned)
    .to_vec();
    args.push(worktree.to_string_lossy().into_owned());
    if let Some(model) = config["model"].as_str().filter(|model| !model.is_empty()) {
        args.extend(["-m".to_owned(), model.to_owned()]);
    }
    if let Some(effort) = config["effort"].as_str() {
        args.extend([
            "-c".to_owned(),
            format!("model_reasoning_effort={}", toml(&json!(effort))),
        ]);
    }
    if let Some(path) = config["system_prompt_path"].as_str() {
        args.extend([
            "-c".to_owned(),
            format!("model_instructions_file={}", toml(&json!(path))),
        ]);
    }
    if !crate::strings(&config["custom"]).is_empty() {
        args.extend([
            "-c".to_owned(),
            format!("mcp_servers.oa.command={}", toml(&json!(self_exe))),
            "-c".to_owned(),
            format!(
                "mcp_servers.oa.args={}",
                toml(&json!(["--serve", config_path]))
            ),
            "-c".to_owned(),
            "mcp_servers.oa.tool_timeout_sec=1800".to_owned(),
            "-c".to_owned(),
            format!("mcp_servers.oa.env_vars={}", toml(&json!(env_names))),
        ]);
    }
    args.extend([
        "-c".to_owned(),
        "web_search=\"disabled\"".to_owned(),
        "--dangerously-bypass-approvals-and-sandbox".to_owned(),
        "-".to_owned(),
    ]);
    args
}

/// Runs the briefed agent on Codex and writes the event log and summary.
pub async fn run(config: &Value, config_path: &str) -> Result<(), String> {
    let worktree = std::path::PathBuf::from(crate::text(config, "worktree")?);
    let worktree = worktree.canonicalize().unwrap_or(worktree);
    let prompt = std::fs::read_to_string(crate::text(config, "prompt_path")?)
        .map_err(|error| format!("reading the prompt: {error}"))?;
    let events_path = crate::text(config, "events_path")?;
    let summary_path = crate::text(config, "summary_path")?;
    let briefed: BTreeSet<String> = crate::strings(&config["briefed_files"])
        .into_iter()
        .collect();
    let timeout = Duration::from_secs(config["timeout_secs"].as_u64().unwrap_or(1200));
    let self_exe = std::env::current_exe()
        .map_err(|error| format!("this program's path: {error}"))?
        .to_string_lossy()
        .into_owned();
    let binary = config["codex_bin"].as_str().unwrap_or("codex");

    let mut child = tokio::process::Command::new(binary)
        .args(argv(
            config,
            config_path,
            &self_exe,
            &worktree,
            &forwarded_env(),
        ))
        .current_dir(&worktree)
        .env_remove("OPENAI_API_KEY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Codex didn't start: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("writing the prompt: {error}"))?;
        drop(stdin);
    }
    let stdout = child.stdout.take().ok_or("Codex has no stdout")?;
    let stderr = child.stderr.take().ok_or("Codex has no stderr")?;
    let stderr_task = tokio::spawn(async move {
        let mut text = String::new();
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            text.push_str(&line);
            text.push('\n');
        }
        text
    });

    let started = Instant::now();
    let mut observed = Observed::default();
    let mut events = String::new();
    let mut timed_out = false;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        match tokio::time::timeout(remaining, lines.next_line()).await {
            Err(_) => {
                timed_out = true;
                let _ = child.kill().await;
                break;
            }
            Ok(Ok(None)) | Ok(Err(_)) => break,
            Ok(Ok(Some(line))) => {
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                observed.see_in(&event, &briefed, &worktree);
                events.push_str(
                    &json!({"type": "codex", "t_ms": started.elapsed().as_millis(), "event": event})
                        .to_string(),
                );
                events.push('\n');
            }
        }
    }
    let status = child.wait().await.ok();
    let stderr_text = stderr_task.await.unwrap_or_default();
    let wall_ms = started.elapsed().as_millis();
    let mut error = observed.error.clone();
    if error.is_none() && status.is_some_and(|status| !status.success()) && !timed_out {
        let tail: String = stderr_text
            .chars()
            .rev()
            .take(600)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        error = Some(format!("Codex exited {status:?}: {}", tail.trim()));
    }

    std::fs::write(&events_path, events).map_err(|error| format!("{events_path}: {error}"))?;
    let finished = config["finish_path"]
        .as_str()
        .is_some_and(|path| Path::new(path).exists());
    let tally = config
        .pointer("/verify/log")
        .and_then(Value::as_str)
        .map(tally_from_log)
        .unwrap_or_default();
    let summary = json!({
        "engine": "codex",
        "model": config["model"].as_str().filter(|model| !model.is_empty()),
        "result": {
            "thread_id": observed.thread_id,
            "num_turns": observed.turns,
            "usage": observed.usage,
            "result": observed.last_message,
        },
        "error": error,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "tool_calls": observed.tool_calls,
        "files_read": [],
        "files_written": observed.files_written,
        "misses": observed.misses,
        "denied": [],
        "check_runs": observed.check_runs,
        "verify": tally.to_json(),
        "finished": finished,
    });
    std::fs::write(
        &summary_path,
        serde_json::to_vec_pretty(&summary).unwrap_or_default(),
    )
    .map_err(|error| format!("{summary_path}: {error}"))?;
    Ok(())
}

/// The names of this process's environment variables that the `oa` server
/// gets (all but Codex's own credential).
fn forwarded_env() -> Vec<String> {
    let mut names: Vec<String> = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter(|name| name != "OPENAI_API_KEY")
        .collect();
    names.sort();
    names
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

    fn briefed() -> BTreeSet<String> {
        ["crates/a/src/lib.rs".to_owned()].into_iter().collect()
    }

    #[test]
    fn argv_replaces_instructions_and_serves_checks() {
        let config = json!({
            "model": "gpt-x", "effort": "high", "system_prompt_path": "/b/system.md",
            "custom": ["verify", "finish"]
        });
        let args = argv(
            &config,
            "/b/agent.json",
            "/bin/briefed-agent",
            Path::new("/w"),
            &["AB_ROOT".to_owned(), "PATH".to_owned()],
        );
        assert_eq!(args[0], "exec");
        assert!(args.contains(&"--json".to_owned()));
        assert!(args.contains(&"model_instructions_file=\"/b/system.md\"".to_owned()));
        assert!(args.contains(&"model_reasoning_effort=\"high\"".to_owned()));
        assert!(args.contains(&"mcp_servers.oa.command=\"/bin/briefed-agent\"".to_owned()));
        assert!(args.contains(&"mcp_servers.oa.args=[\"--serve\",\"/b/agent.json\"]".to_owned()));
        assert!(args.contains(&"mcp_servers.oa.env_vars=[\"AB_ROOT\",\"PATH\"]".to_owned()));
        assert_eq!(
            args[args.len() - 2..],
            ["--dangerously-bypass-approvals-and-sandbox", "-"]
        );
    }

    #[test]
    fn argv_without_check_tools_has_no_server() {
        let args = argv(&json!({}), "/b/agent.json", "/bin/x", Path::new("/w"), &[]);
        assert!(!args.iter().any(|a| a.starts_with("mcp_servers")));
        assert!(!args.contains(&"-m".to_owned()));
    }

    #[test]
    fn events_become_tool_calls_misses_usage_and_errors() {
        let mut seen = Observed::default();
        let lines = [
            r#"{"type":"thread.started","thread_id":"t1"}"#,
            r#"{"type":"item.completed","item":{"id":"1","type":"command_execution","command":"cargo test","exit_code":0}}"#,
            r#"{"type":"item.completed","item":{"id":"2","type":"file_change","changes":[{"path":"/w/crates/a/src/lib.rs","kind":"update"},{"path":"/w/crates/a/src/new.rs","kind":"add"}]}}"#,
            r#"{"type":"item.completed","item":{"id":"3","type":"mcp_tool_call","server":"oa","tool":"verify","status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"4","type":"agent_message","text":"done"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":4,"output_tokens":3}}"#,
            r#"{"type":"turn.failed","error":{"message":"overloaded"}}"#,
        ];
        for line in lines {
            seen.see_in(
                &serde_json::from_str(line).unwrap(),
                &briefed(),
                Path::new("/w"),
            );
        }
        assert_eq!(seen.tool_calls["Bash"], 1);
        assert_eq!(seen.tool_calls["Edit"], 1);
        assert_eq!(seen.tool_calls["verify"], 1);
        assert_eq!(seen.check_runs, 1);
        assert_eq!(seen.files_written.len(), 2);
        assert_eq!(
            seen.misses,
            vec![json!({"path": "crates/a/src/new.rs", "via": "file_change"})]
        );
        assert_eq!(seen.usage["input_tokens"], 10);
        assert_eq!(seen.turns, 1);
        assert_eq!(seen.last_message.as_deref(), Some("done"));
        assert_eq!(seen.error.as_deref(), Some("overloaded"));
        assert_eq!(seen.thread_id.as_deref(), Some("t1"));
    }

    #[test]
    fn tally_reads_the_verify_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("verify.jsonl");
        std::fs::write(
            &log,
            "{\"status\":\"pass\"}\n{\"verdict\":{\"status\":\"test_failure\"}}\nnot json\n",
        )
        .unwrap();
        let tally = tally_from_log(log.to_str().unwrap());
        let json = tally.to_json();
        assert_eq!(json["calls"].as_u64(), Some(2));
    }
}
