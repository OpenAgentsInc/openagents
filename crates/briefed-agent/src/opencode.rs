//! The briefed agent on OpenCode (#11308): the same briefing, checks, and
//! summary as the Claude session in `main.rs`, run as
//! `opencode run --format json`. `engine: "gemini"` is the same run on
//! Vertex AI's Gemini through OpenCode's `google-vertex` provider.
//!
//! - **System prompt**: the briefing is the whole prompt of an OpenCode
//!   agent named `briefed`. No user or project config, `AGENTS.md`, or
//!   `CLAUDE.md` loads (an empty config home and
//!   `OPENCODE_DISABLE_PROJECT_CONFIG`), so the model sees only the briefing.
//! - **Tools**: OpenCode's own `read`, `edit`, `write`, `grep`, and `glob` for
//!   the config's `Read`, `Edit`, `Write`, `Grep`, and `Glob` (plus `bash`
//!   only when the config names `Bash`), and the briefing's check tools
//!   (`verify`, `finish`, ...) as the MCP server `oa`: this same binary in
//!   `--serve CONFIG.json` mode, as Codex gets them. Web, task, and todo
//!   tools are off; LSP, formatters, and snapshots are off.
//! - **Boundary**: OpenCode's `external_directory` permission is `deny`, so
//!   a file tool outside the worktree fails; `bash` is denied unless named.
//! - **Login**: the provider's credential from the environment
//!   (`OPENROUTER_API_KEY`, or Google's application default credentials for
//!   `google-vertex/...`, where `GOOGLE_CLOUD_PROJECT` defaults to
//!   `openagentsgemini` and `VERTEX_LOCATION` to `global`). OpenCode's own
//!   data (sessions, logins) lives in a scratch home, so nothing is kept.
//! - **Misses**: like the Claude session's: a `read` of a file the briefing
//!   did not list, and an edit of an existing file neither listed nor read.
//!
//! Events are kept one JSON line each as OpenCode printed them; the summary
//! has the shape `main.rs` writes, plus `"engine"` and `"model"`. OpenCode
//! reports each step's cost at the model's list price, so `total_cost_usd`
//! is that sum.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

/// The model `engine: "gemini"` runs without a `model` in the config.
pub const GEMINI_MODEL: &str = "google-vertex/gemini-3.1-pro-preview";

/// The model `engine: "opencode"` runs without a `model` in the config: an
/// open model Vertex serves, so the run needs no third-party credit.
pub const OPENCODE_MODEL: &str = "google-vertex/zai-org/glm-5.2-maas";

/// OpenCode's tool names for the config's built-in tool names.
const TOOL_NAMES: [(&str, &str); 6] = [
    ("Read", "read"),
    ("Edit", "edit"),
    ("Write", "write"),
    ("Grep", "grep"),
    ("Glob", "glob"),
    ("Bash", "bash"),
];

/// OpenCode's other tools, all off.
const OFF_TOOLS: [&str; 12] = [
    "list",
    "patch",
    "apply_patch",
    "multiedit",
    "webfetch",
    "websearch",
    "codesearch",
    "task",
    "todowrite",
    "todoread",
    "skill",
    "question",
];

/// The model this run uses: the config's, else the engine's default.
pub fn model_for(config: &Value, engine: &str) -> String {
    match config["model"].as_str().filter(|model| model.contains('/')) {
        Some(model) => model.to_owned(),
        None if engine == "gemini" => GEMINI_MODEL.to_owned(),
        None => OPENCODE_MODEL.to_owned(),
    }
}

/// The OpenCode config for `config`: one primary agent, `briefed`, whose
/// prompt is the system prompt file, with the config's tools and the `oa`
/// check server.
pub fn opencode_config(config: &Value, config_path: &str, self_exe: &str, model: &str) -> Value {
    let named = crate::strings(&config["tools"]);
    let named: Vec<String> = if named.is_empty() {
        crate::DEFAULT_TOOLS.iter().map(|&t| t.to_owned()).collect()
    } else {
        named
    };
    let mut tools = serde_json::Map::new();
    for (ours, theirs) in TOOL_NAMES {
        tools.insert(theirs.to_owned(), json!(named.iter().any(|n| n == ours)));
    }
    for tool in OFF_TOOLS {
        tools.insert(tool.to_owned(), json!(false));
    }
    let bash = if named.iter().any(|n| n == "Bash") {
        "allow"
    } else {
        "deny"
    };
    // No `"*"` rule: OpenCode puts the `tools` switches first and the last
    // matching rule wins, so a wildcard allow would re-allow `bash`.
    let permission = json!({
        "read": "allow",
        "edit": "allow",
        "glob": "allow",
        "grep": "allow",
        "bash": bash,
        "webfetch": "deny",
        "websearch": "deny",
        "task": "deny",
        "external_directory": "deny",
        "doom_loop": "allow",
    });
    let prompt = config["system_prompt_path"]
        .as_str()
        .map(|path| format!("{{file:{path}}}"))
        .unwrap_or_default();
    let mut agent = json!({
        "mode": "primary",
        "description": "The briefed agent: one issue, the briefing's files, verify.",
        "prompt": prompt,
        "model": model,
        "tools": tools,
        "permission": permission,
    });
    if let Some(turns) = config["max_turns"].as_u64() {
        agent["steps"] = json!(turns);
    }
    let mut out = json!({
        "$schema": "https://opencode.ai/config.json",
        "model": model,
        "small_model": model,
        "default_agent": "briefed",
        "autoupdate": false,
        "share": "disabled",
        "snapshot": false,
        "lsp": false,
        "formatter": false,
        "instructions": [],
        "tools": tools,
        "permission": permission,
        "agent": {"briefed": agent},
    });
    if !crate::strings(&config["custom"]).is_empty() {
        out["mcp"] = json!({
            "oa": {
                "type": "local",
                "command": [self_exe, "--serve", config_path],
                "enabled": true,
                // verify compiles and runs tests: minutes, not OpenCode's 5 s.
                "timeout": 1_800_000,
            }
        });
    }
    out
}

/// The environment OpenCode runs with: scratch config and data homes, no
/// project config, and Vertex's project and location when the model is on
/// Vertex and the environment names none.
pub fn env_for(
    model: &str,
    scratch: &Path,
    current: &dyn Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let path = |name: &str| scratch.join(name).to_string_lossy().into_owned();
    let mut env = vec![
        ("XDG_CONFIG_HOME".to_owned(), path("config")),
        ("XDG_DATA_HOME".to_owned(), path("data")),
        ("XDG_STATE_HOME".to_owned(), path("state")),
        ("OPENCODE_CONFIG".to_owned(), path("opencode.json")),
        ("OPENCODE_DISABLE_PROJECT_CONFIG".to_owned(), "1".to_owned()),
        ("OPENCODE_DISABLE_CLAUDE_CODE".to_owned(), "1".to_owned()),
        ("OPENCODE_DISABLE_AUTOUPDATE".to_owned(), "1".to_owned()),
        ("OPENCODE_DISABLE_LSP_DOWNLOAD".to_owned(), "1".to_owned()),
        ("OPENCODE_DISABLE_SHARE".to_owned(), "1".to_owned()),
        (
            "OPENCODE_DISABLE_DEFAULT_PLUGINS".to_owned(),
            "1".to_owned(),
        ),
    ];
    if model.starts_with("google-vertex") {
        let project = ["GOOGLE_CLOUD_PROJECT", "GCP_PROJECT", "GCLOUD_PROJECT"]
            .iter()
            .any(|name| current(name).is_some_and(|value| !value.is_empty()));
        if !project {
            env.push((
                "GOOGLE_CLOUD_PROJECT".to_owned(),
                "openagentsgemini".to_owned(),
            ));
        }
        let location = ["VERTEX_LOCATION", "GOOGLE_CLOUD_LOCATION"]
            .iter()
            .any(|name| current(name).is_some_and(|value| !value.is_empty()));
        if !location {
            env.push(("VERTEX_LOCATION".to_owned(), "global".to_owned()));
        }
    }
    env
}

/// What one run of `opencode run --format json` observed.
#[derive(Default, Debug, PartialEq)]
pub struct Observed {
    pub tool_calls: BTreeMap<String, u64>,
    pub files_read: BTreeSet<String>,
    pub files_written: BTreeSet<String>,
    pub misses: Vec<Value>,
    pub check_runs: u64,
    pub steps: u64,
    pub usage: BTreeMap<String, u64>,
    pub cost: Option<f64>,
    pub last_message: Option<String>,
    pub session_id: Option<String>,
    pub error: Option<String>,
}

impl Observed {
    /// Reads one event line. `tracked` holds the worktree's files at the
    /// base, so writing a file that is not there is a new file, not a miss.
    pub fn see_in(
        &mut self,
        event: &Value,
        briefed: &BTreeSet<String>,
        tracked: &BTreeSet<String>,
        worktree: &Path,
    ) {
        if self.session_id.is_none() {
            self.session_id = event["sessionID"].as_str().map(str::to_owned);
        }
        let part = &event["part"];
        match event["type"].as_str() {
            Some("text") => {
                if let Some(text) = part["text"].as_str().filter(|t| !t.trim().is_empty()) {
                    self.last_message = Some(text.to_owned());
                }
            }
            Some("step_finish") => {
                self.steps += 1;
                if let Some(cost) = part["cost"].as_f64() {
                    *self.cost.get_or_insert(0.0) += cost;
                }
                for (key, pointer) in [
                    ("input_tokens", "/tokens/input"),
                    ("output_tokens", "/tokens/output"),
                    ("reasoning_tokens", "/tokens/reasoning"),
                    ("cache_read_input_tokens", "/tokens/cache/read"),
                    ("cache_creation_input_tokens", "/tokens/cache/write"),
                ] {
                    let n = part.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
                    *self.usage.entry(key.to_owned()).or_insert(0) += n;
                }
            }
            Some("error") => {
                let error = &event["error"];
                let message = error
                    .pointer("/data/message")
                    .and_then(Value::as_str)
                    .or_else(|| error["message"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| error.to_string());
                self.error.get_or_insert(message);
            }
            Some("tool_use") => self.tool(part, briefed, tracked, worktree),
            _ => {}
        }
    }

    fn tool(
        &mut self,
        part: &Value,
        briefed: &BTreeSet<String>,
        tracked: &BTreeSet<String>,
        worktree: &Path,
    ) {
        let raw = part["tool"].as_str().unwrap_or("?");
        let name = match raw {
            "read" => "Read",
            "edit" | "multiedit" => "Edit",
            "write" => "Write",
            "grep" => "Grep",
            "glob" => "Glob",
            "bash" => "Bash",
            "list" => "List",
            other => other.strip_prefix("oa_").unwrap_or(other),
        };
        *self.tool_calls.entry(name.to_owned()).or_insert(0) += 1;
        if name == "verify" || name == "finish" {
            self.check_runs += 1;
        }
        let input = &part["state"]["input"];
        let Some(path) = input["filePath"]
            .as_str()
            .or_else(|| input["file_path"].as_str())
            .or_else(|| input["path"].as_str())
        else {
            return;
        };
        let Some(relative) = relative(worktree, path) else {
            return;
        };
        match name {
            "Read" => {
                if !briefed.contains(&relative) && self.files_read.insert(relative.clone()) {
                    self.misses.push(json!({"file": relative, "tool": "Read"}));
                } else {
                    self.files_read.insert(relative);
                }
            }
            "Edit" | "Write" => {
                let seen = self.files_read.contains(&relative);
                let created = name == "Write" && !tracked.contains(&relative);
                if !briefed.contains(&relative)
                    && !seen
                    && !created
                    && self.files_written.insert(relative.clone())
                {
                    self.misses.push(json!({"file": relative, "tool": name}));
                } else {
                    self.files_written.insert(relative);
                }
            }
            _ => {}
        }
    }
}

/// `path` relative to `worktree`, or `None` when it is outside.
fn relative(worktree: &Path, path: &str) -> Option<String> {
    let resolved = crate::resolve(worktree, path);
    let relative = resolved.strip_prefix(worktree).ok()?;
    Some(relative.to_string_lossy().into_owned())
}

/// The worktree's tracked files (`git ls-files`), or none when it is not a
/// git checkout.
fn tracked_files(worktree: &Path) -> BTreeSet<String> {
    std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["ls-files", "-z"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            out.stdout
                .split(|&b| b == 0)
                .filter(|name| !name.is_empty())
                .map(|name| String::from_utf8_lossy(name).into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The arguments of `opencode run` for `model`: JSON events, the `briefed`
/// agent, the worktree, and the effort as OpenCode's model variant. The
/// prompt goes on stdin.
pub fn argv(config: &Value, model: &str, worktree: &Path) -> Vec<String> {
    let mut args: Vec<String> = ["run", "--format", "json", "--agent", "briefed", "-m"]
        .map(str::to_owned)
        .to_vec();
    args.push(model.to_owned());
    args.push("--dir".to_owned());
    args.push(worktree.to_string_lossy().into_owned());
    if let Some(effort) = config["effort"].as_str().filter(|e| !e.is_empty()) {
        args.extend(["--variant".to_owned(), effort.to_owned()]);
    }
    args
}

/// Runs the briefed agent on OpenCode and writes the event log and summary.
pub async fn run(config: &Value, config_path: &str, engine: &str) -> Result<(), String> {
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
    let self_exe = std::env::current_exe()
        .map_err(|error| format!("this program's path: {error}"))?
        .to_string_lossy()
        .into_owned();
    let binary = config["opencode_bin"].as_str().unwrap_or("opencode");
    let model = model_for(config, engine);
    let config_path = Path::new(config_path).canonicalize().map_or_else(
        |_| config_path.to_owned(),
        |p| p.to_string_lossy().into_owned(),
    );

    let scratch = tempfile::tempdir().map_err(|error| format!("a scratch home: {error}"))?;
    for dir in ["config", "data", "state"] {
        std::fs::create_dir_all(scratch.path().join(dir)).map_err(|error| error.to_string())?;
    }
    let opencode = opencode_config(config, &config_path, &self_exe, &model);
    std::fs::write(
        scratch.path().join("opencode.json"),
        serde_json::to_vec_pretty(&opencode).unwrap_or_default(),
    )
    .map_err(|error| format!("writing OpenCode's config: {error}"))?;
    let env = env_for(&model, scratch.path(), &|name| std::env::var(name).ok());
    let tracked = tracked_files(&worktree);

    let mut child = tokio::process::Command::new(binary)
        .args(argv(config, &model, &worktree))
        .current_dir(&worktree)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("OpenCode didn't start: {error}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|error| format!("writing the prompt: {error}"))?;
        drop(stdin);
    }
    let stdout = child.stdout.take().ok_or("OpenCode has no stdout")?;
    let stderr = child.stderr.take().ok_or("OpenCode has no stderr")?;
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
    let mut finished = false;
    let finish_path = config["finish_path"].as_str().map(PathBuf::from);
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
                observed.see_in(&event, &briefed, &tracked, &worktree);
                events.push_str(
                    &json!({"type": "opencode", "t_ms": started.elapsed().as_millis(), "event": event})
                        .to_string(),
                );
                events.push('\n');
                // `finish` passed: end the run without another step.
                if finish_path.as_deref().is_some_and(Path::exists) {
                    finished = true;
                    let _ = child.kill().await;
                    break;
                }
            }
        }
    }
    let status = child.wait().await.ok();
    let stderr_text = stderr_task.await.unwrap_or_default();
    let wall_ms = started.elapsed().as_millis();
    let mut error = observed.error.clone();
    if error.is_none() && !timed_out && !finished && status.is_some_and(|status| !status.success())
    {
        let tail: String = stderr_text
            .chars()
            .rev()
            .take(600)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        error = Some(format!("OpenCode exited {status:?}: {}", tail.trim()));
    }

    std::fs::write(&events_path, events).map_err(|error| format!("{events_path}: {error}"))?;
    let finished = finished || finish_path.as_deref().is_some_and(Path::exists);
    let tally = config
        .pointer("/verify/log")
        .and_then(Value::as_str)
        .map(crate::codex::tally_from_log)
        .unwrap_or_default();
    let summary = json!({
        "engine": engine,
        "model": model,
        "result": {
            "session_id": observed.session_id,
            "num_turns": observed.steps,
            "usage": observed.usage,
            "total_cost_usd": observed.cost,
            "is_error": error.is_some(),
            "result": observed.last_message,
        },
        "error": error,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "tool_calls": observed.tool_calls,
        "files_read": observed.files_read,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|&s| s.to_owned()).collect()
    }

    #[test]
    fn the_config_is_the_briefing_the_tools_and_the_check_server() {
        let config = json!({
            "system_prompt_path": "/b/system.md",
            "tools": ["Read", "Edit", "Write", "Grep", "Glob"],
            "custom": ["verify"],
        });
        let out = opencode_config(&config, "/b/agent.json", "/bin/briefed-agent", "p/m");
        let agent = &out["agent"]["briefed"];
        assert_eq!(agent["prompt"], "{file:/b/system.md}");
        assert_eq!(agent["model"], "p/m");
        assert_eq!(out["default_agent"], "briefed");
        for tool in ["read", "edit", "write", "grep", "glob"] {
            assert_eq!(agent["tools"][tool], true, "{tool}");
        }
        for tool in ["bash", "webfetch", "task", "apply_patch"] {
            assert_eq!(agent["tools"][tool], false, "{tool}");
        }
        assert_eq!(agent["permission"]["bash"], "deny");
        assert_eq!(agent["permission"]["external_directory"], "deny");
        assert!(agent["permission"].get("*").is_none());
        assert_eq!(
            out["mcp"]["oa"]["command"],
            json!(["/bin/briefed-agent", "--serve", "/b/agent.json"])
        );
        assert_eq!(out["instructions"], json!([]));
        assert_eq!(out["snapshot"], false);
    }

    #[test]
    fn bash_is_on_only_when_named_and_no_checks_means_no_server() {
        let config = json!({"tools": ["Read", "Bash"]});
        let out = opencode_config(&config, "/a.json", "/x", "p/m");
        assert_eq!(out["agent"]["briefed"]["tools"]["bash"], true);
        assert_eq!(out["agent"]["briefed"]["tools"]["edit"], false);
        assert_eq!(out["permission"]["bash"], "allow");
        assert!(out.get("mcp").is_none());
    }

    #[test]
    fn engines_pick_their_default_models() {
        assert_eq!(model_for(&json!({}), "gemini"), GEMINI_MODEL);
        assert_eq!(
            model_for(&json!({"model": "claude-opus-5-5"}), "opencode"),
            OPENCODE_MODEL
        );
        assert_eq!(
            model_for(&json!({"model": "openrouter/x/y"}), "gemini"),
            "openrouter/x/y"
        );
    }

    #[test]
    fn vertex_gets_a_project_and_location_unless_set() {
        let scratch = Path::new("/s");
        let none = |_: &str| None;
        let env = env_for(GEMINI_MODEL, scratch, &none);
        let get = |env: &[(String, String)], key: &str| {
            env.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        };
        assert_eq!(
            get(&env, "GOOGLE_CLOUD_PROJECT").as_deref(),
            Some("openagentsgemini")
        );
        assert_eq!(get(&env, "VERTEX_LOCATION").as_deref(), Some("global"));
        assert_eq!(
            get(&env, "OPENCODE_DISABLE_PROJECT_CONFIG").as_deref(),
            Some("1")
        );
        assert_eq!(get(&env, "XDG_CONFIG_HOME").as_deref(), Some("/s/config"));
        let set_project = |name: &str| (name == "GOOGLE_CLOUD_PROJECT").then(|| "p".to_owned());
        let env = env_for(GEMINI_MODEL, scratch, &set_project);
        assert_eq!(get(&env, "GOOGLE_CLOUD_PROJECT"), None);
        let env = env_for("openrouter/a/b", scratch, &none);
        assert_eq!(get(&env, "VERTEX_LOCATION"), None);
    }

    #[test]
    fn events_become_tool_calls_misses_usage_cost_and_errors() {
        let worktree = Path::new("/w");
        let briefed = set(&["crates/a/src/lib.rs"]);
        let tracked = set(&["crates/a/src/lib.rs", "crates/a/src/other.rs"]);
        let lines = [
            r#"{"type":"step_start","sessionID":"ses_1","part":{}}"#,
            r#"{"type":"tool_use","sessionID":"ses_1","part":{"tool":"read","state":{"status":"completed","input":{"filePath":"/w/crates/a/src/lib.rs"}}}}"#,
            r#"{"type":"tool_use","sessionID":"ses_1","part":{"tool":"read","state":{"status":"completed","input":{"filePath":"/w/crates/a/src/other.rs"}}}}"#,
            r#"{"type":"tool_use","sessionID":"ses_1","part":{"tool":"edit","state":{"status":"completed","input":{"filePath":"/w/crates/a/src/other.rs"}}}}"#,
            r#"{"type":"tool_use","sessionID":"ses_1","part":{"tool":"write","state":{"status":"completed","input":{"filePath":"/w/crates/a/src/new.rs"}}}}"#,
            r#"{"type":"tool_use","sessionID":"ses_1","part":{"tool":"oa_verify","state":{"status":"completed","input":{"tests":["x"]}}}}"#,
            r#"{"type":"step_finish","sessionID":"ses_1","part":{"cost":0.25,"tokens":{"input":100,"output":20,"reasoning":5,"cache":{"read":40,"write":0}}}}"#,
            r#"{"type":"text","sessionID":"ses_1","part":{"text":"done"}}"#,
            r#"{"type":"step_finish","sessionID":"ses_1","part":{"cost":0.5,"tokens":{"input":10,"output":2,"reasoning":0,"cache":{"read":0,"write":0}}}}"#,
            r#"{"type":"error","sessionID":"ses_1","error":{"name":"APIError","data":{"message":"Resource exhausted"}}}"#,
        ];
        let mut seen = Observed::default();
        for line in lines {
            seen.see_in(
                &serde_json::from_str(line).unwrap(),
                &briefed,
                &tracked,
                worktree,
            );
        }
        assert_eq!(seen.session_id.as_deref(), Some("ses_1"));
        assert_eq!(seen.tool_calls["Read"], 2);
        assert_eq!(seen.tool_calls["Edit"], 1);
        assert_eq!(seen.tool_calls["Write"], 1);
        assert_eq!(seen.tool_calls["verify"], 1);
        assert_eq!(seen.check_runs, 1);
        // Reading an unlisted file is a miss; editing it after is not, and
        // writing a new file is not.
        assert_eq!(
            seen.misses,
            vec![json!({"file": "crates/a/src/other.rs", "tool": "Read"})]
        );
        assert_eq!(
            seen.files_written,
            set(&["crates/a/src/other.rs", "crates/a/src/new.rs"])
        );
        assert_eq!(seen.steps, 2);
        assert_eq!(seen.cost, Some(0.75));
        assert_eq!(seen.usage["input_tokens"], 110);
        assert_eq!(seen.usage["cache_read_input_tokens"], 40);
        assert_eq!(seen.usage["reasoning_tokens"], 5);
        assert_eq!(seen.last_message.as_deref(), Some("done"));
        assert_eq!(seen.error.as_deref(), Some("Resource exhausted"));
    }

    #[test]
    fn argv_has_the_agent_model_dir_and_variant() {
        let args = argv(&json!({"effort": "high"}), "p/m", Path::new("/w"));
        assert_eq!(
            args,
            [
                "run",
                "--format",
                "json",
                "--agent",
                "briefed",
                "-m",
                "p/m",
                "--dir",
                "/w",
                "--variant",
                "high"
            ]
        );
        let args = argv(&json!({}), "p/m", Path::new("/w"));
        assert!(!args.contains(&"--variant".to_owned()));
    }
}
