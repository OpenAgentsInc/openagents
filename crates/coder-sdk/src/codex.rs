//! The Codex backend (#11510): Codex on `codex app-server`, its JSON-RPC
//! protocol over standard input and output, with the session's hooks
//! answering Codex's approval requests.
//!
//! A thread starts under `approval_policy = "untrusted"` with full disk
//! access, never under Codex's bypass flags: Codex asks before it runs a
//! command or applies a patch, and the SDK's `PreToolUse` composition
//! answers ([`Hooks::fire`]): the floors, the shared checkout's fence, the
//! gate, the person's and the project's command hooks, the plugins' hooks.
//! An allow runs the call (`accept`); a deny declines it and tells the agent
//! why in the same turn; a deny that ends the session cancels the turn. An
//! `Ask` goes to the host's asker (the approval card), which answers Allow
//! once or Deny while Codex waits, so the same thread goes on either way;
//! with no asker it is held: declined with its step.
//!
//! Codex's calls reach the hooks in Claude Code's shapes, so a hook written
//! once fires on both: a command is `Bash` with its `command` (Codex wraps
//! it as `/bin/zsh -lc '...'`; the floors read through wrappers) and its
//! `cwd`, and a patch is `apply_patch` with the patch text (`*** Update
//! File: path` lines) and Codex's `changes`.
//!
//! The rest of the event set: `SessionStart` fires before the thread opens
//! and its context joins the developer instructions; `UserPromptSubmit`
//! fires before each turn and can refuse it or add context ahead of the
//! message; `Stop` fires when the turn completes, and a block starts the
//! next turn on the same thread with the reason; `PostToolUse` and
//! `PostCompact` are seen as Codex reports them, after the fact, and their
//! context is steered into the running turn. The tier table
//! (`coder_delegate::session::hook_tier`, `HookBackend::CodexAppServer`)
//! says the same.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use coder_delegate::session::HookBackend;
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::UnboundedReceiver;

use super::compose::{Applied, Budget, Refused};
use super::events::{Call, Event};
use super::hooks::{Asker, Hooks};
use super::outcome::{Context, Permission, Stop};
use super::registry::Registry;
use super::tools::ToolServer;

/// The approval policy a Coder thread runs under: Codex asks before every
/// call it doesn't know to be safe, and the hooks answer.
pub const APPROVAL_POLICY: &str = "untrusted";
/// The sandbox: none. Coder's one permission is `bypass`, with the floors
/// on top; the approvals are where they hold.
pub const SANDBOX: &str = "danger-full-access";
/// The tool name a Codex command reaches the hooks under.
pub const COMMAND_TOOL: &str = "Bash";
/// The tool name a Codex patch reaches the hooks under.
pub const PATCH_TOOL: &str = "apply_patch";

/// How to start `codex app-server`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    /// The `codex` program.
    pub program: PathBuf,
    /// `-c key=value` overrides, each `key=value`. The person's
    /// `config.toml` stays as it is.
    pub config: Vec<String>,
    /// Variables set for the process.
    pub env: Vec<(String, String)>,
    /// Variables removed from the process.
    pub env_remove: Vec<String>,
    /// Where the process runs; `None` keeps the host's folder.
    pub cwd: Option<PathBuf>,
    /// How long Codex has to answer `initialize`; `None` waits
    /// [`START_WAIT`]. A program that never answers is not app-server.
    pub start_wait: Option<std::time::Duration>,
}

/// How long Codex has to answer `initialize` unless the launch says.
pub const START_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

impl Launch {
    /// `codex` on `PATH`.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Launch {
            program: program.into(),
            ..Launch::default()
        }
    }

    /// The program's arguments.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        let mut args = vec!["app-server".to_owned()];
        for config in &self.config {
            args.push("-c".into());
            args.push(config.clone());
        }
        args
    }
}

/// How a thread opens.
#[derive(Clone, Debug, Default)]
pub struct Open {
    pub cwd: PathBuf,
    pub model: Option<String>,
    /// `low`, `medium`, `high` or `xhigh`.
    pub effort: Option<String>,
    /// Added to Codex's own instructions: the briefing, Coder's guidance.
    pub developer_instructions: Option<String>,
    /// The thread to continue; `None` starts one.
    pub resume: Option<String>,
    /// Coder's tools, offered as Codex dynamic tools in process: each call
    /// comes back to this connection, through the session's hooks.
    pub tools: Option<ToolServer>,
}

/// What the host can ask of a running turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Control {
    /// Stop the turn; the thread stays.
    Interrupt,
    /// Add a message to the running turn.
    Steer(String),
}

/// What Codex reports while a turn runs, in the host's terms.
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// The thread the session runs in, once Codex names it.
    Thread(String),
    /// The model the turn runs on.
    Model(String),
    /// Part of the reply.
    Text(String),
    /// The model is thinking, or stopped.
    Thinking(bool),
    /// A tool call, as it starts (`running`) and as it ends.
    Tool {
        name: String,
        input: Value,
        output: Value,
        running: bool,
    },
    /// The hooks answered a call: what they came to.
    Gated(Box<Applied>),
    /// The thread's token usage so far (`thread/tokenUsage/updated`).
    Usage(Value),
    /// Where the plan's windows stand (`account/rateLimits/updated`).
    RateLimits(Value),
}

/// How a turn ended.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ended {
    pub thread: String,
    pub turn: String,
    /// `completed`, `interrupted` or `failed`.
    pub status: String,
    /// The reply: the turn's final message, else everything it said.
    pub text: String,
    pub model: Option<String>,
    /// The thread's token usage, as last reported.
    pub usage: Value,
    pub error: Option<String>,
    /// Turns the `Stop` hooks sent the agent back for.
    pub continued: u32,
}

impl Ended {
    /// Whether the turn was stopped.
    #[must_use]
    pub fn interrupted(&self) -> bool {
        self.status == "interrupted"
    }
}

type Reader = Box<dyn AsyncBufRead + Unpin + Send>;
type Writer = Box<dyn AsyncWrite + Unpin + Send>;

/// One `codex app-server` connection, with one session's hooks.
pub struct Codex {
    reader: Reader,
    writer: Writer,
    child: Option<tokio::process::Child>,
    hooks: Hooks,
    next_id: i64,
    thread: Option<String>,
    cwd: PathBuf,
    model: Option<String>,
    /// Items Codex started, by id: a patch's approval names only its item.
    items: HashMap<String, Value>,
    usage: Value,
    tools: Option<ToolServer>,
    /// Codex's version, from its `initialize` answer.
    version: Option<String>,
}

impl std::fmt::Debug for Codex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Codex")
            .field("thread", &self.thread)
            .field("cwd", &self.cwd)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

/// The hooks Codex on app-server runs: `registry`'s, checked against its
/// tiers.
///
/// # Errors
///
/// The hooks that must deny on an event Codex only reports afterwards.
pub fn hooks(registry: Registry, budget: Budget) -> Result<Hooks, Vec<Refused>> {
    Hooks::new(registry, budget, HookBackend::CodexAppServer)
}

impl Codex {
    /// Starts `launch` and initializes it.
    ///
    /// # Errors
    ///
    /// Codex didn't start or didn't answer.
    pub async fn spawn(launch: &Launch, hooks: Hooks) -> Result<Self, String> {
        let mut command = tokio::process::Command::new(&launch.program);
        command
            .args(launch.args())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        if let Some(cwd) = &launch.cwd {
            command.current_dir(cwd);
        }
        for (name, value) in &launch.env {
            command.env(name, value);
        }
        for name in &launch.env_remove {
            command.env_remove(name);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("Codex didn't start: {error}"))?;
        let stdout = child.stdout.take().ok_or("Codex has no output.")?;
        let stdin = child.stdin.take().ok_or("Codex has no input.")?;
        let mut codex = Codex::over(BufReader::new(stdout), stdin, hooks);
        codex.child = Some(child);
        let wait = launch.start_wait.unwrap_or(START_WAIT);
        match tokio::time::timeout(wait, codex.initialize()).await {
            Ok(result) => result?,
            Err(_) => return Err("Codex didn't answer as app-server.".into()),
        };
        Ok(codex)
    }

    /// A connection over `reader` and `writer`, not yet initialized: a
    /// recorded conversation in tests.
    pub fn over(
        reader: impl AsyncBufRead + Unpin + Send + 'static,
        writer: impl AsyncWrite + Unpin + Send + 'static,
        hooks: Hooks,
    ) -> Self {
        Codex {
            reader: Box::new(reader),
            writer: Box::new(writer),
            child: None,
            hooks,
            next_id: 0,
            thread: None,
            cwd: PathBuf::new(),
            model: None,
            items: HashMap::new(),
            usage: Value::Null,
            tools: None,
            version: None,
        }
    }

    /// Answers each `Ask` through `asker`.
    #[must_use]
    pub fn asking(mut self, asker: Asker) -> Self {
        self.hooks = self.hooks.asking(asker);
        self
    }

    /// The thread, once open.
    #[must_use]
    pub fn thread(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    /// Codex's version, once initialized.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The model the thread runs on, once Codex says.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// The `initialize` handshake.
    ///
    /// # Errors
    ///
    /// Codex refused it or went away.
    pub async fn initialize(&mut self) -> Result<Value, String> {
        let result = self
            .request(
                "initialize",
                // Dynamic tools, Coder's tools in process, are part of
                // Codex's experimental surface.
                json!({
                    "clientInfo": {"name": "coder", "title": "Coder", "version": env!("CARGO_PKG_VERSION")},
                    "capabilities": {"experimentalApi": true},
                }),
                &mut |_| {},
            )
            .await?;
        self.notify("initialized", Value::Null).await?;
        // `codex_cli_rs/0.162.1 (Mac OS 26.4.0; arm64) ...`: the version is
        // the first word's tail.
        self.version = result["userAgent"]
            .as_str()
            .and_then(|agent| agent.split_whitespace().next())
            .and_then(|first| first.rsplit('/').next())
            .map(str::to_owned);
        Ok(result)
    }

    /// Opens the thread: continues `open.resume`, or starts one. The
    /// `SessionStart` hooks fire first; their context joins the developer
    /// instructions.
    ///
    /// # Errors
    ///
    /// Codex refused the thread, such as one it no longer has.
    pub async fn open(
        &mut self,
        open: &Open,
        emit: &mut dyn FnMut(Output),
    ) -> Result<String, String> {
        self.cwd.clone_from(&open.cwd);
        let started = self
            .hooks
            .fire(Call::new(
                Event::SessionStart,
                json!({"cwd": open.cwd, "source": if open.resume.is_some() { "resume" } else { "startup" }}),
            ))
            .await;
        let instructions = join(
            open.developer_instructions.as_deref(),
            context_text(&started.context).as_deref(),
        );
        let mut params = json!({
            "cwd": open.cwd,
            "approvalPolicy": APPROVAL_POLICY,
            "sandbox": SANDBOX,
        });
        if let Some(model) = &open.model {
            params["model"] = json!(model);
        }
        if let Some(instructions) = instructions {
            params["developerInstructions"] = json!(instructions);
        }
        if let Some(effort) = &open.effort {
            params["config"] = json!({"model_reasoning_effort": effort});
        }
        if let Some(tools) = &open.tools
            && !tools.is_empty()
            && open.resume.is_none()
        {
            // One namespace, the server's name, so a tool may load on
            // demand (Codex defers only namespaced tools).
            params["dynamicTools"] = json!([{
                "type": "namespace",
                "name": tools.name,
                "description": tools
                    .instructions
                    .clone()
                    .unwrap_or_else(|| "Coder's own tools.".to_owned()),
                "tools": tools
                    .tools()
                    .iter()
                    .map(|tool| json!({
                        "type": "function",
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.schema,
                        "deferLoading": !tool.always_load,
                    }))
                    .collect::<Vec<_>>(),
            }]);
        }
        self.tools.clone_from(&open.tools);
        let result = match &open.resume {
            Some(thread) => {
                params["threadId"] = json!(thread);
                self.request("thread/resume", params, emit).await?
            }
            None => self.request("thread/start", params, emit).await?,
        };
        let thread = result["thread"]["id"]
            .as_str()
            .ok_or("Codex opened no thread.")?
            .to_owned();
        if let Some(model) = result["model"].as_str() {
            self.model = Some(model.to_owned());
            emit(Output::Model(model.to_owned()));
        }
        self.thread = Some(thread.clone());
        emit(Output::Thread(thread.clone()));
        Ok(thread)
    }

    /// Runs one turn on the open thread: `prompt`, then whatever the `Stop`
    /// hooks send the agent back for, until a turn ends with no block.
    /// `control` stops or steers it.
    ///
    /// # Errors
    ///
    /// No thread is open, a `UserPromptSubmit` hook refused the message, or
    /// Codex went away. A failed or stopped turn is an [`Ended`] with its
    /// status.
    pub async fn turn(
        &mut self,
        prompt: &str,
        model: Option<&str>,
        mut control: Option<&mut UnboundedReceiver<Control>>,
        emit: &mut dyn FnMut(Output),
    ) -> Result<Ended, String> {
        let thread = self.thread.clone().ok_or("Codex has no thread open.")?;
        let submitted = self
            .hooks
            .fire(Call::new(
                Event::UserPromptSubmit,
                json!({"prompt": prompt}),
            ))
            .await;
        if let Permission::Deny { reason } = &submitted.permission {
            return Err(reason.clone());
        }
        let mut text = match context_text(&submitted.context) {
            Some(context) => format!("{context}\n\n{prompt}"),
            None => prompt.to_owned(),
        };
        let mut continued = 0;
        loop {
            let mut ended = self
                .one_turn(&thread, &text, model, control.as_deref_mut(), emit)
                .await?;
            ended.continued = continued;
            if ended.status != "completed" {
                return Ok(ended);
            }
            let stopped = self
                .hooks
                .fire(Call::new(
                    Event::Stop,
                    json!({"last_assistant_message": ended.text, "stop_hook_active": continued > 0}),
                ))
                .await;
            match stopped.stop {
                Stop::Block { reason } => {
                    continued += 1;
                    text = reason;
                }
                Stop::Continue | Stop::End => return Ok(ended),
            }
        }
    }

    async fn one_turn(
        &mut self,
        thread: &str,
        text: &str,
        model: Option<&str>,
        mut control: Option<&mut UnboundedReceiver<Control>>,
        emit: &mut dyn FnMut(Output),
    ) -> Result<Ended, String> {
        let mut params = json!({
            "threadId": thread,
            "input": [{"type": "text", "text": text}],
        });
        if let Some(model) = model {
            params["model"] = json!(model);
        }
        let started = self.request("turn/start", params, emit).await?;
        let turn = started["turn"]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut state = Turn {
            thread: thread.to_owned(),
            turn,
            ..Turn::default()
        };
        loop {
            let message = match control.as_deref_mut() {
                Some(control) => {
                    tokio::select! {
                        message = read(&mut self.reader) => message?,
                        asked = control.recv() => {
                            match asked {
                                Some(Control::Interrupt) => {
                                    let params = json!({"threadId": state.thread, "turnId": state.turn});
                                    self.send_request("turn/interrupt", params).await?;
                                }
                                Some(Control::Steer(text)) => self.steer(&state, &text).await?,
                                None => {}
                            }
                            continue;
                        }
                    }
                }
                None => read(&mut self.reader).await?,
            };
            if let Some(ended) = self.handle(message, Some(&mut state), emit).await? {
                return Ok(ended);
            }
        }
    }

    /// Adds `text` to the running turn.
    async fn steer(&mut self, turn: &Turn, text: &str) -> Result<(), String> {
        let params = json!({
            "threadId": turn.thread,
            "expectedTurnId": turn.turn,
            "input": [{"type": "text", "text": text}],
        });
        self.send_request("turn/steer", params).await.map(|_| ())
    }

    /// Ends the session: the `SessionEnd` hooks fire and Codex exits.
    pub async fn close(mut self) {
        let _ = self
            .hooks
            .fire(Call::new(Event::SessionEnd, json!({"reason": "close"})))
            .await;
        let _ = self.writer.shutdown().await;
        if let Some(mut child) = self.child.take() {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
            let _ = child.start_kill();
        }
    }

    async fn write(&mut self, message: &Value) -> Result<(), String> {
        let mut line = message.to_string();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .await
            .map_err(|error| format!("Codex stopped reading: {error}"))?;
        self.writer
            .flush()
            .await
            .map_err(|error| format!("Codex stopped reading: {error}"))
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let mut message = json!({"jsonrpc": "2.0", "method": method});
        if !params.is_null() {
            message["params"] = params;
        }
        self.write(&message).await
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<i64, String> {
        self.next_id += 1;
        let id = self.next_id;
        self.write(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await?;
        Ok(id)
    }

    /// Sends a request and reads until its answer, handling what comes
    /// before it.
    async fn request(
        &mut self,
        method: &str,
        params: Value,
        emit: &mut dyn FnMut(Output),
    ) -> Result<Value, String> {
        let id = self.send_request(method, params).await?;
        loop {
            let message = read(&mut self.reader).await?;
            if message.get("method").is_none() && message["id"] == json!(id) {
                if let Some(error) = message.get("error") {
                    return Err(format!(
                        "Codex refused {method}: {}",
                        error["message"].as_str().unwrap_or("no reason given")
                    ));
                }
                return Ok(message["result"].clone());
            }
            self.handle(message, None, emit).await?;
        }
    }

    /// One message from Codex. Returns the turn's end when it ended.
    async fn handle(
        &mut self,
        message: Value,
        mut turn: Option<&mut Turn>,
        emit: &mut dyn FnMut(Output),
    ) -> Result<Option<Ended>, String> {
        let Some(method) = message["method"].as_str().map(str::to_owned) else {
            // An answer to a request we don't wait on (an interrupt, a
            // steer): nothing to do.
            return Ok(None);
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        if let Some(id) = message.get("id").cloned() {
            let (result, said) = self.answer(&method, &params, emit).await;
            let reply = match result {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err(error) => {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": error}})
                }
            };
            self.write(&reply).await?;
            // The agent reads why in the same turn, as on Claude Code.
            if let (Some(turn), Some(said)) = (turn.as_deref(), said) {
                self.steer(turn, &said).await?;
            }
            return Ok(None);
        }
        match method.as_str() {
            "item/started" => {
                let item = &params["item"];
                if let Some(id) = item["id"].as_str() {
                    self.items.insert(id.to_owned(), item.clone());
                }
                if let Some((name, input)) = tool_of(item) {
                    emit(Output::Tool {
                        name,
                        input,
                        output: Value::Null,
                        running: true,
                    });
                }
                match item["type"].as_str() {
                    Some("reasoning") => {
                        if let Some(turn) = turn.as_deref_mut() {
                            turn.thinking = true;
                        }
                        emit(Output::Thinking(true));
                    }
                    // A new message reads as a new paragraph.
                    Some("agentMessage") => {
                        if let Some(turn) = turn.as_deref_mut()
                            && !turn.said.is_empty()
                        {
                            turn.said.push_str("\n\n");
                            emit(Output::Text("\n\n".into()));
                        }
                    }
                    _ => {}
                }
            }
            "item/agentMessage/delta" => {
                if let (Some(delta), Some(turn)) = (params["delta"].as_str(), turn.as_deref_mut()) {
                    if turn.thinking {
                        turn.thinking = false;
                        emit(Output::Thinking(false));
                    }
                    turn.said.push_str(delta);
                    turn.streamed = true;
                    emit(Output::Text(delta.to_owned()));
                }
            }
            "item/completed" => {
                let item = params["item"].clone();
                if let Some(id) = item["id"].as_str() {
                    self.items.remove(id);
                }
                match item["type"].as_str() {
                    Some("agentMessage") => {
                        if let Some(turn) = turn.as_deref_mut() {
                            let text = item["text"].as_str().unwrap_or_default();
                            if item["phase"] == "final_answer" {
                                turn.final_answer = Some(text.to_owned());
                            }
                            // A message that never streamed is said whole.
                            if !std::mem::take(&mut turn.streamed) {
                                turn.said.push_str(text);
                                emit(Output::Text(text.to_owned()));
                            }
                        }
                    }
                    Some("reasoning") => emit(Output::Thinking(false)),
                    _ => {}
                }
                if let Some((name, input)) = tool_of(&item) {
                    emit(Output::Tool {
                        name,
                        input,
                        output: tool_output(&item),
                        running: false,
                    });
                }
                // Seen after the fact: context is steered into the turn. A
                // command or a patch reaches the hooks as it did before it
                // ran.
                if let Some(call) = self.hook_call(&item) {
                    let failed = matches!(item["status"].as_str(), Some("failed" | "declined"));
                    let tool = call.tool.clone().unwrap_or_default();
                    let applied = self
                        .hooks
                        .fire(Call::tool(
                            Event::PostToolUse,
                            &tool,
                            json!({"tool_input": call.input, "tool_response": tool_output(&item), "failed": failed}),
                        ))
                        .await;
                    if let Some(turn) = turn.as_deref_mut() {
                        self.after(turn, &applied, emit).await?;
                    }
                }
            }
            "thread/compacted" => {
                let applied = self
                    .hooks
                    .fire(Call::new(Event::PostCompact, params.clone()))
                    .await;
                if let Some(turn) = turn.as_deref_mut() {
                    self.after(turn, &applied, emit).await?;
                }
            }
            "thread/tokenUsage/updated" => {
                self.usage = params["tokenUsage"].clone();
                emit(Output::Usage(self.usage.clone()));
            }
            "account/rateLimits/updated" => emit(Output::RateLimits(params["rateLimits"].clone())),
            "model/rerouted" => {
                if let Some(model) = params["toModel"]
                    .as_str()
                    .or_else(|| params["model"].as_str())
                {
                    self.model = Some(model.to_owned());
                    emit(Output::Model(model.to_owned()));
                }
            }
            "error" => {
                if let Some(turn) = turn.as_deref_mut() {
                    let error = &params["error"];
                    turn.error = error["message"]
                        .as_str()
                        .or_else(|| params["message"].as_str())
                        .map(str::to_owned);
                }
            }
            "turn/completed" => {
                if let Some(turn) = turn {
                    let done = &params["turn"];
                    if done["id"].as_str().is_some_and(|id| id != turn.turn) {
                        return Ok(None);
                    }
                    if turn.thinking {
                        emit(Output::Thinking(false));
                    }
                    let error = done["error"]["message"]
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| turn.error.clone());
                    return Ok(Some(Ended {
                        thread: turn.thread.clone(),
                        turn: turn.turn.clone(),
                        status: done["status"].as_str().unwrap_or("completed").to_owned(),
                        text: turn
                            .final_answer
                            .clone()
                            .unwrap_or_else(|| turn.said.trim_end().to_owned()),
                        model: self.model.clone(),
                        usage: self.usage.clone(),
                        error,
                        continued: 0,
                    }));
                }
            }
            _ => {}
        }
        Ok(None)
    }

    /// After an event seen once it happened: its context is steered into
    /// the running turn, and an end stops the turn.
    async fn after(
        &mut self,
        turn: &Turn,
        applied: &Applied,
        emit: &mut dyn FnMut(Output),
    ) -> Result<(), String> {
        if !applied.context.is_empty() || applied.stop != Stop::Continue {
            emit(Output::Gated(Box::new(applied.clone())));
        }
        if let Some(text) = context_text(&applied.context) {
            self.steer(turn, &text).await?;
        }
        if applied.stop == Stop::End {
            let params = json!({"threadId": turn.thread, "turnId": turn.turn});
            self.send_request("turn/interrupt", params).await?;
        }
        Ok(())
    }

    /// The answer to one of Codex's requests: an approval through the
    /// hooks; anything Coder doesn't take is refused.
    /// Also what the agent should read about it, if anything.
    async fn answer(
        &mut self,
        method: &str,
        params: &Value,
        emit: &mut dyn FnMut(Output),
    ) -> (Result<Value, String>, Option<String>) {
        let legacy = matches!(method, "execCommandApproval" | "applyPatchApproval");
        let call = match method {
            "item/commandExecution/requestApproval" | "execCommandApproval" => {
                command_call(params, &self.cwd)
            }
            "item/fileChange/requestApproval" | "applyPatchApproval" => {
                let item = params["itemId"]
                    .as_str()
                    .and_then(|id| self.items.get(id))
                    .cloned()
                    .unwrap_or(Value::Null);
                patch_call(params, &item)
            }
            "item/tool/call" => return (Ok(self.tool_call(params, emit).await), None),
            "item/tool/requestUserInput" | "mcpServer/elicitation/request" => {
                return (
                    Err(
                        "Coder answers no questions from inside a turn; ask the person in your reply."
                            .into(),
                    ),
                    None,
                );
            }
            other => return (Err(format!("Coder doesn't take {other}.")), None),
        };
        let applied = self.hooks.fire(call).await;
        emit(Output::Gated(Box::new(applied.clone())));
        let decision = match (&applied.permission, &applied.stop) {
            (Permission::Deny { .. }, Stop::End) => "cancel",
            (Permission::Deny { .. } | Permission::Ask { .. }, _) => "decline",
            (Permission::Allow | Permission::NoOpinion, _) => "accept",
        };
        let mut said = Vec::new();
        if let Permission::Deny { reason } | Permission::Ask { step: reason } = &applied.permission
        {
            said.push(reason.clone());
        }
        if let Some(context) = context_text(&applied.context) {
            said.push(context);
        }
        let said = (!said.is_empty() && decision != "cancel").then(|| said.join("\n\n"));
        let result = if legacy {
            json!({"decision": match decision {
                "accept" => "approved",
                "cancel" => "abort",
                _ => "denied",
            }})
        } else {
            json!({"decision": decision})
        };
        (Ok(result), said)
    }
}

impl Codex {
    /// A finished command or patch as the hooks saw it before it ran.
    fn hook_call(&self, item: &Value) -> Option<Call> {
        match item["type"].as_str()? {
            "commandExecution" => {
                let mut call = command_call(item, &self.cwd);
                call.input["commandActions"] = item["commandActions"].clone();
                Some(call)
            }
            "fileChange" => Some(patch_call(&Value::Null, item)),
            _ => None,
        }
    }

    /// One of Coder's tools, called by Codex: through the hooks (as
    /// `mcp__coder__<name>`, the name it has on Claude Code), then the tool.
    async fn tool_call(&mut self, params: &Value, emit: &mut dyn FnMut(Output)) -> Value {
        let name = params["tool"].as_str().unwrap_or_default().to_owned();
        let arguments = params["arguments"].clone();
        let Some(tools) = self.tools.clone() else {
            return tool_reply(false, &format!("There is no tool named {name}."));
        };
        let qualified = format!("mcp__{}__{name}", tools.name);
        let pre = self
            .hooks
            .fire(Call::tool(Event::PreToolUse, &qualified, arguments.clone()))
            .await;
        if !matches!(pre.permission, Permission::Allow | Permission::NoOpinion) {
            emit(Output::Gated(Box::new(pre.clone())));
        }
        if let Permission::Deny { reason } | Permission::Ask { step: reason } = &pre.permission {
            return tool_reply(false, reason);
        }
        emit(Output::Tool {
            name: name.clone(),
            input: pre.input.clone(),
            output: Value::Null,
            running: true,
        });
        let result = tools.call(&name, pre.input.clone()).await;
        let value = result.to_value();
        emit(Output::Tool {
            name: name.clone(),
            input: pre.input.clone(),
            output: value.clone(),
            running: false,
        });
        let post = self
            .hooks
            .fire(Call::tool(
                Event::PostToolUse,
                &qualified,
                json!({"tool_input": pre.input, "tool_response": value, "failed": result.is_error}),
            ))
            .await;
        let mut text = result
            .content
            .iter()
            .filter_map(|content| match content {
                claude_agent_sdk::ToolContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        for context in pre.context.iter().chain(&post.context) {
            text.push_str(&format!("\n[{}] {}", context.source, context.text));
        }
        tool_reply(!result.is_error, &text)
    }
}

/// A dynamic tool's answer to Codex.
fn tool_reply(success: bool, text: &str) -> Value {
    json!({"success": success, "contentItems": [{"type": "inputText", "text": text}]})
}

/// A turn as it runs.
#[derive(Debug, Default)]
struct Turn {
    thread: String,
    turn: String,
    said: String,
    final_answer: Option<String>,
    thinking: bool,
    /// The message under way arrived in parts.
    streamed: bool,
    error: Option<String>,
}

/// Reads one message; a line that isn't JSON is skipped.
async fn read(reader: &mut Reader) -> Result<Value, String> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|error| format!("Codex's output broke off: {error}"))?;
        if read == 0 {
            return Err("Codex stopped before the turn ended.".into());
        }
        if let Ok(message) = serde_json::from_str::<Value>(line.trim()) {
            return Ok(message);
        }
    }
}

/// A command approval as the hooks see it: `Bash` with its command and
/// where it runs.
#[must_use]
pub fn command_call(params: &Value, cwd: &Path) -> Call {
    let command = match &params["command"] {
        Value::Array(words) => words
            .iter()
            .filter_map(Value::as_str)
            .map(quote)
            .collect::<Vec<_>>()
            .join(" "),
        other => other.as_str().unwrap_or_default().to_owned(),
    };
    let here = params["cwd"]
        .as_str()
        .map_or_else(|| cwd.to_path_buf(), PathBuf::from);
    let mut input = json!({"command": command, "cwd": here, "workdir": here});
    if let Some(reason) = params["reason"].as_str() {
        input["description"] = json!(reason);
    }
    Call::tool(Event::PreToolUse, COMMAND_TOOL, input)
}

/// A patch approval as the hooks see it: `apply_patch` with the patch's
/// files as patch text and Codex's own `changes`.
#[must_use]
pub fn patch_call(params: &Value, item: &Value) -> Call {
    // The item Codex started carries the changes; the legacy request
    // carries them itself, keyed by path.
    let changes: Vec<(String, String, String)> = match item["changes"].as_array() {
        Some(changes) => changes
            .iter()
            .map(|change| {
                (
                    change["path"].as_str().unwrap_or_default().to_owned(),
                    change["kind"]["type"]
                        .as_str()
                        .unwrap_or("update")
                        .to_owned(),
                    change["diff"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect(),
        None => params["fileChanges"]
            .as_object()
            .map(|files| {
                files
                    .iter()
                    .map(|(path, change)| {
                        let kind = change["type"].as_str().unwrap_or("update").to_owned();
                        let diff = change["unified_diff"]
                            .as_str()
                            .or_else(|| change["content"].as_str())
                            .unwrap_or_default()
                            .to_owned();
                        (path.clone(), kind, diff)
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let mut patch = String::from("*** Begin Patch\n");
    for (path, kind, diff) in &changes {
        let head = match kind.as_str() {
            "add" => "*** Add File: ",
            "delete" => "*** Delete File: ",
            _ => "*** Update File: ",
        };
        patch.push_str(head);
        patch.push_str(path);
        patch.push('\n');
        patch.push_str(diff);
        if !diff.ends_with('\n') && !diff.is_empty() {
            patch.push('\n');
        }
    }
    patch.push_str("*** End Patch");
    let mut input = json!({
        "input": patch,
        "changes": changes
            .iter()
            .map(|(path, kind, _)| json!({"path": path, "kind": kind}))
            .collect::<Vec<_>>(),
    });
    if let Some(root) = params["grantRoot"].as_str() {
        input["grant_root"] = json!(root);
    }
    Call::tool(Event::PreToolUse, PATCH_TOOL, input)
}

/// A started or finished item as a tool call the transcript shows: a
/// command as `Run`, a patch as `Edit`, an MCP or dynamic tool by its name.
fn tool_of(item: &Value) -> Option<(String, Value)> {
    match item["type"].as_str()? {
        "commandExecution" => Some((
            "Run".into(),
            json!({"command": unwrap_display(item["command"].as_str().unwrap_or_default())}),
        )),
        "fileChange" => Some((
            "Edit".into(),
            json!({"changes": item["changes"].as_array().map(|changes| changes
                .iter()
                .map(|change| json!({"path": change["path"], "kind": change["kind"]["type"]}))
                .collect::<Vec<_>>())}),
        )),
        "mcpToolCall" => Some((
            item["tool"].as_str().unwrap_or("tool").to_owned(),
            item["arguments"].clone(),
        )),
        "webSearch" => Some(("WebSearch".into(), json!({"query": item["query"]}))),
        _ => None,
    }
}

fn tool_output(item: &Value) -> Value {
    match item["type"].as_str() {
        Some("commandExecution") => json!({
            "exit": item["exitCode"],
            "output": item["aggregatedOutput"],
            "status": item["status"],
        }),
        Some("fileChange") => json!({"status": item["status"]}),
        Some("mcpToolCall") => {
            json!({"result": item["result"], "error": item["error"], "status": item["status"]})
        }
        _ => json!({"status": item["status"]}),
    }
}

/// `/bin/zsh -lc 'cargo test'` as the person reads it: `cargo test`.
#[must_use]
pub fn unwrap_display(command: &str) -> String {
    for shell in [
        "/bin/zsh -lc ",
        "/bin/bash -lc ",
        "bash -lc ",
        "zsh -lc ",
        "/bin/sh -c ",
    ] {
        if let Some(rest) = command.strip_prefix(shell) {
            let rest = rest.trim();
            return one_word(rest).unwrap_or_else(|| rest.to_owned());
        }
    }
    command.to_owned()
}

/// `text` as one shell word with its quotes taken off, when it is one.
fn one_word(text: &str) -> Option<String> {
    let mut word = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => loop {
                match chars.next()? {
                    '\'' => break,
                    c => word.push(c),
                }
            },
            '"' => loop {
                match chars.next()? {
                    '"' => break,
                    '\\' => word.push(chars.next()?),
                    c => word.push(c),
                }
            },
            '\\' => word.push(chars.next()?),
            c if c.is_whitespace() => return None,
            c => word.push(c),
        }
    }
    Some(word)
}

/// One shell word, quoted when it needs it.
fn quote(word: &str) -> String {
    if !word.is_empty()
        && word.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '/' | '.' | '-' | '_' | '=' | ':' | ',' | '@' | '+')
        })
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

fn context_text(context: &[Context]) -> Option<String> {
    super::claude::context_text(context)
}

fn join(first: Option<&str>, second: Option<&str>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}\n\n{second}")),
        (Some(text), None) | (None, Some(text)) => Some(text.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
