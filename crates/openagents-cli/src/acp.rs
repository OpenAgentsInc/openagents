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

    /// The mode id sent in `session/set_mode`.
    pub fn mode_id(&self) -> &'static str {
        match self {
            PermissionMode::Dangerous => "bypass",
            PermissionMode::Prompt => "default",
            PermissionMode::ReadOnly => "read-only",
        }
    }
}

/// What a running ACP child reports as it works.
#[derive(Debug, Clone)]
pub enum AcpEvent {
    Session { id: String },
    /// A tool the agent ran. `title` is Devin's own phrase — "Ran ls", "Read
    /// src/a.ts" — so it is the activity rather than a name to look up.
    Tool { kind: String, title: String },
    Tokens { input: u64, output: u64 },
    /// A piece of the answer, as it is written.
    Text { chunk: String },
}

/// How an ACP child is started.
#[derive(Debug, Clone)]
pub struct AcpHarness {
    /// The binary. Defaults to `devin`; a test points it at a stand-in.
    pub command: String,
    /// The subcommand that puts it in ACP mode.
    pub args: Vec<String>,
    pub mode: Option<PermissionMode>,
}

impl Default for AcpHarness {
    fn default() -> Self {
        Self {
            command: "devin".to_string(),
            args: vec!["acp".to_string()],
            mode: Some(PermissionMode::Dangerous),
        }
    }
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

impl AcpHarness {
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
        mut on_event: F,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<String, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Its own process group, so stopping the child stops what the
            // child started.
            .process_group(0)
            .spawn()
            .map_err(|error| {
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
    ) -> Result<String, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let mut seq: u64 = 0;
        let mut answer = String::new();

        request(
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
        )
        .await?;

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
        )
        .await?;

        let session_id = opened
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcpFailure::Refused("the agent opened no session".to_string()))?
            .to_string();
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
                serde_json::json!({"sessionId": session_id, "modeId": mode.mode_id()}),
                REQUEST_TIMEOUT,
                &mut answer,
                on_event,
                cancel,
            )
            .await;
        }

        request(
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
        )
        .await?;

        Ok(answer.trim().to_string())
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
                ))
            }
            Err(error) => {
                return Err(AcpFailure::Refused(format!(
                    "the agent's output could not be read: {error}"
                )))
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
        let is_reply = message.get("id").and_then(|v| v.as_u64()).is_some()
            && message.get("method").is_none();
        if is_reply {
            if message.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let text = serde_json::to_string(error).unwrap_or_default();
                return Err(AcpFailure::Refused(format!(
                    "the agent refused `{method}`: {}",
                    &text[..text.len().min(200)]
                )));
            }
            return Ok(message
                .get("result")
                .cloned()
                .unwrap_or(serde_json::json!({})));
        }

        handle_incoming(&message, stdin, answer, on_event).await?;
    }
}

/// A notification, or a request from the agent.
async fn handle_incoming<F>(
    message: &serde_json::Value,
    stdin: &mut ChildStdin,
    answer: &mut String,
    on_event: &mut F,
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
        let outcome = match first_allow_option(&params) {
            Some(option) => serde_json::json!({"outcome": "selected", "optionId": option}),
            None => serde_json::json!({"outcome": "cancelled"}),
        };
        write_line(
            stdin,
            &serde_json::json!({"jsonrpc": "2.0", "id": id, "result": {"outcome": outcome}}),
        )
        .await?;
        return Ok(());
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
            let meta = update.get("_meta").cloned().unwrap_or(serde_json::json!({}));
            let input = meta.get("cognition.ai/inputTokens").and_then(|v| v.as_u64());
            let output = meta.get("cognition.ai/outputTokens").and_then(|v| v.as_u64());
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

async fn write_line(
    stdin: &mut ChildStdin,
    value: &serde_json::Value,
) -> Result<(), AcpFailure> {
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
