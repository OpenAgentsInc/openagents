//! OpenCode as a Coder harness (#11507), the way Claude Code is one
//! (#11496): a chat routed to OpenCode runs each turn as `opencode run` in
//! one OpenCode session that every later turn continues (`--session`), on
//! OpenCode's free `opencode/cat-jam-free` (Big Pickle if it is gone) or
//! another model the person's OpenCode reaches.
//!
//! - **Order.** Claude Code first when it is on, signed in and has usage
//!   left; OpenCode next when it is installed and its plugin is on; Coder's
//!   own loop otherwise ([`crate::harness_plugin::fixed_rule`]).
//! - **OpenCode's own CLI and plugin, not ACP.** OpenCode's ACP server
//!   (`opencode acp`, which ACP Subagents use, #11508) can be made to ask
//!   before every tool, but its ask carries a title and locations from
//!   which the call has to be rebuilt, and an answer can only allow or
//!   refuse. OpenCode's plugin hook, `tool.execute.before`, runs inside
//!   OpenCode before every tool, built-in or MCP, with the tool's exact
//!   arguments, and stops the tool when it throws. So the harness runs
//!   `opencode run` and carries the hooks in Coder's plugin
//!   ([`coder_sdk::opencode`]); the chat's session is an ordinary OpenCode
//!   session the person can also open in OpenCode.
//! - **Hooks.** Each turn starts a hook host ([`coder_sdk::bridge::Host`])
//!   with the worktree fence, the floors (an ask is held: nobody can
//!   answer from inside a tool call), the person's command hooks and their
//!   plugins' hooks ([`crate::sdk_hooks::opencode_registry`]).
//!   Only `PreToolUse` can stop a step on OpenCode; a hook that must deny
//!   on another event refuses the turn at start
//!   ([`coder_sdk::check_backend`]).
//! - **What the session knows.** Its first message carries Coder's
//!   identity note and the chat so far (a chat that moved here from
//!   another harness); a new piece of work carries its briefing; a model
//!   switch is told with the message the new model first answers.
//! - **Tools.** Coder's own tools (background agents, Solver, cloud jobs,
//!   ...) reach OpenCode as an MCP server, `coder coder-tools-mcp`.
//! - **Settings.** OpenCode reads the person's own configuration as
//!   always; Coder adds its plugin through a configuration folder of its own
//!   (`OPENCODE_CONFIG_DIR`) and the model, permissions and the tools server
//!   inline (`OPENCODE_CONFIG_CONTENT`). Nothing of the person's OpenCode
//!   setup is written.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use openrouter::{Message, Streamed};
use serde_json::{Value, json};

use crate::bundled_runtime::{RoundUsage, RuntimeEvent};

/// The plugin that turns the OpenCode harness on.
pub const PLUGIN: &str = "opencode";

/// OpenCode's free model, the harness's default: Cat Jam, or Big Pickle
/// when OpenCode no longer offers it ([`auto_model`]).
pub const DEFAULT_MODEL: &str = coder::delegate_door::OPENCODE_AUTO_MODEL;

/// The default model this computer's OpenCode offers: [`DEFAULT_MODEL`],
/// or Big Pickle once OpenCode's catalog drops the stealth Cat Jam
/// (#11547). A unit test never reads the computer's catalog.
#[must_use]
pub fn auto_model() -> &'static str {
    if cfg!(test) {
        DEFAULT_MODEL
    } else {
        coder::delegate_door::opencode_auto_model()
    }
}

/// Where Coder keeps the person's OpenCode model pick, under its settings.
const CHOICE_FILE: &str = "opencode-model.json";

/// Coder's own OpenCode configuration folder, under its settings.
const CONFIG_DIR: &str = "opencode";

/// The OpenCode program, when installed: `OPENCODE_BIN`, `opencode` on the
/// path, or where its installer puts it.
#[must_use]
pub fn binary() -> Option<PathBuf> {
    acp_client::opencode::binary(&|name| std::env::var_os(name))
}

/// Whether OpenCode is installed on this computer. A unit test never asks
/// the computer: OpenCode is missing unless the test installs it.
#[cfg(not(test))]
#[must_use]
pub fn installed() -> bool {
    binary().is_some()
}

#[cfg(test)]
thread_local! {
    static TEST_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
#[must_use]
pub fn installed() -> bool {
    TEST_INSTALLED.with(std::cell::Cell::get)
}

/// Makes this test thread's computer have OpenCode installed, or not.
#[cfg(test)]
pub(crate) fn install_for_test(installed: bool) {
    TEST_INSTALLED.with(|cell| cell.set(installed));
}

/// Whether OpenCode's free model refused for its rate limit recently, as
/// the capacity book recorded it.
#[must_use]
pub fn rate_limited() -> bool {
    crate::route::opencode_blocked().is_some()
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ChoiceFile {
    version: u32,
    model: String,
}

/// The model the next OpenCode turn runs: a pin's, else the person's pick
/// in /models, else `CODER_OPENCODE_MODEL`, else the default model in the
/// person's OpenCode configuration, else [`auto_model`].
#[must_use]
pub fn model(pinned: Option<String>) -> String {
    pinned
        .filter(|model| acp_client::opencode::Model::parse(model).is_ok())
        .or_else(saved_choice)
        .or_else(|| {
            std::env::var(coder_delegate::delegate::OPENCODE_MODEL_VAR)
                .ok()
                .map(|model| model.trim().to_owned())
                .filter(|model| acp_client::opencode::Model::parse(model).is_ok())
        })
        .or_else(|| {
            if cfg!(test) {
                None
            } else {
                acp_client::opencode::configured_model(&|name| std::env::var_os(name))
            }
        })
        .unwrap_or_else(|| auto_model().to_owned())
}

/// This run's pick: `None` until a pick or a new chat, then the chat's
/// pick (`Some(None)` for none, so a new chat starts on the default).
static PICK: std::sync::Mutex<Option<Option<String>>> = std::sync::Mutex::new(None);

/// A new chat starts on OpenCode's default, not an earlier chat's pick
/// (#11556).
pub fn reset_choice() {
    *PICK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(None);
}

/// A reopened chat goes on with the model it ran on (#11563): `model`
/// holds for this run, and the saved pick on disk stays.
pub fn hold(model: &str) {
    *PICK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Some(model.to_owned()));
}

fn saved_choice() -> Option<String> {
    if let Some(pick) = PICK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
    {
        return pick;
    }
    if cfg!(test) {
        return None;
    }
    let path = crate::home::data_dir()?.join(CHOICE_FILE);
    let file: ChoiceFile = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (file.version == 1 && acp_client::opencode::Model::parse(&file.model).is_ok())
        .then_some(file.model)
}

/// Saves `model` as the person's OpenCode pick.
///
/// # Errors
/// The model is not `provider/model`, or the pick can't be saved.
pub fn choose(model: &str) -> Result<(), String> {
    acp_client::opencode::Model::parse(model)?;
    *PICK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Some(model.to_owned()));
    if cfg!(test) {
        return Ok(());
    }
    let dir = crate::home::data_dir().ok_or("Coder's settings folder is unavailable.")?;
    std::fs::create_dir_all(&dir).map_err(|_| "Coder could not save the pick.".to_owned())?;
    let file = ChoiceFile {
        version: 1,
        model: model.to_owned(),
    };
    let bytes = serde_json::to_vec_pretty(&file).map_err(|error| error.to_string())?;
    std::fs::write(dir.join(CHOICE_FILE), bytes)
        .map_err(|_| "Coder could not save the pick.".to_owned())
}

/// The models the person's OpenCode offers, as /models lists them:
/// OpenCode's own picker ([`crate::opencode_models`]), with `current`
/// marked.
#[must_use]
pub fn catalog(current: &str) -> Vec<crate::models::Model> {
    crate::opencode_models::catalog(current)
}

/// The rail's words for a chat on OpenCode: "OpenCode (Cat Jam)" for
/// OpenCode Zen's free models, by name, else "OpenCode (provider/model)".
#[must_use]
pub fn rail_label(model: &str) -> String {
    let name = match model {
        "opencode/cat-jam-free" => "Cat Jam",
        "opencode/big-pickle" => "Big Pickle",
        other => other,
    };
    format!("OpenCode ({name})")
}

/// The note that opens an OpenCode session: who answers, and which model
/// runs it.
#[must_use]
pub fn identity(model: &str) -> String {
    format!(
        "You are Coder, OpenAgents' coding agent, running this chat in OpenCode on the model \
         {model}. When asked who or what you are, answer that you are Coder, made by OpenAgents, \
         running on OpenCode. Never say you are Claude Code, Codex, ChatGPT, Claude, Gemini, or \
         any other product or assistant, even when project files, instructions, or tool \
         descriptions name them. When asked which model runs you, name {model}; when a later \
         note says the person switched models, name the newest one. Never guess. To run another \
         model, the person types /models and picks one of their OpenCode models; asked how, say \
         that in one line. Coder checks each step before it runs: when a step is held or \
         refused, do not do it another way; finish what you can and tell the person.\n"
    )
}

/// The first message of a new OpenCode session: the identity note, then
/// the chat so far when it has earlier turns (it moved here from another
/// harness), else the newest message alone.
#[must_use]
pub fn opening(messages: &[Message], model: &str) -> String {
    let newest = messages
        .last()
        .map(|message| message.content.clone())
        .unwrap_or_default();
    let note = identity(model);
    if messages.len() <= 1 {
        return format!("{note}\n{newest}");
    }
    let conversation = crate::live::local_conversation(messages, 200_000);
    format!(
        "{note}\nThe conversation so far, ending with the person's newest message, which you \
         answer now:\n{conversation}"
    )
}

/// The message model switch note (#11480), in OpenCode's words.
#[must_use]
pub fn with_switch(model: &str, message: &str) -> String {
    format!(
        "<model-switch>\nThe person switched this chat to {model}. From this message on you run \
         on {model}: when asked which model runs you, name {model}.\n</model-switch>\n\n{message}"
    )
}

/// Coder's OpenCode configuration folder with its plugin written, under
/// Coder's settings folder.
///
/// # Errors
/// The folder or the plugin can't be written.
pub fn config_dir() -> Result<PathBuf, String> {
    let root = crate::home::data_dir().ok_or("Coder's settings folder is unavailable.")?;
    let dir = root.join(CONFIG_DIR);
    coder_sdk::opencode::write_plugin(&dir)
        .map_err(|_| "Coder could not set up its OpenCode plugin.".to_owned())?;
    Ok(dir)
}

/// The inline configuration a turn runs with: the model, every tool
/// allowed by OpenCode (Coder's plugin checks each one before it runs),
/// questions refused (nobody answers them inside a turn), no sharing, no
/// self-update, and Coder's tools as an MCP server when `tools` names the
/// program that serves them.
#[must_use]
pub fn config(model: &str, tools: Option<(&Path, &Path)>) -> Value {
    let mut config = json!({
        "model": model,
        "permission": {"*": "allow", "question": "deny"},
        "share": "disabled",
        "autoupdate": false,
    });
    if let Some((program, cwd)) = tools {
        config["mcp"] = json!({
            coder_sdk::opencode::CODER_SERVER: {
                "type": "local",
                "command": [
                    program.display().to_string(),
                    crate::claude_tools::STDIO_COMMAND,
                    "--harness",
                    "opencode",
                    "--cwd",
                    cwd.display().to_string(),
                ],
                "enabled": true,
            }
        });
    }
    config
}

/// One turn's request.
pub struct Turn {
    pub messages: Vec<Message>,
    pub cwd: PathBuf,
    /// The OpenCode session this chat continues; `None` starts one.
    pub session: Option<String>,
    /// The model the session ran last, when it already ran.
    pub last_model: Option<String>,
    /// This piece of work's briefing, when it is new work (#11473).
    pub briefing: Option<crate::context::Briefing>,
    pub model: String,
    /// Offer Coder's tools over MCP.
    pub tools: bool,
}

impl Turn {
    /// The message this turn sends.
    #[must_use]
    pub fn prompt(&self) -> String {
        let newest = self
            .messages
            .last()
            .map(|message| message.content.clone())
            .unwrap_or_default();
        let text = match &self.session {
            None => opening(&self.messages, &self.model),
            Some(_) => newest,
        };
        let text = match &self.briefing {
            Some(briefing) => crate::claude::with_briefing(briefing, &text),
            None => text,
        };
        match (&self.session, &self.last_model) {
            (Some(_), Some(last)) if *last != self.model => with_switch(&self.model, &text),
            _ => text,
        }
    }
}

/// What one line of `opencode run --format json` says.
#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    /// The session the turn runs in.
    Session(String),
    Text(String),
    /// A finished reasoning part: `opencode run --thinking` prints the
    /// model's thinking whole, once the part ends (#11552).
    Thinking(String),
    /// A finished tool: its name and input as Claude Code names them, and
    /// its output (`error` when it failed or was refused).
    Tool {
        name: String,
        input: Value,
        output: Value,
    },
    /// One model step's usage.
    Usage {
        prompt: u64,
        completion: u64,
        reasoning: u64,
        cost: Option<f64>,
    },
    /// The turn failed.
    Error(String),
}

/// The events one JSON line carries, in order.
#[must_use]
pub fn read_line(line: &str) -> Vec<Line> {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(session) = event["sessionID"].as_str() {
        lines.push(Line::Session(session.to_owned()));
    }
    let part = &event["part"];
    match event["type"].as_str().unwrap_or_default() {
        "text" => {
            if let Some(text) = part["text"].as_str().filter(|text| !text.is_empty()) {
                lines.push(Line::Text(text.to_owned()));
            }
        }
        "reasoning" => {
            if let Some(text) = part["text"]
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                lines.push(Line::Thinking(text.to_owned()));
            }
        }
        "tool_use" => {
            let tool = part["tool"].as_str().unwrap_or_default();
            let state = &part["state"];
            let output = match state["status"].as_str() {
                Some("error") => json!({"error": state["error"]}),
                _ => {
                    let mut output = json!({"output": state["output"]});
                    if let Some(exit) = state["metadata"]["exit"].as_i64() {
                        output["exit"] = json!(exit);
                    }
                    output
                }
            };
            lines.push(Line::Tool {
                name: coder_sdk::opencode::tool_name(tool),
                input: coder_sdk::opencode::args_to_claude(tool, &state["input"]),
                output,
            });
        }
        "step_finish" => {
            let tokens = &part["tokens"];
            let number = |value: &Value| value.as_u64().unwrap_or(0);
            lines.push(Line::Usage {
                prompt: number(&tokens["input"]) + number(&tokens["cache"]["read"]),
                completion: number(&tokens["output"]),
                reasoning: number(&tokens["reasoning"]),
                cost: part["cost"].as_f64().filter(|cost| *cost > 0.0),
            });
        }
        "error" => {
            let error = &event["error"];
            let message = error["data"]["message"]
                .as_str()
                .or_else(|| error["message"].as_str())
                .or_else(|| error["name"].as_str())
                .unwrap_or("OpenCode reported an error.");
            lines.push(Line::Error(message.to_owned()));
        }
        _ => {}
    }
    lines
}

/// Runs `turn` on OpenCode, reporting its text, tools and usage as it goes
/// and the session as soon as OpenCode names it.
///
/// # Errors
/// OpenCode is missing, a hook it can't keep refuses the turn, OpenCode
/// fails, or the turn was stopped.
pub async fn run(
    turn: Turn,
    cancel: &Arc<AtomicBool>,
    on_event: &mut (dyn FnMut(RuntimeEvent) + Send),
    on_session: &mut (dyn FnMut(&str) + Send),
) -> Result<Streamed, String> {
    let program = binary().ok_or("OpenCode is not installed on this computer.")?;
    let (registry, _problems) = crate::sdk_hooks::opencode_registry(&turn.cwd);
    run_with(&program, turn, registry, cancel, on_event, on_session).await
}

/// [`run`] on OpenCode's `program`, under `registry`'s hooks: an issue
/// run's or a Solver hand's session adds the worktree guard and its own
/// fence (#11533).
///
/// # Errors
/// As [`run`].
pub async fn run_with(
    program: &Path,
    turn: Turn,
    registry: coder_sdk::Registry,
    cancel: &Arc<AtomicBool>,
    on_event: &mut (dyn FnMut(RuntimeEvent) + Send),
    on_session: &mut (dyn FnMut(&str) + Send),
) -> Result<Streamed, String> {
    use tokio::io::AsyncBufReadExt;

    if let Err(refused) = coder_sdk::check_backend(&registry, coder_sdk::HookBackend::OpenCode) {
        return Err(refused
            .into_iter()
            .map(|refused| refused.message)
            .collect::<Vec<_>>()
            .join(" "));
    }
    let runner = coder_sdk::Runner::new(registry, coder_sdk::Budget::default());
    let host = coder_sdk::bridge::Host::start(Arc::new(std::sync::Mutex::new(runner)))
        .map_err(|_| "Coder could not start its checks for OpenCode.".to_owned())?;
    let config_dir = config_dir()?;
    let this =
        std::env::current_exe().map_err(|_| "Coder could not find its own program.".to_owned())?;
    let config = config(&turn.model, turn.tools.then_some((&this, &turn.cwd)));
    let prompt = turn.prompt();

    let mut command = tokio::process::Command::new(&program);
    acp_client::process::non_interactive_async(&mut command);
    command
        .arg("run")
        // `--thinking` prints the model's reasoning parts (#11552).
        .args(["--format", "json", "--thinking", "--dir"])
        .arg(&turn.cwd)
        .args(["-m", &turn.model]);
    if let Some(session) = &turn.session {
        command.args(["--session", session]);
    }
    command
        .arg("--")
        .arg(&prompt)
        .current_dir(&turn.cwd)
        .env("OPENCODE_CONFIG_DIR", &config_dir)
        .env(acp_client::opencode::CONFIG_VAR, config.to_string())
        .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .env(coder_sdk::bridge::SOCKET_VAR, host.path())
        .env(coder_sdk::opencode::PROGRAM_VAR, &this)
        // A delegated child's place in the tree (#11511).
        .envs(crate::bundled_runtime::child_env())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let started = std::time::Instant::now();
    let mut child = command
        .spawn()
        .map_err(|_| "OpenCode could not start.".to_owned())?;
    let pid = child.id();
    let stdout = child.stdout.take().ok_or("OpenCode gave no output.")?;
    let stderr = child.stderr.take();
    let tail = tokio::spawn(async move {
        let mut tail = String::new();
        if let Some(stderr) = stderr {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tail.push_str(&line);
                tail.push('\n');
                if tail.len() > 4000 {
                    tail.drain(..tail.len() - 2000);
                }
            }
        }
        tail
    });
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let mut reply = Streamed {
        model: turn.model.clone(),
        ..Streamed::default()
    };
    let mut told_session = false;
    let mut error: Option<String> = None;
    let mut stopped = false;
    on_event(RuntimeEvent::Model(turn.model.clone()));
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break };
                for event in read_line(&line) {
                    match event {
                        Line::Session(session) if !told_session => {
                            told_session = true;
                            on_session(&session);
                        }
                        Line::Session(_) => {}
                        Line::Text(text) => {
                            let text = if reply.text.is_empty() {
                                text
                            } else {
                                format!("\n\n{text}")
                            };
                            if reply.first_text_ms.is_none() {
                                reply.first_text_ms =
                                    Some(started.elapsed().as_millis() as u64);
                            }
                            reply.text.push_str(&text);
                            on_event(RuntimeEvent::Text(text));
                        }
                        Line::Thinking(text) => {
                            on_event(RuntimeEvent::Thinking(true));
                            on_event(RuntimeEvent::Tool {
                                name: crate::claude_stream::THINKING.into(),
                                input: Value::Null,
                                output: json!({"text": text}),
                                running: false,
                            });
                            on_event(RuntimeEvent::Thinking(false));
                        }
                        Line::Tool { name, input, output } => {
                            on_event(RuntimeEvent::Tool {
                                name: name.clone(),
                                input: input.clone(),
                                output: Value::Null,
                                running: true,
                            });
                            on_event(RuntimeEvent::Tool { name, input, output, running: false });
                        }
                        Line::Usage { prompt, completion, reasoning, cost } => {
                            reply.usage.prompt_tokens += prompt;
                            reply.usage.completion_tokens += completion;
                            reply.usage.total_tokens += prompt + completion;
                            on_event(RuntimeEvent::Usage(Box::new(RoundUsage {
                                model: turn.model.clone(),
                                prompt_tokens: prompt,
                                completion_tokens: completion,
                                reasoning_tokens: (reasoning > 0).then_some(reasoning),
                                cost,
                                milliseconds: started.elapsed().as_millis() as u64,
                                first_token_ms: reply.first_text_ms,
                                cached_tokens: None,
                                cache_write_tokens: None,
                            })));
                        }
                        Line::Error(message) => error = Some(message),
                    }
                }
            }
            _ = tick.tick() => {
                if cancel.load(Ordering::Relaxed) {
                    stopped = true;
                    if let Some(pid) = pid {
                        let _ = std::process::Command::new("kill")
                            .args(["-TERM", &format!("-{pid}")])
                            .status();
                    }
                    let _ = child.start_kill();
                    break;
                }
            }
        }
    }
    let status = child.wait().await.ok();
    let tail = tail.await.unwrap_or_default();
    drop(host);
    reply.milliseconds = started.elapsed().as_millis() as u64;
    // An OpenCode model on a free or the person's own provider: no dollars
    // reach the chat's totals.
    reply.usage.cost = None;
    if stopped {
        return Err("Stopped.".into());
    }
    if let Some(error) = error {
        if reply.text.trim().is_empty() {
            return Err(format!("OpenCode: {error}"));
        }
    }
    if reply.text.trim().is_empty() {
        let failed = status.is_some_and(|status| !status.success());
        let said = tail
            .lines()
            .rev()
            .find(|line| line.to_ascii_lowercase().contains("error"))
            .map(|line| line.chars().take(300).collect::<String>());
        return Err(match (failed, said) {
            (_, Some(said)) => format!("OpenCode stopped: {said}"),
            (true, None) => "OpenCode stopped before it answered.".into(),
            (false, None) => "OpenCode returned no answer.".into(),
        });
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines OpenCode 1.18.26 printed for one turn (`--format json`).
    const TURN: &str = r#"{"type":"step_start","timestamp":1,"sessionID":"ses_1","part":{"type":"step-start"}}
{"type":"tool_use","timestamp":2,"sessionID":"ses_1","part":{"type":"tool","tool":"read","callID":"c1","state":{"status":"completed","input":{"filePath":"/r/a.txt"},"output":"1: hello","metadata":{"preview":"hello"}}}}
{"type":"tool_use","timestamp":3,"sessionID":"ses_1","part":{"type":"tool","tool":"bash","callID":"c2","state":{"status":"error","input":{"command":"rm a.txt"},"error":"Held for the person: this would delete files (rm a.txt)."}}}
{"type":"tool_use","timestamp":4,"sessionID":"ses_1","part":{"type":"tool","tool":"bash","callID":"c3","state":{"status":"completed","input":{"command":"cargo test"},"output":"ok","metadata":{"exit":0}}}}
{"type":"step_finish","timestamp":5,"sessionID":"ses_1","part":{"type":"step-finish","reason":"tool-calls","tokens":{"total":20855,"input":20411,"output":21,"reasoning":350,"cache":{"write":0,"read":73}},"cost":0}}
{"type":"text","timestamp":6,"sessionID":"ses_1","part":{"type":"text","text":"hello"}}"#;

    #[test]
    fn a_turns_lines_read_as_coders_events() {
        let read: Vec<Line> = TURN.lines().flat_map(read_line).collect();
        assert_eq!(read[0], Line::Session("ses_1".into()));
        assert!(read.contains(&Line::Tool {
            name: "Read".into(),
            input: json!({"file_path": "/r/a.txt"}),
            output: json!({"output": "1: hello"}),
        }));
        assert!(read.contains(&Line::Tool {
            name: "Bash".into(),
            input: json!({"command": "rm a.txt"}),
            output: json!({"error": "Held for the person: this would delete files (rm a.txt)."}),
        }));
        assert!(read.contains(&Line::Tool {
            name: "Bash".into(),
            input: json!({"command": "cargo test"}),
            output: json!({"output": "ok", "exit": 0}),
        }));
        assert!(read.contains(&Line::Usage {
            prompt: 20484,
            completion: 21,
            reasoning: 350,
            cost: None,
        }));
        assert_eq!(read.last(), Some(&Line::Text("hello".into())));
        assert!(read_line("not json").is_empty());
        // #11552: `opencode run --format json --thinking` (opencode 1.18.26,
        // recorded 2026-10-13 on opencode/cat-jam-free) prints the
        // reasoning part whole, ahead of the answer.
        let path = format!(
            "{}/fixtures/thinking/opencode-run.jsonl",
            env!("CARGO_MANIFEST_DIR")
        );
        let lines: Vec<Line> = std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .flat_map(read_line)
            .collect();
        let thinking = lines.iter().position(|line| {
            matches!(line, Line::Thinking(text) if text.starts_with("The user is asking whether 9991 is prime."))
        });
        let text = lines
            .iter()
            .position(|line| matches!(line, Line::Text(text) if text.contains("97")));
        assert!(thinking.is_some() && thinking < text, "{lines:?}");
        let failed = read_line(
            r#"{"type":"error","sessionID":"ses_1","error":{"name":"APIError","data":{"message":"Rate limit exceeded"}}}"#,
        );
        assert_eq!(failed[1], Line::Error("Rate limit exceeded".into()));
    }

    fn message(role: &str, content: &str) -> Message {
        let mut message = Message::user(content);
        message.role = role.into();
        message
    }

    /// #11547: the rail names OpenCode's free models honestly, by name.
    #[test]
    fn the_rail_names_opencode_free_models() {
        assert_eq!(rail_label(DEFAULT_MODEL), "OpenCode (Cat Jam)");
        assert_eq!(
            rail_label(coder::delegate_door::OPENCODE_AUTO_FALLBACK),
            "OpenCode (Big Pickle)"
        );
        assert_eq!(
            rail_label("anthropic/claude-sonnet-5"),
            "OpenCode (anthropic/claude-sonnet-5)"
        );
        assert_eq!(model(None), DEFAULT_MODEL);
    }

    #[test]
    fn a_new_session_opens_with_the_identity_and_a_follow_up_sends_the_message() {
        let mut turn = Turn {
            messages: vec![message("user", "What is in a.txt?")],
            cwd: PathBuf::from("/r"),
            session: None,
            last_model: None,
            briefing: None,
            model: DEFAULT_MODEL.into(),
            tools: false,
        };
        let first = turn.prompt();
        assert!(first.starts_with("You are Coder"));
        assert!(first.contains("opencode/cat-jam-free"));
        assert!(first.ends_with("What is in a.txt?"));
        turn.messages.push(message("assistant", "hello"));
        turn.messages.push(message("user", "Now change it."));
        turn.session = Some("ses_1".into());
        turn.last_model = Some(DEFAULT_MODEL.into());
        assert_eq!(turn.prompt(), "Now change it.");
        turn.model = "anthropic/claude-sonnet-5".into();
        assert!(turn.prompt().starts_with("<model-switch>"));
        // A chat that moved here from another harness brings its turns.
        turn.session = None;
        let moved = turn.prompt();
        assert!(moved.contains("hello") && moved.contains("Now change it."));
    }

    #[test]
    fn the_configuration_allows_tools_for_the_plugin_to_check_and_offers_coders_tools() {
        let bare = config(DEFAULT_MODEL, None);
        assert_eq!(bare["model"], DEFAULT_MODEL);
        assert_eq!(bare["permission"]["question"], "deny");
        assert!(bare.get("mcp").is_none());
        let tools = config(
            DEFAULT_MODEL,
            Some((Path::new("/bin/coder"), Path::new("/r"))),
        );
        assert_eq!(
            tools["mcp"]["coder"]["command"],
            json!([
                "/bin/coder",
                "coder-tools-mcp",
                "--harness",
                "opencode",
                "--cwd",
                "/r"
            ])
        );
        assert_eq!(
            coder_sdk::opencode::CODER_SERVER,
            crate::claude_tools::SERVER
        );
    }

    #[test]
    fn the_plugin_is_written_once_and_kept_current() {
        let dir = tempfile::tempdir().unwrap();
        coder_sdk::opencode::write_plugin(dir.path()).unwrap();
        let path = dir
            .path()
            .join("plugin")
            .join(coder_sdk::opencode::PLUGIN_FILE);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            coder_sdk::opencode::PLUGIN_SOURCE
        );
        std::fs::write(&path, "changed").unwrap();
        coder_sdk::opencode::write_plugin(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            coder_sdk::opencode::PLUGIN_SOURCE
        );
    }

    #[test]
    fn the_model_is_a_pin_then_the_default() {
        assert_eq!(model(None), DEFAULT_MODEL);
        assert_eq!(
            model(Some("anthropic/claude-sonnet-5".into())),
            "anthropic/claude-sonnet-5"
        );
        assert_eq!(model(Some("not a model".into())), DEFAULT_MODEL);
    }

    /// No dead controls (#11507): where OpenCode isn't installed, the
    /// Plugins screen doesn't list it and nothing turns it on.
    #[test]
    fn nothing_offers_opencode_where_it_is_not_installed() {
        install_for_test(false);
        let mut app = crate::App::default();
        assert!(!app.plugins.definitions().any(|row| row.id == PLUGIN));
        assert!(!crate::plugin_definition::listed().any(|row| row.id == PLUGIN));
        assert!(!app.plugins.bundled.enabled(PLUGIN));
        assert!(!app.plugins.bundled.toggle(PLUGIN));
        install_for_test(true);
        assert!(app.plugins.definitions().any(|row| row.id == PLUGIN));
        assert!(app.plugins.bundled.enabled(PLUGIN));
        install_for_test(false);
    }

    /// The harness order on a real chat: OpenCode when Claude Code is off,
    /// Claude Code when it is on, and the turn goes to OpenCode's runner.
    #[test]
    fn a_new_chat_runs_on_opencode_when_claude_code_is_off() {
        use crate::harness_plugin::Harness;
        install_for_test(true);
        crate::harness_plugin::sign_in_claude(false);
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        assert_eq!(app.route_turn(), Harness::OpenCode);
        assert!(app.on_opencode() && !app.on_coder_loop() && !app.on_claude());
        app.draft.text = "What is in a.txt?".into();
        app.draft.cursor = app.draft.text.len();
        app.submit_live();
        match &app.request.as_ref().expect("a turn").kind {
            crate::live::Work::OpenCode {
                session,
                model,
                tools,
                ..
            } => {
                assert!(session.is_none());
                assert_eq!(model, DEFAULT_MODEL);
                assert!(*tools);
            }
            _ => panic!("the turn runs on OpenCode"),
        }
        // OpenCode names its session; the follow-up continues it.
        let id = app.request_id;
        app.apply_update(crate::live::Update::OpenCodeSession {
            id,
            session: "ses_1".into(),
            model: DEFAULT_MODEL.into(),
        });
        app.live
            .entries
            .push(crate::live::Entry::User("next".into()));
        assert_eq!(
            app.live.opencode_resume(),
            Some(("ses_1".into(), DEFAULT_MODEL.into()))
        );
        crate::harness_plugin::sign_in_claude(true);
        app.plugins.bundled.claude = true;
        app.live = Default::default();
        assert_eq!(app.route_turn(), Harness::ClaudeCode);
        crate::harness_plugin::sign_in_claude(false);
        install_for_test(false);
    }
}
