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

/// A provider's error as words: a provider that answers with its own
/// JSON (`{"message":"Update Required …","type":"cursor_error"}`) reads as
/// its message, never the raw JSON.
#[must_use]
pub fn plain_error(message: &str) -> String {
    let mut message = message.trim().to_owned();
    for _ in 0..3 {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&message) else {
            break;
        };
        let inner = value["message"]
            .as_str()
            .or_else(|| value["error"]["message"].as_str())
            .or_else(|| value["error"].as_str());
        match inner {
            Some(inner) => message = inner.trim().to_owned(),
            None => break,
        }
    }
    message
}

/// The rail's words for a chat on OpenCode: "OpenCode (Cat Jam)" for
/// OpenCode Zen's free models, by name, else "OpenCode (provider/model)".
#[must_use]
pub fn rail_label(model: &str) -> String {
    let name = match model {
        "opencode/cat-jam-free" => "Cat Jam".to_owned(),
        "opencode/big-pickle" => "Big Pickle".to_owned(),
        // OpenCode's own name for it, else its words: never a raw
        // `provider/id` (owner, 2026-10-13).
        other => crate::opencode_models::name(other)
            .or_else(|| {
                if cfg!(test) {
                    None
                } else {
                    acp_client::opencode::configured_name(&|name| std::env::var_os(name), other)
                }
            })
            .unwrap_or_else(|| crate::ui::rail_model_name(other)),
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

/// Has OpenCode finish setting up Coder's configuration folder (install
/// its plugin package, `coder_sdk::opencode::plugin_ready`) before a turn
/// counts the time OpenCode takes to start (#11683): on a fresh computer
/// that install is silent and can outlast the start limit. Quick once done.
/// False when it still isn't set up; the turn runs anyway.
pub async fn prepare(program: &Path) -> bool {
    static SETTING_UP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let Ok(dir) = config_dir() else {
        return false;
    };
    if coder_sdk::opencode::plugin_ready(&dir) {
        return true;
    }
    let _one = SETTING_UP.lock().await;
    if coder_sdk::opencode::plugin_ready(&dir) {
        return true;
    }
    let mut command = acp_client::process::command_async(program);
    acp_client::process::non_interactive_async(&mut command);
    command
        .args(coder_sdk::opencode::SETUP_ARGUMENTS)
        .current_dir(&dir)
        .env("OPENCODE_CONFIG_DIR", &dir)
        .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let finished = match command.spawn() {
        Ok(mut child) => tokio::time::timeout(std::time::Duration::from_secs(300), child.wait())
            .await
            .is_ok(),
        Err(_) => false,
    };
    let ready = coder_sdk::opencode::plugin_ready(&dir);
    if !ready {
        eprintln!(
            "coder: OpenCode didn't finish setting up its plugin folder (finished: {finished})"
        );
    }
    ready
}

/// [`run`] on OpenCode's `program`, under `registry`'s hooks: an issue
/// run's or a Solver hand's session adds the worktree guard and its own
/// fence (#11533).
///
/// The turn is an ACP session on `opencode acp` (#11684): it streams the
/// answer and the thinking as the model writes them, where `opencode run
/// --format json` prints each part only once it is whole. Coder's plugin
/// (`coder_sdk::opencode`) checks every tool before it runs, as before,
/// through the checks host this turn starts.
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
    if let Err(refused) = coder_sdk::check_backend(&registry, coder_sdk::HookBackend::OpenCode) {
        return Err(refused
            .into_iter()
            .map(|refused| refused.message)
            .collect::<Vec<_>>()
            .join(" "));
    }
    prepare(program).await;
    let runner = coder_sdk::Runner::new(registry, coder_sdk::Budget::default());
    let host = coder_sdk::bridge::Host::start(Arc::new(std::sync::Mutex::new(runner)))
        .map_err(|_| "Coder could not start its checks for OpenCode.".to_owned())?;
    let config_dir = config_dir()?;
    let this =
        std::env::current_exe().map_err(|_| "Coder could not find its own program.".to_owned())?;
    let config = config(&turn.model, turn.tools.then_some((&this, &turn.cwd)));
    let prompt = turn.prompt();

    let mut environment: Vec<(String, String)> = std::env::vars()
        .filter(|(name, _)| {
            !matches!(
                name.as_str(),
                "OPENCODE_CONFIG_DIR" | "OPENCODE_DISABLE_AUTOUPDATE"
            ) && name != acp_client::opencode::CONFIG_VAR
                && name != coder_sdk::bridge::SOCKET_VAR
                && name != coder_sdk::opencode::PROGRAM_VAR
        })
        .collect();
    environment.extend([
        (
            "OPENCODE_CONFIG_DIR".to_owned(),
            config_dir.display().to_string(),
        ),
        (
            acp_client::opencode::CONFIG_VAR.to_owned(),
            config.to_string(),
        ),
        ("OPENCODE_DISABLE_AUTOUPDATE".to_owned(), "1".to_owned()),
        (
            coder_sdk::bridge::SOCKET_VAR.to_owned(),
            host.path().display().to_string(),
        ),
        (
            coder_sdk::opencode::PROGRAM_VAR.to_owned(),
            this.display().to_string(),
        ),
    ]);
    // A delegated child's place in the tree (#11511).
    for (name, value) in crate::bundled_runtime::child_env() {
        let (name, value) = (
            name.to_string_lossy().into_owned(),
            value.to_string_lossy().into_owned(),
        );
        environment.retain(|(held, _)| *held != name);
        environment.push((name, value));
    }
    let opening = acp_client::Opening {
        spec: acp_client::process::Spec {
            program: program.to_owned(),
            arguments: acp_client::opencode::arguments(),
            cwd: turn.cwd.clone(),
            environment,
        },
        resume: turn.session.clone(),
        meta: None,
        mode: None,
        authenticate: None,
    };
    let stopped = || cancel.load(Ordering::Relaxed);
    let started = std::time::Instant::now();
    on_event(RuntimeEvent::Model(turn.model.clone()));
    let mut session = acp_client::Session::open(&opening, &stopped)
        .await
        .map_err(|failure| {
            if stopped() {
                "Stopped.".to_owned()
            } else {
                format!("OpenCode could not start: {failure}")
            }
        })?;
    on_session(session.id());
    let mut events = Events {
        emit: on_event,
        started,
        text: String::new(),
        first_text_ms: None,
        thought: crate::claude_stream::Thought::default(),
        tools: std::collections::BTreeMap::new(),
        tokens: 0,
    };
    let result = session
        .prompt(
            &prompt,
            std::time::Duration::from_secs(300),
            &stopped,
            std::time::Duration::from_secs(2),
            &mut events,
        )
        .await;
    events.end_thought();
    let tail = session.stderr_tail().join("\n");
    session.close(std::time::Duration::from_secs(2)).await;
    drop(host);
    if stopped() {
        return Err("Stopped.".into());
    }
    let mut reply = Streamed {
        model: turn.model.clone(),
        text: events.text,
        first_text_ms: events.first_text_ms,
        milliseconds: started.elapsed().as_millis() as u64,
        ..Streamed::default()
    };
    let prompted = match result {
        Ok(prompted) => prompted,
        Err(error) => {
            if reply.text.trim().is_empty() {
                return Err(format!("OpenCode: {}", plain_error(&error.to_string())));
            }
            return Ok(reply);
        }
    };
    if let Some(usage) = prompted.usage {
        let prompt_tokens = usage.input_tokens.unwrap_or(0) + usage.cached_read_tokens.unwrap_or(0);
        let completion = usage.output_tokens.unwrap_or(0);
        reply.usage.prompt_tokens = prompt_tokens;
        reply.usage.completion_tokens = completion;
        reply.usage.total_tokens = prompt_tokens + completion;
        (events.emit)(RuntimeEvent::Usage(Box::new(RoundUsage {
            fell_back: None,
            model: turn.model.clone(),
            prompt_tokens,
            completion_tokens: completion,
            reasoning_tokens: None,
            cost: None,
            milliseconds: reply.milliseconds,
            first_token_ms: reply.first_text_ms,
            cached_tokens: usage.cached_read_tokens,
            cache_write_tokens: None,
        })));
    }
    // An OpenCode model on a free or the person's own provider: no dollars
    // reach the chat's totals.
    reply.usage.cost = None;
    if reply.text.trim().is_empty() {
        return Err(no_answer(true, &tail));
    }
    Ok(reply)
}

/// One OpenCode turn's ACP updates as Coder's events: the answer and the
/// thinking as they stream, each tool under Claude Code's name for it.
struct Events<'a> {
    emit: &'a mut (dyn FnMut(RuntimeEvent) + Send),
    started: std::time::Instant,
    text: String,
    first_text_ms: Option<u64>,
    thought: crate::claude_stream::Thought,
    /// Each tool call's name and input so far, by its id.
    tools: std::collections::BTreeMap<String, (String, String, Value)>,
    tokens: u64,
}

impl Events<'_> {
    fn end_thought(&mut self) {
        for event in self.thought.end(None) {
            (self.emit)(event);
        }
    }
}

impl acp_client::Handler for Events<'_> {
    fn update(&mut self, update: acp_client::Update) {
        use acp_client::Update;
        if matches!(
            update,
            Update::AgentText(_) | Update::ToolCall { .. } | Update::ToolCallUpdate { .. }
        ) {
            self.end_thought();
        }
        match update {
            Update::AgentText(text) if !text.is_empty() => {
                if self.first_text_ms.is_none() {
                    self.first_text_ms = Some(self.started.elapsed().as_millis() as u64);
                }
                self.text.push_str(&text);
                (self.emit)(RuntimeEvent::Text(text));
            }
            Update::Thought(text) => {
                for event in self.thought.piece(&text) {
                    (self.emit)(event);
                }
            }
            Update::Usage(usage) if !usage.subagent => {
                let tokens = usage
                    .input_tokens
                    .unwrap_or(0)
                    .saturating_add(usage.output_tokens.unwrap_or(0));
                if tokens > self.tokens {
                    self.tokens = tokens;
                    (self.emit)(RuntimeEvent::Tokens(tokens));
                }
            }
            Update::ToolCall {
                id,
                title,
                status,
                raw_input,
                tool,
                ..
            } => {
                let own = tool.unwrap_or_else(|| title.clone());
                let name = coder_sdk::opencode::tool_name(&own);
                let input = coder_sdk::opencode::args_to_claude(&own, &raw_input);
                self.tools.insert(id, (own, name.clone(), raw_input));
                (self.emit)(RuntimeEvent::Tool {
                    name,
                    input,
                    output: Value::Null,
                    running: status != "completed" && status != "failed",
                });
            }
            Update::ToolCallUpdate {
                id,
                title,
                status,
                text,
                raw_input,
            } => {
                let (own, name, held) = self.tools.entry(id.clone()).or_insert_with(|| {
                    let own = title.unwrap_or(id);
                    (own.clone(), coder_sdk::opencode::tool_name(&own), json!({}))
                });
                // OpenCode sends the command and its description here,
                // after the call opened.
                if let Some(Value::Object(fields)) = raw_input {
                    if !held.is_object() {
                        *held = json!({});
                    }
                    if let Some(object) = held.as_object_mut() {
                        object.extend(fields);
                    }
                }
                let failed = status.as_deref() == Some("failed");
                let output = match text {
                    Some(text) if failed => json!({"error": text}),
                    Some(text) => json!({"output": text}),
                    None => Value::Null,
                };
                (self.emit)(RuntimeEvent::Tool {
                    name: name.clone(),
                    input: coder_sdk::opencode::args_to_claude(own, held),
                    output,
                    running: status
                        .as_deref()
                        .is_none_or(|status| status != "completed" && status != "failed"),
                });
            }
            _ => {}
        }
    }
}

/// Why an OpenCode turn that ended without words failed: the error line
/// it printed, else its last line of standard error, so the real reason
/// is always there to read (#11645).
fn no_answer(failed: bool, stderr: &str) -> String {
    let clip = |line: &str| line.trim().chars().take(300).collect::<String>();
    let lines = || stderr.lines().rev().filter(|line| !line.trim().is_empty());
    let said = lines()
        .find(|line| line.to_ascii_lowercase().contains("error"))
        .or_else(|| lines().next())
        .map(clip);
    match (failed, said) {
        (_, Some(said)) => format!("OpenCode stopped: {said}"),
        (true, None) => "OpenCode stopped before it answered.".into(),
        (false, None) => "OpenCode returned no answer.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_without_words_says_what_opencode_printed() {
        assert_eq!(
            no_answer(true, "booting\nError: no provider configured\n"),
            "OpenCode stopped: Error: no provider configured"
        );
        assert_eq!(
            no_answer(true, "node: cannot find module\n\n"),
            "OpenCode stopped: node: cannot find module"
        );
        assert_eq!(no_answer(true, ""), "OpenCode stopped before it answered.");
        assert_eq!(no_answer(false, ""), "OpenCode returned no answer.");
    }

    /// #11684: the answer and the thinking reach the chat as OpenCode's
    /// ACP updates arrive, piece by piece, before the turn ends; each tool
    /// shows under Claude Code's name with its input, and a refused one
    /// shows its reason.
    #[test]
    fn a_turns_updates_stream_as_coders_events() {
        use acp_client::{Handler, Update};
        let mut seen: Vec<RuntimeEvent> = Vec::new();
        let mut emit = |event: RuntimeEvent| seen.push(event);
        let mut events = Events {
            emit: &mut emit,
            started: std::time::Instant::now(),
            text: String::new(),
            first_text_ms: None,
            thought: crate::claude_stream::Thought::default(),
            tools: std::collections::BTreeMap::new(),
            tokens: 0,
        };
        events.update(Update::Thought("Checking ".into()));
        events.update(Update::Thought("the file.".into()));
        events.update(Update::ToolCall {
            id: "c1".into(),
            title: "read".into(),
            kind: "read".into(),
            status: "pending".into(),
            raw_input: json!({"filePath": "/r/a.txt"}),
            tool: Some("read".into()),
        });
        events.update(Update::ToolCallUpdate {
            id: "c1".into(),
            title: None,
            status: Some("completed".into()),
            text: Some("1: hello".into()),
            raw_input: None,
        });
        events.update(Update::ToolCall {
            id: "c2".into(),
            title: "bash".into(),
            kind: "execute".into(),
            status: "pending".into(),
            raw_input: json!({}),
            tool: Some("bash".into()),
        });
        events.update(Update::ToolCallUpdate {
            id: "c2".into(),
            title: None,
            status: Some("failed".into()),
            text: Some("Held for the person: this would delete files (rm a.txt).".into()),
            raw_input: Some(json!({"command": "rm a.txt"})),
        });
        events.update(Update::AgentText("Hel".into()));
        let first = events.first_text_ms;
        events.update(Update::AgentText("lo.".into()));
        events.end_thought();
        assert!(first.is_some());
        assert_eq!(events.text, "Hello.");
        let texts: Vec<&str> = seen
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["Hel", "lo."], "the answer streams in pieces");
        assert!(
            seen.iter()
                .any(|event| matches!(event, RuntimeEvent::Thinking(true)))
        );
        assert!(seen.iter().any(|event| matches!(
            event,
            RuntimeEvent::Tool { name, input, output, running: false }
                if name == "Read" && input == &json!({"file_path": "/r/a.txt"}) && output == &json!({"output": "1: hello"})
        )), "{seen:?}");
        assert!(
            seen.iter().any(|event| matches!(
                event,
                RuntimeEvent::Tool { name, input, output, running: false }
                    if name == "Bash" && input == &json!({"command": "rm a.txt"})
                        && output["error"].as_str().is_some_and(|error| error.starts_with("Held"))
            )),
            "{seen:?}"
        );
        // A provider that answers with its own JSON (Cursor's "Update
        // Required", owner's gate 2026-10-13) reads as its message.
        assert_eq!(
            plain_error(
                r#"{"message":"Update Required Your version of Cursor is no longer supported.","type":"cursor_error"}"#
            ),
            "Update Required Your version of Cursor is no longer supported."
        );
        assert_eq!(plain_error("plain words"), "plain words");
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
            "OpenCode (Claude Sonnet 5)"
        );
        assert_eq!(rail_label("cursor/composer-2.5"), "OpenCode (Composer 2.5)");
        // The person's opencode.json names its own provider's models.
        assert_eq!(
            acp_client::opencode::name_in(
                br#"{"provider":{"cursor":{"options":{"apiKey":"x"},"models":{"composer-2.5":{"name":"Cursor 2.5"}}}}}"#,
                "cursor",
                "composer-2.5"
            )
            .as_deref(),
            Some("Cursor 2.5")
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

/// A real turn on the OpenCode installed here, on its free model, timing
/// each piece of the answer: they arrive while the model writes (#11684).
/// Run by hand: `cargo test -p coder-new --lib live_opencode_streams -- --ignored --nocapture`.
#[cfg(test)]
#[tokio::test]
#[ignore = "runs the installed OpenCode and a model"]
async fn live_opencode_streams() {
    let program = binary().expect("OpenCode installed");
    let dir = tempfile::tempdir().unwrap();
    let turn = Turn {
        messages: vec![Message::user(
            "Write a 300-word story about a lighthouse keeper. No tools.",
        )],
        cwd: dir.path().to_owned(),
        session: None,
        last_model: None,
        briefing: None,
        model: auto_model().to_owned(),
        tools: false,
    };
    let started = std::time::Instant::now();
    let mut pieces: Vec<(u128, usize)> = Vec::new();
    let mut on_event = |event: RuntimeEvent| {
        if let RuntimeEvent::Text(text) = event {
            pieces.push((started.elapsed().as_millis(), text.len()));
        }
    };
    let mut on_session = |_: &str| {};
    let cancel = Arc::new(AtomicBool::new(false));
    let (registry, _) = crate::sdk_hooks::opencode_registry(dir.path());
    let reply = run_with(
        &program,
        turn,
        registry,
        &cancel,
        &mut on_event,
        &mut on_session,
    )
    .await
    .expect("OpenCode answered");
    eprintln!(
        "pieces {} first {:?} last {:?} total {} ms",
        pieces.len(),
        pieces.first(),
        pieces.last(),
        reply.milliseconds
    );
    assert!(pieces.len() > 3, "{pieces:?}");
    let (first, _) = pieces[0];
    let (last, _) = pieces[pieces.len() - 1];
    assert!(
        last - first > 500,
        "the answer arrived all at once: {pieces:?}"
    );
}
