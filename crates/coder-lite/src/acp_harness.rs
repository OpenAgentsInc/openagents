//! ACP child agent harness for coder-lite.
//!
//! Spawns an ACP-compatible CLI agent over stdio and streams JSON-RPC
//! `session/update` events as they arrive: `initialize`, `session/new`, an
//! optional `session/set_mode`, then `session/prompt`. A
//! `session/request_permission` the agent sends back is answered without
//! asking the reader, preferring whichever option the agent marked `allow*`.
//!
//! ## The child is stopped with its whole tree
//!
//! A coding agent shells out. Killing only the agent leaves its build, its
//! test run, or its `sleep` behind with nothing left to stop them, so the
//! child is spawned into a process group of its own and
//! [`openagents_cli::signals::stop_tree`] signals the group — `SIGTERM`, then
//! `SIGKILL` after a grace period, so an agent that writes a transcript on the
//! way out gets to write it. This used to be a bare `child.kill()`, which
//! stopped the agent and orphaned everything under it.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use openagents_cli::signals::stop_tree;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};

/// How much the child is allowed to do without being asked.
///
/// The names are coder-lite's; the wire carries the agent's own. A build of
/// the agent that does not know a mode is not a reason to lose the child, so
/// setting it is best effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn mode_id(self) -> &'static str {
        match self {
            PermissionMode::Dangerous => "bypass",
            PermissionMode::Prompt => "default",
            PermissionMode::ReadOnly => "read-only",
        }
    }
}

#[derive(Debug)]
pub enum AcpFailure {
    Unstartable(String),
    Refused(String),
}

impl std::fmt::Display for AcpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpFailure::Unstartable(why) => write!(f, "{why}"),
            AcpFailure::Refused(why) => write!(f, "{why}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AcpEvent {
    Session { id: String },
    Tool { kind: String, title: String },
    Tokens { input: u64, output: u64 },
    Text { chunk: String },
}

#[derive(Debug, Clone)]
pub struct AcpHarness {
    pub command: String,
    pub args: Vec<String>,
    /// Sent as `session/set_mode` after the session opens. `None` leaves the
    /// agent's own default, which is a different answer from naming one.
    pub mode: Option<PermissionMode>,
}

impl Default for AcpHarness {
    fn default() -> Self {
        Self {
            command: "devin".to_string(),
            args: vec!["acp".to_string()],
            mode: None,
        }
    }
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(900);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);

impl AcpHarness {
    pub async fn run<F>(
        &self,
        prompt: &str,
        cwd: &Path,
        mut on_event: F,
    ) -> Result<String, AcpFailure>
    where
        F: FnMut(AcpEvent) + Send,
    {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own process group, so stopping the child stops what the child
        // started.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| {
                AcpFailure::Unstartable(if error.kind() == std::io::ErrorKind::NotFound {
                    format!("the `{}` command is not on PATH", self.command)
                } else {
                    format!("the `{}` command would not start: {error}", self.command)
                })
            })?;

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
            .converse(prompt, cwd, &mut stdin, &mut lines, &mut on_event)
            .await;

        stop_tree(&mut child).await;
        outcome
    }

    async fn converse<F>(
        &self,
        prompt: &str,
        cwd: &Path,
        stdin: &mut ChildStdin,
        lines: &mut tokio::io::Lines<BufReader<ChildStdout>>,
        on_event: &mut F,
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

        if let Some(mode) = self.mode {
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
        )
        .await?;

        Ok(answer.trim().to_string())
    }
}

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
        let Ok(message) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };

        let is_reply = message.get("id").and_then(|v| v.as_u64()).is_some()
            && message.get("method").is_none();
        if is_reply {
            if message.get("id").and_then(|v| v.as_u64()) != Some(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                // The agent's own bytes, and `serde_json` does not escape
                // non-ASCII, so a refusal carrying an accent or an emoji
                // across byte 200 used to panic here and take the whole
                // session with it. Floored to a character boundary, as the
                // four cuts in `28704f72ff` were.
                let text = serde_json::to_string(error).unwrap_or_default();
                let end = openagents_cli::tracker::floor_char_boundary(&text, 200);
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

        handle_incoming(&message, stdin, answer, on_event).await?;
    }
}

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
            let kind = update
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("tool")
                .to_string();
            let title = update
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            answer.push_str(&format!("[{}] {}\n", kind, title));
            on_event(AcpEvent::Tool { kind, title });
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

fn first_allow_option(params: &serde_json::Value) -> Option<String> {
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
    /// A refusal carrying a multi-byte character across the 200-byte bound
    /// used to panic here, which killed the whole session rather than the one
    /// delegation. Same defect class as `28704f72ff`.
    #[test]
    fn a_long_refusal_with_a_multibyte_character_on_the_bound_does_not_panic() {
        // Sized so the multi-byte character straddles byte 200 of the
        // *encoded* JSON, not of the message: the envelope counts too, and a
        // fixture that guessed the offset proved nothing. Searched rather than
        // computed, so a change in how `serde_json` encodes cannot silently
        // turn this into a test of a string that was ASCII all along. The
        // length check matters as much as the boundary one — `is_char_boundary`
        // is also false for an index past the end.
        let text = (150..250)
            .map(|width| {
                let message = format!("{}\u{20ac}", "x".repeat(width));
                serde_json::to_string(&serde_json::json!({ "message": message })).unwrap()
            })
            .find(|text| text.len() > 200 && !text.is_char_boundary(200))
            .expect("no width made the character straddle byte 200");

        let end = openagents_cli::tracker::floor_char_boundary(&text, 200);
        let head = &text[..end]; // the old code was `&text[..200]`, which panics
        assert!(end < 200);
        assert!(head.len() == end);
    }
}
