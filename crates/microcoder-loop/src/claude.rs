//! Generation through the operator's Claude Code login (`--provider
//! claude`).
//!
//! Each step runs the `claude` binary once in print mode with every tool
//! off, one turn, no settings files, no claude.ai connectors
//! ([`NO_CONNECTORS`]), and no saved session, so the call is
//! one model request that answers under the `next_action` JSON schema
//! (`--json-schema`). The binary reads the operator's OAuth login itself;
//! Microcoder never reads the credential file.
//!
//! The binary prints its stream (`--output-format stream-json`), so a
//! step reads both the final `result` event and the `rate_limit_event`s
//! before it. When a usage limit refuses the call, the `rejected` event
//! carries when the limit resets (`resetsAt`), which the capacity book
//! records instead of a flat hold ([`crate::capacity::Refusal::claude`]).
//!
//! The call streams too: with `--include-partial-messages` the binary
//! prints each piece of the `StructuredOutput` tool's input as it is
//! written, and [`ClaudeGenerator::invoke_streaming`] hands those pieces
//! to its caller, so a host can show the step's reply
//! ([`crate::reply::Tap`]) before the call ends. The call returns as soon
//! as the `result` event is read, without waiting for the binary to exit.
//! The retained stdout leaves out those partial `stream_event` lines, which
//! repeat the complete `assistant` messages it keeps, and counts them.
//!
//! The binary reads its prompt as one `stream-json` user message, so it can
//! be started before the prompt exists ([`ClaudeGenerator::warm`]): it
//! loads while the host prepares the step and sends nothing to the model
//! until the prompt arrives. A warm process that is never used is killed
//! when the generator is dropped.
//!
//! Claude Code reports the request's list-price cost (`total_cost_usd`) and
//! its tokens, so a step's cost basis is [`Basis::ListPrice`]. A call that
//! fails before a request is sent cost nothing; one that fails after may
//! have cost what the report says, or an unknown amount when there is no
//! report.
//!
//! The binary is `CLAUDE_BIN`, else `claude` on `PATH`, else
//! `~/.local/bin/claude`, where Claude Code's installer puts it.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

use crate::models::{Basis, Generate, Generated, dump_request, next_action_schema, parse_action};

/// The variable that names the binary.
pub const BIN_VAR: &str = "CLAUDE_BIN";

/// The model alias a default Microcoder model name maps to: Microcoder's
/// defaults name Codex models, which Claude Code can't serve.
pub const DEFAULT_ALIAS: &str = "opus";

/// The service the binary sends requests to; a repository grant that names
/// this provider must name this endpoint, since Microcoder can't redirect
/// the binary.
pub const ENDPOINT: &str = "https://api.anthropic.com";

/// The longest one call may take before the binary is killed.
pub const TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The variable and value that keep a signed-in Claude Code from attaching
/// the account's claude.ai connectors to a call. Their tool lists go into
/// the prompt even with every tool off: on 2026-09-28 one Haiku call wrote
/// 128,267 prompt tokens to the cache with them and 4,505 without, and cost
/// $0.2567 against $0.0092. On Opus, the first step of an issue-flow smoke
/// cost $1.35 with them.
pub const NO_CONNECTORS: (&str, &str) = ("ENABLE_CLAUDEAI_MCP_SERVERS", "false");

/// The Claude Code model for `model`: [`DEFAULT_ALIAS`] when `model` is
/// one of Microcoder's Codex defaults, else `model` as given (an alias such
/// as `opus` or `sonnet`, or a full name).
#[must_use]
pub fn alias(model: &str) -> String {
    if model == crate::MODEL || model == crate::STRONG_MODEL {
        DEFAULT_ALIAS.to_string()
    } else {
        model.to_string()
    }
}

/// Generation through the `claude` binary.
pub struct ClaudeGenerator {
    /// The model alias or name passed as `--model`.
    pub model: String,
    /// `low`, `medium`, `high`, `xhigh`, or `max`, or `None` for the
    /// model's default.
    pub effort: Option<String>,
    pub binary: PathBuf,
    /// Pass `--permission-mode bypassPermissions`, for a run with the
    /// owner's full access. Each call has every tool off, so Claude Code
    /// runs nothing itself either way; the flag keeps a permission prompt
    /// from ever holding a headless call.
    pub bypass_permissions: bool,
    /// A process started ahead of its prompt ([`ClaudeGenerator::warm`]).
    warm: std::sync::Mutex<Option<Box<Warm>>>,
}

/// A started binary waiting for its prompt, and the system text it was
/// started with.
struct Warm {
    system: String,
    child: tokio::process::Child,
}

impl ClaudeGenerator {
    /// A generator for `model` on the `claude` binary.
    ///
    /// # Errors
    ///
    /// When no `claude` binary can be found.
    pub fn from_env(model: &str, effort: Option<String>) -> Result<Self, String> {
        Ok(Self::new(alias(model), effort, find_binary()?))
    }

    /// A generator for `model` (passed as given) on `binary`.
    #[must_use]
    pub fn new(model: String, effort: Option<String>, binary: PathBuf) -> Self {
        ClaudeGenerator {
            model,
            effort,
            binary,
            bypass_permissions: false,
            warm: std::sync::Mutex::new(None),
        }
    }

    /// This generator, passing `--permission-mode bypassPermissions` when
    /// `bypass` is set.
    #[must_use]
    pub fn bypassing_permissions(mut self, bypass: bool) -> Self {
        self.bypass_permissions = bypass;
        self
    }

    /// The arguments one step passes, before the prompt on stdin.
    #[must_use]
    pub fn args(&self, system: &str) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--include-partial-messages".to_string(),
            "--no-session-persistence".to_string(),
            "--tools".to_string(),
            String::new(),
            "--max-turns".to_string(),
            "1".to_string(),
            "--setting-sources".to_string(),
            String::new(),
            "--model".to_string(),
            self.model.clone(),
        ];
        if let Some(effort) = &self.effort {
            args.push("--effort".to_string());
            args.push(effort.clone());
        }
        if self.bypass_permissions {
            args.push("--permission-mode".to_string());
            args.push("bypassPermissions".to_string());
        }
        args.push("--system-prompt".to_string());
        args.push(system.to_string());
        args.push("--json-schema".to_string());
        args.push(next_action_schema().to_string());
        args
    }
}

fn find_binary() -> Result<PathBuf, String> {
    if let Some(named) = std::env::var_os(BIN_VAR).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(named);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{BIN_VAR} names {}, which isn't a file",
                path.display()
            ))
        };
    }
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("claude"))
            .find(|path| path.is_file())
    });
    if let Some(path) = on_path {
        return Ok(path);
    }
    let local = std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/bin/claude"));
    match local {
        Some(path) if path.is_file() => Ok(path),
        _ => Err(
            "no claude binary: set CLAUDE_BIN or put claude on PATH; run `claude login` there"
                .into(),
        ),
    }
}

/// The fields of Claude Code's `result` event this reads.
#[derive(Debug, Default, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub is_error: bool,
    /// The HTTP status of the API error that ended the turn, when one did,
    /// such as 429 for a usage or rate limit.
    #[serde(default)]
    pub api_error_status: Option<u16>,
    #[serde(default)]
    pub result: String,
    /// The result's kind, such as `success` or `error_max_turns`.
    #[serde(default)]
    pub subtype: Option<String>,
    /// Error messages an error result carries.
    #[serde(default)]
    pub errors: Vec<Value>,
    #[serde(default)]
    pub structured_output: Option<Value>,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub usage: Usage,
    /// Keyed by the canonical model name, one entry per model that served.
    #[serde(default, rename = "modelUsage")]
    pub model_usage: serde_json::Map<String, Value>,
    /// The last `rate_limit_event` before the result, when the stream
    /// carried one.
    #[serde(skip)]
    pub rate_limit: Option<RateLimit>,
}

/// The token counts of one result.
#[derive(Debug, Default, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
}

/// Where a `rate_limit_event` says the login stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitStatus {
    Allowed,
    AllowedWarning,
    /// The limit refused the request.
    Rejected,
    /// A status this reader doesn't know.
    #[serde(other)]
    Other,
}

/// The window a `rate_limit_event` names as the limiting one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitWindow {
    FiveHour,
    SevenDay,
    SevenDayOpus,
    SevenDaySonnet,
    SevenDayOverageIncluded,
    Overage,
    /// A window this reader doesn't know.
    #[serde(other)]
    Other,
}

impl LimitWindow {
    /// The window's length in minutes, when it is a fixed window.
    #[must_use]
    pub const fn minutes(self) -> Option<u64> {
        match self {
            LimitWindow::FiveHour => Some(5 * 60),
            LimitWindow::SevenDay
            | LimitWindow::SevenDayOpus
            | LimitWindow::SevenDaySonnet
            | LimitWindow::SevenDayOverageIncluded => Some(7 * 24 * 60),
            LimitWindow::Overage | LimitWindow::Other => None,
        }
    }
}

/// A `rate_limit_event`'s `rate_limit_info`: the typed fields only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct RateLimit {
    pub status: LimitStatus,
    /// When the limiting window resets, in Unix seconds.
    #[serde(default, rename = "resetsAt")]
    pub resets_at: Option<u64>,
    #[serde(default, rename = "rateLimitType")]
    pub window: Option<LimitWindow>,
}

impl RateLimit {
    /// Whether the event says the limit refused the request.
    #[must_use]
    pub fn rejected(&self) -> bool {
        self.status == LimitStatus::Rejected
    }
}

/// One line of the binary's stream, by its `type`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Line {
    Result(Box<Report>),
    RateLimitEvent {
        rate_limit_info: RateLimit,
    },
    #[serde(other)]
    Other,
}

impl Report {
    /// The report in the binary's stdout: the stream's `result` event,
    /// with the last `rate_limit_event` before it. Output with no typed
    /// `result` event is read as a single result object, the binary's
    /// `--output-format json`, taking the last JSON object, since a warning
    /// may precede it.
    ///
    /// # Errors
    ///
    /// When there is no result.
    pub fn parse(stdout: &str) -> Result<Report, String> {
        let mut rate_limit = None;
        let mut result = None;
        for line in stdout.lines().map(str::trim).filter(|l| l.starts_with('{')) {
            match serde_json::from_str::<Line>(line) {
                Ok(Line::Result(report)) => result = Some(report),
                Ok(Line::RateLimitEvent { rate_limit_info }) => rate_limit = Some(rate_limit_info),
                Ok(Line::Other) | Err(_) => {}
            }
        }
        if let Some(mut report) = result {
            report.rate_limit = rate_limit;
            return Ok(*report);
        }
        let start = stdout
            .rfind("\n{")
            .map(|i| i + 1)
            .or_else(|| stdout.trim_start().starts_with('{').then_some(0))
            .ok_or_else(|| {
                let excerpt: String = stdout.trim().chars().take(300).collect();
                format!("claude printed no JSON result: {excerpt}")
            })?;
        let mut report: Report = serde_json::from_str(stdout[start..].trim())
            .map_err(|e| format!("claude's result isn't the expected JSON: {e}"))?;
        report.rate_limit = rate_limit;
        Ok(report)
    }

    /// What an error result says went wrong: its text, else its error
    /// messages, else its kind.
    #[must_use]
    pub fn error(&self) -> String {
        let text = self.result.trim();
        if !text.is_empty() {
            return text.to_string();
        }
        let errors: Vec<&str> = self.errors.iter().filter_map(Value::as_str).collect();
        if !errors.is_empty() {
            return errors.join("; ");
        }
        self.subtype
            .clone()
            .unwrap_or_else(|| "no detail".to_string())
    }

    /// Every input token, cached or not.
    #[must_use]
    pub fn input_tokens(&self) -> u64 {
        self.usage.input_tokens
            + self.usage.cache_creation_input_tokens
            + self.usage.cache_read_input_tokens
    }

    /// The canonical name of the model that served, when one did.
    #[must_use]
    pub fn served_model(&self) -> Option<String> {
        self.model_usage.keys().next().cloned()
    }
}

/// One call of the binary: the step it produced and what the process
/// printed, for a host that retains native evidence before reducing it.
#[derive(Debug)]
pub struct Invocation {
    pub generated: Generated,
    /// The binary's exit status, or `None` when it did not run, was
    /// killed, or had not exited yet when its result was read.
    pub status: Option<i32>,
    /// The API status of an error result, such as 429 for a usage or rate
    /// limit; `None` for a success or a call with no report.
    pub api_error_status: Option<u16>,
    /// The last `rate_limit_event` the stream carried, if any.
    pub rate_limit: Option<RateLimit>,
    /// Whether the binary reported an error result.
    pub is_error: bool,
    /// What the binary printed, less its partial `stream_event` lines.
    pub stdout: String,
    pub stderr: String,
    /// How many partial `stream_event` lines the binary printed.
    pub stream_events: usize,
}

impl Invocation {
    /// The capacity refusal this call met, if a usage or rate limit
    /// refused it, with the reset the stream reported.
    #[must_use]
    pub fn refusal(&self, now: u64) -> Option<crate::capacity::Refusal> {
        crate::capacity::Refusal::claude(
            self.is_error,
            self.api_error_status,
            self.rate_limit.as_ref(),
            now,
        )
    }
}

impl Generate for ClaudeGenerator {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        self.invoke(system, prompt).await.generated
    }

    fn warm(&self, system: &str) {
        ClaudeGenerator::warm(self, system);
    }
}

impl ClaudeGenerator {
    /// Starts the binary for a step under `system`, to wait for its prompt,
    /// unless one is already waiting with that system text. It sends
    /// nothing to the model. A start that fails is left to the step, which
    /// starts the binary again and reports why.
    pub fn warm(&self, system: &str) {
        let Ok(mut warm) = self.warm.lock() else {
            return;
        };
        if let Some(ready) = warm.as_mut()
            && ready.system == system
            && matches!(ready.child.try_wait(), Ok(None))
        {
            return;
        }
        *warm = self.spawn(system).ok().map(|child| {
            Box::new(Warm {
                system: system.to_owned(),
                child,
            })
        });
    }

    /// The waiting process for `system`, if one is still running.
    fn take_warm(&self, system: &str) -> Option<tokio::process::Child> {
        let mut ready = self.warm.lock().ok()?.take()?;
        (ready.system == system && matches!(ready.child.try_wait(), Ok(None)))
            .then_some(ready.child)
    }

    fn spawn(&self, system: &str) -> std::io::Result<tokio::process::Child> {
        tokio::process::Command::new(&self.binary)
            .args(self.args(system))
            .current_dir(std::env::temp_dir())
            .env(NO_CONNECTORS.0, NO_CONNECTORS.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
    }

    /// Runs the binary once for this step.
    pub async fn invoke(&self, system: &str, prompt: &str) -> Invocation {
        self.invoke_streaming(system, prompt, &mut |_| {}).await
    }

    /// Runs the binary once for this step, handing `partial` each piece of
    /// the next action's JSON text as the model writes it. The pieces, in
    /// order, are the text of the action the call returns, when it returns
    /// one; a call that fails may have handed over part of an action.
    pub async fn invoke_streaming(
        &self,
        system: &str,
        prompt: &str,
        partial: &mut dyn FnMut(&str),
    ) -> Invocation {
        let started = Instant::now();
        let milliseconds = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let failed = |why: String, usd: Option<f64>| Invocation {
            generated: Generated {
                action: Err(why.clone()),
                model: self.model.clone(),
                prompt_tokens: 0,
                completion_tokens: 0,
                usd,
                known_usd: usd.unwrap_or(0.0),
                cost_unknown: usd.is_none().then_some(why),
                usd_upper: usd,
                cost_basis: Basis::ListPrice,
                milliseconds: milliseconds(),
            },
            status: None,
            api_error_status: None,
            rate_limit: None,
            is_error: false,
            stdout: String::new(),
            stderr: String::new(),
            stream_events: 0,
        };
        let args = self.args(system);
        let mut child = match self.take_warm(system) {
            Some(child) => child,
            None => match self.spawn(system) {
                Ok(child) => child,
                Err(error) => {
                    return failed(
                        format!("can't start {}: {error}", self.binary.display()),
                        Some(0.0),
                    );
                }
            },
        };
        if let Some(mut stdin) = child.stdin.take() {
            let mut line =
                json!({"type":"user","message":{"role":"user","content":prompt}}).to_string();
            line.push('\n');
            let written = stdin.write_all(line.as_bytes()).await;
            drop(stdin);
            if let Err(error) = written {
                return failed(format!("can't write the prompt to claude: {error}"), None);
            }
        }
        // Stderr is read beside stdout, so a full pipe never holds the
        // binary; what it printed by the result is what the call keeps.
        let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let errors = errors.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut chunk = [0u8; 8192];
                while let Ok(read) = stderr.read(&mut chunk).await {
                    if read == 0 {
                        break;
                    }
                    if let Ok(mut errors) = errors.lock() {
                        errors.extend_from_slice(&chunk[..read]);
                    }
                }
            });
        }
        let Some(stdout) = child.stdout.take() else {
            return failed("claude's output was not captured".into(), None);
        };
        let read = tokio::time::timeout(TIMEOUT, read_stream(stdout, partial)).await;
        let stream = match read {
            Ok(stream) => stream,
            Err(_) => {
                return failed(
                    format!(
                        "claude ran past {} seconds and was killed",
                        TIMEOUT.as_secs()
                    ),
                    None,
                );
            }
        };
        // A stream that ended without a result is the binary exiting: its
        // status says how. After a result the call returns at once, and the
        // binary is left to exit on its own.
        let status = if stream.result {
            let status = child.try_wait().ok().flatten().and_then(|s| s.code());
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
            status
        } else {
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => status.code(),
                _ => None,
            }
        };
        let stdout = stream.kept;
        let stderr = errors
            .lock()
            .map(|errors| String::from_utf8_lossy(&errors).into_owned())
            .unwrap_or_default();
        let report = Report::parse(&stdout);
        dump_request(
            "claude",
            &json!({ "binary": self.binary, "args": args, "prompt": prompt }),
            json!({
                "status": status,
                "stdout": stdout,
                "stderr": stderr,
                "error": report.as_ref().err(),
            }),
        );
        let report = match report {
            Ok(report) => report,
            Err(why) => {
                let excerpt: String = stderr.trim().chars().take(300).collect();
                return Invocation {
                    status,
                    stdout,
                    stderr,
                    stream_events: stream.events,
                    ..failed(format!("{why} (exit {status:?}; stderr: {excerpt})"), None)
                };
            }
        };
        let usd = report.total_cost_usd;
        let model = report.served_model().unwrap_or_else(|| self.model.clone());
        let action = if report.is_error {
            Err(format!("claude reported an error: {}", report.error()))
        } else {
            match &report.structured_output {
                Some(value) => serde_json::from_value(value.clone())
                    .map_err(|e| format!("the structured output isn't a next action: {e}")),
                None => parse_action(&report.result),
            }
        };
        Invocation {
            generated: Generated {
                action,
                model,
                prompt_tokens: report.input_tokens(),
                completion_tokens: report.usage.output_tokens,
                usd,
                known_usd: usd.unwrap_or(0.0),
                cost_unknown: usd
                    .is_none()
                    .then(|| "claude reported no cost for the call".to_string()),
                usd_upper: usd,
                cost_basis: Basis::ListPrice,
                milliseconds: milliseconds(),
            },
            status,
            api_error_status: report.is_error.then_some(report.api_error_status).flatten(),
            rate_limit: report.rate_limit,
            is_error: report.is_error,
            stdout,
            stderr,
            stream_events: stream.events,
        }
    }
}

/// What one call printed on stdout, read until its `result` event.
struct Stream {
    /// Every line but the partial `stream_event`s.
    kept: String,
    /// How many `stream_event` lines were read and left out of `kept`.
    events: usize,
    /// Whether the `result` event was read.
    result: bool,
}

/// Reads the binary's stream until its `result` event or its end, handing
/// `partial` the `StructuredOutput` tool's input as it streams.
async fn read_stream(stdout: tokio::process::ChildStdout, partial: &mut dyn FnMut(&str)) -> Stream {
    use tokio::io::AsyncBufReadExt;
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    let mut stream = Stream {
        kept: String::new(),
        events: 0,
        result: false,
    };
    // The content block that holds the structured output.
    let mut block: Option<u64> = None;
    while let Ok(Some(line)) = lines.next_line().await {
        let value = line
            .trim_start()
            .starts_with('{')
            .then(|| serde_json::from_str::<Value>(&line).ok())
            .flatten();
        let kind = value.as_ref().and_then(|v| v["type"].as_str());
        if kind == Some("stream_event") {
            stream.events += 1;
            let event = &value.as_ref().map_or(&Value::Null, |v| &v["event"]);
            match event["type"].as_str() {
                Some("content_block_start") => {
                    let content = &event["content_block"];
                    if content["type"] == "tool_use" && content["name"] == STRUCTURED_OUTPUT {
                        block = event["index"].as_u64();
                    }
                }
                Some("content_block_delta")
                    if block.is_some() && event["index"].as_u64() == block =>
                {
                    if let Some(text) = event["delta"]["partial_json"]
                        .as_str()
                        .filter(|_| event["delta"]["type"] == "input_json_delta")
                    {
                        partial(text);
                    }
                }
                _ => {}
            }
            continue;
        }
        stream.kept.push_str(&line);
        stream.kept.push('\n');
        if kind == Some("result") {
            stream.result = true;
            break;
        }
    }
    stream
}

/// The tool Claude Code answers a `--json-schema` call through.
const STRUCTURED_OUTPUT: &str = "StructuredOutput";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_map_to_the_alias_and_names_pass_through() {
        assert_eq!(alias(crate::MODEL), DEFAULT_ALIAS);
        assert_eq!(alias(crate::STRONG_MODEL), DEFAULT_ALIAS);
        assert_eq!(alias("sonnet"), "sonnet");
        assert_eq!(alias("claude-opus-5-5"), "claude-opus-5-5");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_call_keeps_the_accounts_connectors_out() {
        // A stand-in binary that answers with the variable it was given.
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("claude");
        std::fs::write(
            &binary,
            "#!/bin/sh\ncat > /dev/null\necho \"{\\\"type\\\":\\\"result\\\",\\\"is_error\\\":false,\\\"result\\\":\\\"\\\",\\\"structured_output\\\":{\\\"rationale\\\":\\\"r\\\",\\\"commands\\\":[],\\\"view\\\":[],\\\"freeze_tests\\\":false,\\\"expand\\\":[],\\\"finished\\\":true,\\\"reply\\\":\\\"$ENABLE_CLAUDEAI_MCP_SERVERS\\\"},\\\"total_cost_usd\\\":0.001}\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        let generator = ClaudeGenerator::new("haiku".into(), None, binary);
        let generated = generator.generate("SYS", "hello").await;
        let action = generated
            .action
            .expect("the stand-in answers a next action");
        assert_eq!(action.reply, NO_CONNECTORS.1);
    }

    /// A stand-in `claude` binary running `script`.
    #[cfg(unix)]
    fn stand_in(dir: &Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let binary = dir.join("claude");
        std::fs::write(&binary, format!("#!/bin/sh\n{script}")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        binary
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn the_structured_output_streams_and_the_call_returns_at_its_result() {
        let dir = tempfile::tempdir().unwrap();
        let binary = stand_in(
            dir.path(),
            r#"read line
printf '%s\n' '{"type":"system","subtype":"init"}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"prose"}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","name":"StructuredOutput","input":{}}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"reply\": \"hi"}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\", \"finished\": true}"}}}'
printf '%s\n' '{"type":"result","is_error":false,"result":"","structured_output":{"reply":"hi","ask":"none","rationale":"r","commands":[],"view":[],"freeze_tests":false,"expand":[],"finished":true},"total_cost_usd":0.001}'
sleep 20
"#,
        );
        let generator = ClaudeGenerator::new("haiku".into(), None, binary);
        let started = Instant::now();
        let mut pieces = Vec::new();
        let invocation = generator
            .invoke_streaming("SYS", "hello", &mut |text| pieces.push(text.to_owned()))
            .await;
        // The call ends at the result, not when the binary exits.
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(invocation.generated.action.unwrap().reply, "hi");
        // Only the structured output's pieces, in order.
        assert_eq!(pieces.concat(), r#"{"reply": "hi", "finished": true}"#);
        let mut tap = crate::reply::Tap::new();
        for piece in &pieces {
            tap.feed(piece);
        }
        assert_eq!((tap.reply(), tap.complete()), ("hi", true));
        // The partial lines are counted, not kept.
        assert_eq!(invocation.stream_events, 5);
        assert!(!invocation.stdout.contains("stream_event"));
        assert!(invocation.stdout.contains(r#""type":"result""#));
        assert_eq!(invocation.status, None);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_warm_binary_waits_for_its_prompt_and_answers_the_step() {
        let dir = tempfile::tempdir().unwrap();
        let started = dir.path().join("started");
        let prompt = dir.path().join("prompt");
        let binary = stand_in(
            dir.path(),
            &format!(
                r#"echo $$ > '{}'
read -r line
printf '%s' "$line" > '{}'
printf '{{"type":"result","is_error":false,"result":"","structured_output":{{"reply":"%s","ask":"none","rationale":"r","commands":[],"view":[],"freeze_tests":false,"expand":[],"finished":true}}}}\n' $$
"#,
                started.display(),
                prompt.display()
            ),
        );
        let generator = ClaudeGenerator::new("haiku".into(), None, binary);
        generator.warm("SYS");
        // The warm binary starts before any prompt exists.
        let deadline = Instant::now() + Duration::from_secs(10);
        while !started.exists() {
            assert!(Instant::now() < deadline, "the warm binary never started");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(!prompt.exists());
        // A second warm with the same system text keeps it.
        generator.warm("SYS");
        let pid = std::fs::read_to_string(&started).unwrap().trim().to_owned();
        let reply = generator
            .invoke("SYS", "the \"prompt\"")
            .await
            .generated
            .action
            .unwrap()
            .reply;
        assert_eq!(reply, pid, "the step ran on the warm binary");
        // The prompt arrived as one stream-json user message.
        let line: Value = serde_json::from_str(&std::fs::read_to_string(&prompt).unwrap()).unwrap();
        assert_eq!(
            line,
            json!({"type":"user","message":{"role":"user","content":"the \"prompt\""}})
        );
        // A step under other system text starts its own binary.
        generator.warm("OTHER");
        let other = generator
            .invoke("SYS", "x")
            .await
            .generated
            .action
            .unwrap()
            .reply;
        assert_ne!(other, pid);
    }

    #[test]
    fn args_turn_every_tool_off_and_carry_the_schema() {
        let generator =
            ClaudeGenerator::new("opus".into(), Some("xhigh".into()), PathBuf::from("claude"));
        let args = generator.args("SYS");
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at("--tools"), Some(String::new()));
        assert_eq!(at("--max-turns"), Some("1".into()));
        assert_eq!(at("--output-format"), Some("stream-json".into()));
        assert_eq!(at("--input-format"), Some("stream-json".into()));
        assert!(args.contains(&"--include-partial-messages".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert_eq!(at("--setting-sources"), Some(String::new()));
        assert_eq!(at("--model"), Some("opus".into()));
        assert_eq!(at("--effort"), Some("xhigh".into()));
        assert_eq!(at("--system-prompt"), Some("SYS".into()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        let schema: Value = serde_json::from_str(&at("--json-schema").unwrap()).unwrap();
        assert_eq!(schema, next_action_schema());
        assert_eq!(at("--permission-mode"), None);
    }

    #[test]
    fn a_full_access_call_bypasses_permissions_and_still_runs_no_tool() {
        let generator =
            ClaudeGenerator::new("claude-opus-5-5".into(), None, PathBuf::from("claude"))
                .bypassing_permissions(true);
        let args = generator.args("SYS");
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at("--permission-mode"), Some("bypassPermissions".into()));
        assert_eq!(at("--tools"), Some(String::new()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn a_report_yields_the_action_tokens_cost_and_served_model() {
        let stdout = concat!(
            "some warning\n",
            r#"{"is_error":false,"result":"{}","structured_output":{"rationale":"r","commands":["ls"],"view":[],"freeze_tests":false,"expand":[],"finished":false},"#,
            r#""total_cost_usd":0.032,"usage":{"input_tokens":2,"cache_creation_input_tokens":3926,"cache_read_input_tokens":10,"output_tokens":51},"#,
            r#""modelUsage":{"claude-opus-5-5":{"costUSD":0.032}}}"#,
            "\n"
        );
        let report = Report::parse(stdout).unwrap();
        assert!(!report.is_error);
        assert_eq!(report.input_tokens(), 3938);
        assert_eq!(report.usage.output_tokens, 51);
        assert_eq!(report.total_cost_usd, Some(0.032));
        assert_eq!(report.served_model().as_deref(), Some("claude-opus-5-5"));
        let action: crate::models::NextAction =
            serde_json::from_value(report.structured_output.unwrap()).unwrap();
        assert_eq!(action.commands, vec!["ls".to_string()]);
    }

    #[test]
    fn a_login_failure_is_an_error_report() {
        let report = Report::parse(
            r#"{"is_error":true,"result":"Not logged in · Please run /login","total_cost_usd":0}"#,
        )
        .unwrap();
        assert!(report.is_error);
        assert_eq!(report.api_error_status, None);
        let limited = Report::parse(
            r#"{"type":"result","subtype":"success","is_error":true,"api_error_status":429,"result":"limit reached","total_cost_usd":0}"#,
        )
        .unwrap();
        assert_eq!(limited.api_error_status, Some(429));
        assert_eq!(limited.rate_limit, None);
        // An error result with no text names its kind or its errors.
        let turns = Report::parse(
            r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":""}"#,
        )
        .unwrap();
        assert_eq!(turns.error(), "error_max_turns");
        let errors = Report::parse(
            r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["schema mismatch"]}"#,
        )
        .unwrap();
        assert_eq!(errors.error(), "schema mismatch");
        assert_eq!(report.result, "Not logged in · Please run /login");
        assert!(Report::parse("nothing here").is_err());
    }

    #[test]
    fn a_recorded_session_limit_stream_carries_its_reset() {
        let stdout = include_str!("../fixtures/claude/session-limit.stream.jsonl");
        let report = Report::parse(stdout).unwrap();
        assert!(report.is_error);
        assert_eq!(report.api_error_status, Some(429));
        let limit = report.rate_limit.unwrap();
        assert!(limit.rejected());
        assert_eq!(limit.resets_at, Some(1_790_164_200));
        assert_eq!(limit.window, Some(LimitWindow::FiveHour));
    }

    #[test]
    fn a_stream_takes_the_result_event_and_the_last_limit_event_before_it() {
        let stdout = concat!(
            r#"{"type":"system","subtype":"init","tools":[]}"#,
            "\n",
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1790715600,"rateLimitType":"seven_day","utilization":0.9,"unifiedWindows":{}}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[]}}"#,
            "\n",
            r#"{"type":"result","subtype":"success","is_error":false,"structured_output":{"rationale":"r","commands":[],"view":[],"freeze_tests":false,"expand":[],"finished":true,"reply":"hello"},"total_cost_usd":0.0199,"modelUsage":{"claude-haiku-4-5":{}}}"#,
            "\n"
        );
        let report = Report::parse(stdout).unwrap();
        assert!(!report.is_error);
        assert_eq!(report.total_cost_usd, Some(0.0199));
        assert_eq!(report.served_model().as_deref(), Some("claude-haiku-4-5"));
        let limit = report.rate_limit.unwrap();
        assert_eq!(limit.status, LimitStatus::AllowedWarning);
        assert!(!limit.rejected());
        // A status or window this reader doesn't know still parses.
        let odd = Report::parse(concat!(
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"paused","rateLimitType":"hourly"}}"#,
            "\n",
            r#"{"type":"result","is_error":true,"api_error_status":429,"result":"x"}"#
        ))
        .unwrap();
        let limit = odd.rate_limit.unwrap();
        assert_eq!(
            (limit.status, limit.window),
            (LimitStatus::Other, Some(LimitWindow::Other))
        );
    }
}
