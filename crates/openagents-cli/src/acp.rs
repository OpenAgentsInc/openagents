//! An Agent Client Protocol client, and the Devin harness built on it.
//!
//! What was here built one `initialize` request as a struct, never sent it,
//! and never spoke to anything. No process was started, no socket or pipe was
//! opened, and `handle_response` was called by nothing. This is the client
//! OpenAgentsInc/openagents#72 asks for: it starts the agent, speaks the
//! protocol over its stdio, and returns what the agent said.
//!
//! Devin's print mode (`devin -p`) writes nothing until the very end, so a
//! child doing four minutes of work reports nothing for four minutes and a
//! reader cannot tell it from a hang. `devin acp` is the same agent as an ACP
//! server over stdio, and it streams: `tool_call` with a title,
//! `tool_call_update` with a status, `usage_update` with token counts, and
//! `agent_message_chunk` with the answer as it is written.
//!
//! Newline-delimited JSON-RPC, one server per child. A shared server would
//! save a process and cost a lifecycle nobody asked for: one crash would take
//! every child with it.
//!
//! Devin logs heavily to stderr and none of it is protocol. It is drained and
//! dropped rather than parsed.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::sync::watch;

use crate::signals::stop_tree;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

/// How much the child is allowed to do without being asked.
///
/// The names are this CLI's; the wire carries the agent's own. Devin calls the
/// permissive one `bypass`. A build of the agent that does not know a mode is
/// not a reason to lose the child, so setting it is best effort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionMode {
    Dangerous,
    Prompt,
    ReadOnly,
}

impl PermissionMode {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "dangerous" | "bypass" => Some(PermissionMode::Dangerous),
            "prompt" | "default" | "ask" => Some(PermissionMode::Prompt),
            "read-only" | "readonly" => Some(PermissionMode::ReadOnly),
            _ => None,
        }
    }

    /// The mode id sent in `session/set_mode` for this agent.
    ///
    /// The names are this CLI's; the wire carries the agent's own. Devin,
    /// OpenCode, and Gemini use `bypass` / `default` / `read-only`. The Claude
    /// adapter uses `bypassPermissions` / `default` / `plan`.
    pub fn mode_id(&self, agent_id: &str) -> &'static str {
        match agent_id {
            "claude" => match self {
                PermissionMode::Dangerous => "bypassPermissions",
                PermissionMode::Prompt => "default",
                PermissionMode::ReadOnly => "plan",
            },
            _ => match self {
                PermissionMode::Dangerous => "bypass",
                PermissionMode::Prompt => "default",
                PermissionMode::ReadOnly => "read-only",
            },
        }
    }
}

/// What a running ACP child reports as it works.
#[derive(Debug, Clone)]
pub enum AcpEvent {
    Session {
        id: String,
    },
    /// A tool the agent ran. `title` is Devin's own phrase — "Ran ls", "Read
    /// src/a.ts" — so it is the activity rather than a name to look up.
    Tool {
        kind: String,
        title: String,
    },
    Tokens {
        input: u64,
        output: u64,
    },
    /// A piece of the answer, as it is written.
    Text {
        chunk: String,
    },
}

/// The line a parent `delegate` box shows for one ACP event.
///
/// `Text` is omitted: the caller buffers those chunks into whole lines so a
/// token stream does not become one box row per piece. Session, tool, and
/// token events become one line each.
pub fn acp_event_subagent_line(event: &AcpEvent) -> Option<String> {
    match event {
        AcpEvent::Session { id } => Some(format!("session {id}")),
        AcpEvent::Tool { kind, title } => {
            let title = title.trim();
            if title.is_empty() {
                Some(kind.clone())
            } else {
                Some(title.to_string())
            }
        }
        AcpEvent::Tokens { input, output } => Some(format!("{input} in / {output} out")),
        AcpEvent::Text { .. } => None,
    }
}

/// What the agent is asking permission to do.
///
/// The wire shape is ACP's `session/request_permission` `toolCall`: a `kind`
/// naming the class of action, a `title` the agent wrote, and the tool's raw
/// input. A caller that runs the agent under a policy needs all three — the
/// command a shell tool wants to run is in `raw_input`, not in `kind`.
#[derive(Debug, Clone)]
pub struct PermissionQuery {
    pub kind: String,
    pub title: String,
    pub raw_input: serde_json::Value,
}

/// Whether the agent may do what it just asked to do.
///
/// A delegated agent has nobody to ask, so somebody has to answer for it. With
/// no gate the harness answers "yes" to everything, which is the right answer
/// for a child the reader started themselves and the wrong one for a child a
/// server asked for: see [`AcpHarness::permission`].
pub type PermissionGate = Arc<dyn Fn(&PermissionQuery) -> bool + Send + Sync>;

/// A JSON-RPC request the agent sends back to its client, other than a
/// permission request. `None` answers `method not found`.
pub type ReverseHandler =
    Arc<dyn Fn(&str, &serde_json::Value) -> Option<serde_json::Value> + Send + Sync>;

/// How an ACP child is started.
#[derive(Clone)]
pub struct AcpHarness {
    /// The binary. Defaults to `devin`; a test points it at a stand-in.
    pub command: String,
    /// The subcommand that puts it in ACP mode.
    pub args: Vec<String>,
    /// Catalog id this child is started as. `session/set_mode` uses this
    /// agent's own mode ids. Empty keeps the default (Devin-shaped) ids.
    pub agent_id: String,
    pub mode: Option<PermissionMode>,
    /// Who decides what the agent may do.
    ///
    /// `None` allows whatever the agent asks, which is what a child the reader
    /// started in their own terminal should get. A delegated child gets a gate
    /// carrying the machine's policy, and a refused request is answered with
    /// the agent's own `reject*` option rather than left hanging.
    pub permission: Option<PermissionGate>,
    /// Reverse requests other than `session/request_permission` — the delegated
    /// `git/push` among them. Unset means the agent is told the method does not
    /// exist, which is the honest answer for a client that cannot serve it.
    pub on_request: Option<ReverseHandler>,
    /// Reattach to this session with `session/load` instead of opening a new
    /// one. Refused when the agent does not report the `loadSession`
    /// capability: silently opening a fresh session would look like a resume
    /// and lose everything the earlier one knew.
    pub resume_session_id: Option<String>,
    /// The child's whole environment. `None` inherits this process's, which is
    /// right for a child the reader started and wrong for a delegated one —
    /// that would hand a server-requested agent this process's credentials.
    pub env: Option<Vec<(String, String)>>,
}

impl std::fmt::Debug for AcpHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The hooks are closures and the environment may carry a credential,
        // so neither is printed. Everything a reader needs to identify the
        // child is.
        f.debug_struct("AcpHarness")
            .field("command", &self.command)
            .field("args", &self.args)
            .field("agent_id", &self.agent_id)
            .field("mode", &self.mode)
            .field("gated", &self.permission.is_some())
            .field("reverse_handler", &self.on_request.is_some())
            .field("resume_session_id", &self.resume_session_id)
            .field("environment_scrubbed", &self.env.is_some())
            .finish()
    }
}

impl Default for AcpHarness {
    fn default() -> Self {
        Self {
            command: "devin".to_string(),
            args: vec!["acp".to_string()],
            agent_id: String::new(),
            mode: Some(PermissionMode::Dangerous),
            permission: None,
            on_request: None,
            resume_session_id: None,
            env: None,
        }
    }
}

/// Everything one run produced, for a caller that needs more than the answer.
#[derive(Debug, Clone, Default)]
pub struct AcpOutcome {
    /// What the agent said, and the tools it named on the way.
    pub answer: String,
    /// The ACP session, whether opened or reattached. A caller checkpoints
    /// this so the next run can resume it.
    pub session_id: String,
    /// The agent's own `stopReason` from `session/prompt` — `end_turn`,
    /// `cancelled`, `refusal`, and so on. Empty when the agent sent none.
    pub stop_reason: String,
}

/// How long the client waits for the agent to answer one request.
///
/// `session/prompt` is the whole turn, so this is the child's own ceiling
/// rather than a network timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(900);

/// How long the client waits for the agent's first protocol line.
///
/// An agent that needs a credential and is waiting for a terminal never writes
/// one, and without this the child hangs for as long as the reader leaves it.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a run ended other than by answering.
#[derive(Debug)]
pub enum AcpFailure {
    /// The binary is not on `PATH`, or would not start.
    Unstartable(String),
    /// The agent refused, exited early, or broke the protocol.
    Refused(String),
    /// The reader stopped the fan-out.
    Cancelled,
}

impl std::fmt::Display for AcpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpFailure::Unstartable(why) => write!(f, "{why}"),
            AcpFailure::Refused(why) => write!(f, "{why}"),
            AcpFailure::Cancelled => write!(f, "stopped before finishing"),
        }
    }
}

/// Insert `--always-approve` after `agent` and before `stdio`.
///
/// Grok takes agent flags between those two tokens. A trailing `--always-approve`
/// is a stdio-mode flag it does not read. Idempotent if the flag or `--yolo`
/// is already present.
pub fn with_always_approve_before_stdio(args: &[String]) -> Vec<String> {
    if args
        .iter()
        .any(|arg| arg == "--always-approve" || arg == "--yolo")
    {
        return args.to_vec();
    }
    let Some(stdio_at) = args.iter().position(|arg| arg == "stdio") else {
        return args.to_vec();
    };
    let Some(agent_at) = args[..stdio_at].iter().rposition(|arg| arg == "agent") else {
        return args.to_vec();
    };
    let mut out = args.to_vec();
    out.insert(agent_at + 1, "--always-approve".to_string());
    out
}

fn grok_stdio_argv(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "agent") && args.iter().any(|arg| arg == "stdio")
}

fn advertised_auth_method_ids(initialized: &serde_json::Value) -> Vec<String> {
    initialized
        .get("authMethods")
        .and_then(|value| value.as_array())
        .map(|methods| {
            methods
                .iter()
                .filter_map(|method| {
                    method
                        .get("id")
                        .and_then(|id| id.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_interactive_auth_method(id: &str) -> bool {
    id == "grok.com" || id == "oidc"
}

fn select_auth_method_id(
    initialized: &serde_json::Value,
    methods: &[String],
    has_api_key: bool,
) -> Result<String, AcpFailure> {
    let preferred = initialized
        .get("_meta")
        .and_then(|meta| meta.get("defaultAuthMethodId"))
        .and_then(|id| id.as_str())
        .map(str::to_string);
    let candidates = preferred
        .into_iter()
        .chain(methods.iter().cloned())
        .filter(|id| methods.is_empty() || methods.iter().any(|method| method == id));

    let mut saw_interactive = false;
    for id in candidates {
        if is_interactive_auth_method(&id) {
            saw_interactive = true;
            continue;
        }
        if id == "xai.api_key" && !has_api_key {
            continue;
        }
        return Ok(id);
    }

    if saw_interactive {
        return Err(AcpFailure::Refused(format!(
            "Grok needs an interactive login ({}). Run `grok login` or set XAI_API_KEY for delegate.",
            methods.join(", ")
        )));
    }
    Err(AcpFailure::Refused(
        "Grok is not signed in. Run `grok login` or set XAI_API_KEY.".to_string(),
    ))
}

impl AcpHarness {
    fn spawn_args(&self) -> Vec<String> {
        let unattended = matches!(self.mode, None | Some(PermissionMode::Dangerous));
        if unattended && grok_stdio_argv(&self.args) {
            with_always_approve_before_stdio(&self.args)
        } else {
            self.args.clone()
        }
    }

    fn is_grok_agent(&self) -> bool {
        matches!(self.mode_agent_id(), "grok" | "grok-build")
    }

    fn child_has_xai_api_key(&self) -> bool {
        if let Some(environment) = &self.env {
            return environment
                .iter()
                .any(|(key, value)| key == "XAI_API_KEY" && !value.is_empty());
        }
        std::env::var("XAI_API_KEY")
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    }

    /// Grok requires `authenticate` after `initialize`. Agents that advertise
    /// no methods keep the Devin/Claude path: skip the round-trip.
    async fn authenticate_if_needed<F>(
        &self,
        initialized: &serde_json::Value,
        stdin: &mut ChildStdin,
        lines: &mut tokio::io::Lines<BufReader<ChildStdout>>,
        seq: &mut u64,
        answer: &mut String,
        on_event: &mut F,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<(), AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let methods = advertised_auth_method_ids(initialized);
        if methods.is_empty() && !self.is_grok_agent() {
            return Ok(());
        }
        let method_id = select_auth_method_id(initialized, &methods, self.child_has_xai_api_key())?;
        request(
            stdin,
            lines,
            seq,
            "authenticate",
            serde_json::json!({
                "methodId": method_id,
                "_meta": { "headless": true }
            }),
            HANDSHAKE_TIMEOUT,
            answer,
            on_event,
            cancel,
            self,
        )
        .await?;
        Ok(())
    }

    /// Agent id used for `session/set_mode`. An explicit catalog id wins;
    /// otherwise the Claude adapter binary is recognised from the command.
    fn mode_agent_id(&self) -> &str {
        if !self.agent_id.is_empty() {
            return &self.agent_id;
        }
        match Path::new(&self.command)
            .file_name()
            .and_then(|name| name.to_str())
        {
            Some("claude-agent-acp") => "claude",
            Some("grok") => "grok",
            _ => self.agent_id.as_str(),
        }
    }

    /// Run one prompt to completion and return what the agent said.
    ///
    /// `on_event` is called as the agent works, so a caller can stream rather
    /// than wait for the end. The child is spawned into a process group of its
    /// own and the group is killed on every exit path, including cancellation
    /// and a timeout — an agent that shells out must not outlive the run that
    /// started it.
    pub async fn run<F>(
        &self,
        prompt: &str,
        cwd: &Path,
        on_event: F,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<String, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        self.run_detailed(prompt, cwd, on_event, cancel)
            .await
            .map(|outcome| outcome.answer)
    }

    /// The same run, reporting the session it used and why it stopped.
    ///
    /// A delegated run needs both: the session id is what a later request
    /// resumes, and the stop reason is the difference between an agent that
    /// finished and one that refused.
    pub async fn run_detailed<F>(
        &self,
        prompt: &str,
        cwd: &Path,
        mut on_event: F,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<AcpOutcome, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let mut command = Command::new(&self.command);
        command
            .args(&self.spawn_args())
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own process group, so stopping the child stops what the child
        // started. `process_group` is a Unix extension and does not exist on
        // Windows, where the whole crate fails to compile if it is called
        // unconditionally.
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        if let Some(environment) = &self.env {
            command.env_clear().envs(environment.iter().cloned());
        }
        let mut child = command.spawn().map_err(|error| {
            AcpFailure::Unstartable(if error.kind() == std::io::ErrorKind::NotFound {
                format!("the `{}` command is not on PATH", self.command)
            } else {
                format!("the `{}` command would not start: {error}", self.command)
            })
        })?;

        // Devin's own logging. Not protocol, and there is a lot of it.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(_)) = lines.next_line().await {}
            });
        }

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let (Some(mut stdin), Some(stdout)) = (stdin, stdout) else {
            stop_tree(&mut child).await;
            return Err(AcpFailure::Refused(
                "the agent's standard streams could not be opened".to_string(),
            ));
        };
        let mut lines = BufReader::new(stdout).lines();

        let outcome = self
            .converse(prompt, cwd, &mut stdin, &mut lines, &mut on_event, cancel)
            .await;

        stop_tree(&mut child).await;
        outcome
    }

    /// The conversation itself: handshake, session, prompt, answer.
    async fn converse<F>(
        &self,
        prompt: &str,
        cwd: &Path,
        stdin: &mut ChildStdin,
        lines: &mut tokio::io::Lines<BufReader<ChildStdout>>,
        on_event: &mut F,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<AcpOutcome, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let mut seq: u64 = 0;
        let mut answer = String::new();

        let initialized = request(
            stdin,
            lines,
            &mut seq,
            "initialize",
            serde_json::json!({
                "protocolVersion": 1,
                "clientCapabilities": {"fs": {"readTextFile": false, "writeTextFile": false}}
            }),
            HANDSHAKE_TIMEOUT,
            &mut answer,
            on_event,
            cancel,
            self,
        )
        .await?;

        self.authenticate_if_needed(
            &initialized,
            stdin,
            lines,
            &mut seq,
            &mut answer,
            on_event,
            cancel,
        )
        .await?;

        let session_id = match &self.resume_session_id {
            // Reattach. An agent that cannot load a session must say so here
            // rather than have a fresh, empty session passed off as a resume:
            // the caller asked to continue work the new session has never
            // heard of.
            Some(resume) => {
                let loads = initialized
                    .get("agentCapabilities")
                    .and_then(|value| value.get("loadSession"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                if !loads {
                    return Err(AcpFailure::Refused(format!(
                        "the `{}` agent cannot reattach a session",
                        self.command
                    )));
                }
                request(
                    stdin,
                    lines,
                    &mut seq,
                    "session/load",
                    serde_json::json!({
                        "sessionId": resume,
                        "cwd": cwd.to_string_lossy(),
                        "mcpServers": [],
                    }),
                    REQUEST_TIMEOUT,
                    &mut answer,
                    on_event,
                    cancel,
                    self,
                )
                .await?;
                resume.clone()
            }
            None => {
                let opened = request(
                    stdin,
                    lines,
                    &mut seq,
                    "session/new",
                    serde_json::json!({"cwd": cwd.to_string_lossy(), "mcpServers": []}),
                    REQUEST_TIMEOUT,
                    &mut answer,
                    on_event,
                    cancel,
                    self,
                )
                .await?;
                opened
                    .get("sessionId")
                    .and_then(|v| v.as_str())
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| AcpFailure::Refused("the agent opened no session".to_string()))?
                    .to_string()
            }
        };
        on_event(AcpEvent::Session {
            id: session_id.clone(),
        });

        if let Some(mode) = &self.mode {
            // Best effort: a build of the agent without this mode should not
            // cost the child over the name of a permission setting.
            let _ = request(
                stdin,
                lines,
                &mut seq,
                "session/set_mode",
                serde_json::json!({"sessionId": session_id, "modeId": mode.mode_id(self.mode_agent_id())}),
                REQUEST_TIMEOUT,
                &mut answer,
                on_event,
                cancel,
                self,
            )
            .await;
        }

        let finished = request(
            stdin,
            lines,
            &mut seq,
            "session/prompt",
            serde_json::json!({
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": prompt}]
            }),
            REQUEST_TIMEOUT,
            &mut answer,
            on_event,
            cancel,
            self,
        )
        .await?;

        Ok(AcpOutcome {
            answer: answer.trim().to_string(),
            session_id,
            stop_reason: finished
                .get("stopReason")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
        })
    }
}

/// Send one request and pump the stream until its reply arrives.
///
/// Everything else that comes down the pipe while waiting is handled on the
/// way past: notifications become events, and a permission request is answered
/// here because a delegated child has nobody to ask. An unanswered permission
/// request hangs the agent for as long as the reader leaves it.
#[allow(clippy::too_many_arguments)]
async fn request<F>(
    stdin: &mut ChildStdin,
    lines: &mut tokio::io::Lines<BufReader<ChildStdout>>,
    seq: &mut u64,
    method: &str,
    params: serde_json::Value,
    limit: Duration,
    answer: &mut String,
    on_event: &mut F,
    cancel: &mut watch::Receiver<bool>,
    harness: &AcpHarness,
) -> Result<serde_json::Value, AcpFailure>
where
    F: FnMut(AcpEvent) + Send,
{
    *seq += 1;
    let id = *seq;
    let line = serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    write_line(stdin, &line).await?;

    let deadline = tokio::time::Instant::now() + limit;

    loop {
        let next = tokio::select! {
            biased;
            _ = cancel.changed() => return Err(AcpFailure::Cancelled),
            read = lines.next_line() => read,
            _ = tokio::time::sleep_until(deadline) => {
                return Err(AcpFailure::Refused(format!(
                    "the agent did not answer `{method}` within {}s", limit.as_secs()
                )));
            }
        };

        let raw = match next {
            Ok(Some(raw)) => raw,
            Ok(None) => {
                return Err(AcpFailure::Refused(
                    "the agent exited before it answered".to_string(),
                ));
            }
            Err(error) => {
                return Err(AcpFailure::Refused(format!(
                    "the agent's output could not be read: {error}"
                )));
            }
        };

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Not protocol. Agents write plain lines to stdout too.
        let Ok(message) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        // A reply to something asked for.
        let is_reply =
            message.get("id").and_then(|v| v.as_u64()).is_some() && message.get("method").is_none();
        if is_reply {
            if message.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                // The agent's own bytes, and `serde_json` does not escape
                // non-ASCII, so a refusal carrying an accent or an emoji across
                // byte 200 panicked here and took the whole run with it.
                // Floored to a character boundary, as the four cuts in
                // `28704f72ff` were.
                let text = serde_json::to_string(error).unwrap_or_default();
                let end = crate::tracker::floor_char_boundary(&text, 200);
                return Err(AcpFailure::Refused(format!(
                    "the agent refused `{method}`: {}",
                    &text[..end]
                )));
            }
            return Ok(message
                .get("result")
                .cloned()
                .unwrap_or(serde_json::json!({})));
        }

        handle_incoming(&message, stdin, answer, on_event, harness).await?;
    }
}

/// A notification, or a request from the agent.
async fn handle_incoming<F>(
    message: &serde_json::Value,
    stdin: &mut ChildStdin,
    answer: &mut String,
    on_event: &mut F,
    harness: &AcpHarness,
) -> Result<(), AcpFailure>
where
    F: FnMut(AcpEvent) + Send,
{
    let method = message.get("method").and_then(|v| v.as_str()).unwrap_or("");

    if method == "session/request_permission" {
        let Some(id) = message.get("id").and_then(|v| v.as_u64()) else {
            return Ok(());
        };
        let params = message
            .get("params")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let outcome = match &harness.permission {
            // Ungated: whatever the agent asks for, which is the answer for a
            // child the reader started themselves.
            None => match first_allow_option(&params) {
                Some(option) => serde_json::json!({"outcome": "selected", "optionId": option}),
                None => serde_json::json!({"outcome": "cancelled"}),
            },
            // Gated: the policy answers. A refusal picks the agent's own
            // `reject*` option so the agent learns it was denied and carries
            // on; cancelling would end the turn over one denied tool call.
            Some(gate) => {
                let query = permission_query(&params);
                let allowed = gate(&query);
                match option_of_kind(&params, if allowed { "allow" } else { "reject" }) {
                    Some(option) => serde_json::json!({"outcome": "selected", "optionId": option}),
                    None => serde_json::json!({"outcome": "cancelled"}),
                }
            }
        };
        write_line(
            stdin,
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"outcome": outcome}}),
        )
        .await?;
        return Ok(());
    }

    // Any other request the agent makes of its client. An unanswered request
    // hangs the agent, so every one gets a reply: the handler's, or the
    // JSON-RPC "method not found" that says this client does not serve it.
    if !method.is_empty() && method != "session/update" {
        if let Some(id) = message.get("id").and_then(|v| v.as_u64()) {
            let params = message
                .get("params")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let reply = match &harness.on_request {
                Some(handler) => handler(method, &params),
                None => None,
            };
            let answer_line = match reply {
                Some(result) => serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}),
                None => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": "method not found"},
                }),
            };
            write_line(stdin, &answer_line).await?;
            return Ok(());
        }
    }

    if method != "session/update" {
        return Ok(());
    }

    let update = message
        .get("params")
        .and_then(|p| p.get("update"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    match update.get("sessionUpdate").and_then(|v| v.as_str()) {
        Some("tool_call") => {
            on_event(AcpEvent::Tool {
                kind: update
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string(),
                title: update
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            });
        }
        Some("usage_update") => {
            let meta = update
                .get("_meta")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let input = meta
                .get("cognition.ai/inputTokens")
                .and_then(|v| v.as_u64());
            let output = meta
                .get("cognition.ai/outputTokens")
                .and_then(|v| v.as_u64());
            if let (Some(input), Some(output)) = (input, output) {
                on_event(AcpEvent::Tokens { input, output });
            }
        }
        Some("agent_message_chunk") => {
            if let Some(piece) = update
                .get("content")
                .and_then(|c| c.get("text"))
                .and_then(|v| v.as_str())
            {
                answer.push_str(piece);
                on_event(AcpEvent::Text {
                    chunk: piece.to_string(),
                });
            }
        }
        _ => {}
    }

    Ok(())
}

async fn write_line(stdin: &mut ChildStdin, value: &serde_json::Value) -> Result<(), AcpFailure> {
    let mut line = serde_json::to_string(value).unwrap_or_default();
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .await
        .map_err(|error| AcpFailure::Refused(format!("the agent stopped reading: {error}")))?;
    stdin
        .flush()
        .await
        .map_err(|error| AcpFailure::Refused(format!("the agent stopped reading: {error}")))
}

/// What the agent asked for, out of a `session/request_permission` payload.
///
/// Agents disagree about where the tool call lives — some nest it under
/// `toolCall`, some put `kind` and `title` at the top — so both are read. A
/// query that finds nothing is still a query, and a gate that sees an empty
/// kind should refuse rather than guess.
pub fn permission_query(params: &serde_json::Value) -> PermissionQuery {
    let empty = serde_json::json!({});
    let call = params.get("toolCall").unwrap_or(params);
    let text = |value: &serde_json::Value, name: &str| {
        value
            .get(name)
            .and_then(|found| found.as_str())
            .unwrap_or_default()
            .to_string()
    };
    PermissionQuery {
        kind: text(call, "kind"),
        title: text(call, "title"),
        raw_input: call
            .get("rawInput")
            .or_else(|| call.get("input"))
            .cloned()
            .unwrap_or(empty),
    }
}

/// The option whose `kind` starts with `prefix` — `allow` or `reject`.
pub fn option_of_kind(params: &serde_json::Value, prefix: &str) -> Option<String> {
    params
        .get("options")?
        .as_array()?
        .iter()
        .find(|option| {
            option
                .get("kind")
                .and_then(|value| value.as_str())
                .is_some_and(|kind| kind.starts_with(prefix))
        })
        .and_then(|option| option.get("optionId"))
        .and_then(|value| value.as_str())
        .map(String::from)
}

/// The option a permission request offers that lets the work continue.
pub fn first_allow_option(params: &serde_json::Value) -> Option<String> {
    let options = params.get("options")?.as_array()?;
    let named: Vec<&serde_json::Value> = options
        .iter()
        .filter(|option| option.get("optionId").and_then(|v| v.as_str()).is_some())
        .collect();
    let allow = named.iter().find(|option| {
        option
            .get("kind")
            .and_then(|v| v.as_str())
            .is_some_and(|kind| kind.starts_with("allow"))
    });
    allow
        .or(named.first())
        .and_then(|option| option.get("optionId"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_events_map_onto_subagent_lines() {
        assert_eq!(
            acp_event_subagent_line(&AcpEvent::Session {
                id: "sess_1".to_string()
            })
            .as_deref(),
            Some("session sess_1")
        );
        assert_eq!(
            acp_event_subagent_line(&AcpEvent::Tool {
                kind: "read".to_string(),
                title: "Read src/a.ts".to_string(),
            })
            .as_deref(),
            Some("Read src/a.ts")
        );
        assert_eq!(
            acp_event_subagent_line(&AcpEvent::Tool {
                kind: "read".to_string(),
                title: "  ".to_string(),
            })
            .as_deref(),
            Some("read")
        );
        assert_eq!(
            acp_event_subagent_line(&AcpEvent::Tokens {
                input: 12,
                output: 4
            })
            .as_deref(),
            Some("12 in / 4 out")
        );
        assert_eq!(
            acp_event_subagent_line(&AcpEvent::Text {
                chunk: "hello".to_string()
            }),
            None
        );
    }

    #[test]
    fn permission_mode_ids_are_translated_per_agent() {
        assert_eq!(PermissionMode::Dangerous.mode_id("devin"), "bypass");
        assert_eq!(PermissionMode::Prompt.mode_id("opencode"), "default");
        assert_eq!(PermissionMode::ReadOnly.mode_id("gemini"), "read-only");
        assert_eq!(PermissionMode::Dangerous.mode_id(""), "bypass");
        assert_eq!(
            PermissionMode::Dangerous.mode_id("claude"),
            "bypassPermissions"
        );
        assert_eq!(PermissionMode::Prompt.mode_id("claude"), "default");
        assert_eq!(PermissionMode::ReadOnly.mode_id("claude"), "plan");
    }

    #[test]
    fn claude_adapter_command_selects_claude_mode_ids() {
        let harness = AcpHarness {
            command: "/usr/local/bin/claude-agent-acp".to_string(),
            args: Vec::new(),
            ..AcpHarness::default()
        };
        assert_eq!(harness.mode_agent_id(), "claude");
        assert_eq!(
            PermissionMode::Dangerous.mode_id(harness.mode_agent_id()),
            "bypassPermissions"
        );
        let named = AcpHarness {
            command: "/tmp/stub-acp-agent".to_string(),
            agent_id: "claude".to_string(),
            ..AcpHarness::default()
        };
        assert_eq!(named.mode_agent_id(), "claude");
    }

    #[test]
    fn always_approve_sits_between_agent_and_stdio() {
        assert_eq!(
            with_always_approve_before_stdio(&["agent".to_string(), "stdio".to_string()]),
            vec![
                "agent".to_string(),
                "--always-approve".to_string(),
                "stdio".to_string()
            ]
        );
        assert_eq!(
            with_always_approve_before_stdio(&[
                "-y".to_string(),
                "@xai-official/grok@1.0.10".to_string(),
                "agent".to_string(),
                "stdio".to_string()
            ]),
            vec![
                "-y".to_string(),
                "@xai-official/grok@1.0.10".to_string(),
                "agent".to_string(),
                "--always-approve".to_string(),
                "stdio".to_string()
            ]
        );
        let already = vec![
            "agent".to_string(),
            "--always-approve".to_string(),
            "stdio".to_string(),
        ];
        assert_eq!(with_always_approve_before_stdio(&already), already);
        assert_eq!(
            with_always_approve_before_stdio(&["acp".to_string()]),
            vec!["acp".to_string()]
        );
    }

    #[test]
    fn dangerous_grok_harness_inserts_always_approve() {
        let harness = AcpHarness {
            command: "grok".to_string(),
            args: vec!["agent".to_string(), "stdio".to_string()],
            agent_id: "grok".to_string(),
            mode: Some(PermissionMode::Dangerous),
            ..AcpHarness::default()
        };
        assert_eq!(
            harness.spawn_args(),
            vec![
                "agent".to_string(),
                "--always-approve".to_string(),
                "stdio".to_string()
            ]
        );
        let prompt = AcpHarness {
            mode: Some(PermissionMode::Prompt),
            ..harness.clone()
        };
        assert_eq!(
            prompt.spawn_args(),
            vec!["agent".to_string(), "stdio".to_string()]
        );
    }

    #[test]
    fn grok_command_selects_the_grok_agent_id() {
        let harness = AcpHarness {
            command: "/usr/local/bin/grok".to_string(),
            args: grok_stdio_argv_for_test(),
            ..AcpHarness::default()
        };
        assert_eq!(harness.mode_agent_id(), "grok");
        assert!(harness.is_grok_agent());
    }

    fn grok_stdio_argv_for_test() -> Vec<String> {
        vec!["agent".to_string(), "stdio".to_string()]
    }

    #[test]
    fn auth_method_prefers_default_then_cached_token() {
        let initialized = serde_json::json!({
            "authMethods": [
                {"id": "grok.com"},
                {"id": "cached_token"},
                {"id": "xai.api_key"}
            ],
            "_meta": {"defaultAuthMethodId": "cached_token"}
        });
        let methods = advertised_auth_method_ids(&initialized);
        assert_eq!(
            select_auth_method_id(&initialized, &methods, false).unwrap(),
            "cached_token"
        );
    }

    #[test]
    fn auth_method_uses_api_key_when_the_env_has_one() {
        let initialized = serde_json::json!({
            "authMethods": [{"id": "xai.api_key"}, {"id": "cached_token"}]
        });
        let methods = advertised_auth_method_ids(&initialized);
        assert_eq!(
            select_auth_method_id(&initialized, &methods, true).unwrap(),
            "xai.api_key"
        );
        assert_eq!(
            select_auth_method_id(&initialized, &methods, false).unwrap(),
            "cached_token"
        );
    }

    #[test]
    fn auth_method_refuses_interactive_only_agents() {
        let initialized = serde_json::json!({
            "authMethods": [{"id": "grok.com"}, {"id": "oidc"}]
        });
        let methods = advertised_auth_method_ids(&initialized);
        let error = select_auth_method_id(&initialized, &methods, false).unwrap_err();
        assert!(
            matches!(error, AcpFailure::Refused(ref why) if why.contains("grok login")),
            "{error}"
        );
    }
}
