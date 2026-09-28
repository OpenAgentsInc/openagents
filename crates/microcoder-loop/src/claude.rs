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
}

impl ClaudeGenerator {
    /// A generator for `model` on the `claude` binary.
    ///
    /// # Errors
    ///
    /// When no `claude` binary can be found.
    pub fn from_env(model: &str, effort: Option<String>) -> Result<Self, String> {
        Ok(ClaudeGenerator {
            model: alias(model),
            effort,
            binary: find_binary()?,
            bypass_permissions: false,
        })
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
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
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
    /// The binary's exit status, or `None` when it did not run or was killed.
    pub status: Option<i32>,
    /// The API status of an error result, such as 429 for a usage or rate
    /// limit; `None` for a success or a call with no report.
    pub api_error_status: Option<u16>,
    /// The last `rate_limit_event` the stream carried, if any.
    pub rate_limit: Option<RateLimit>,
    /// Whether the binary reported an error result.
    pub is_error: bool,
    pub stdout: String,
    pub stderr: String,
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
}

impl ClaudeGenerator {
    /// Runs the binary once for this step.
    pub async fn invoke(&self, system: &str, prompt: &str) -> Invocation {
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
        };
        let args = self.args(system);
        let mut child = match tokio::process::Command::new(&self.binary)
            .args(&args)
            .current_dir(std::env::temp_dir())
            .env(NO_CONNECTORS.0, NO_CONNECTORS.1)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                return failed(
                    format!("can't start {}: {error}", self.binary.display()),
                    Some(0.0),
                );
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let written = stdin.write_all(prompt.as_bytes()).await;
            drop(stdin);
            if let Err(error) = written {
                return failed(format!("can't write the prompt to claude: {error}"), None);
            }
        }
        let output = match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => return failed(format!("claude didn't finish: {error}"), None),
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
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let report = Report::parse(&stdout);
        dump_request(
            "claude",
            &json!({ "binary": self.binary, "args": args, "prompt": prompt }),
            json!({
                "status": output.status.code(),
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
                    status: output.status.code(),
                    stdout,
                    stderr,
                    ..failed(
                        format!("{why} (exit {:?}; stderr: {excerpt})", output.status.code()),
                        None,
                    )
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
            status: output.status.code(),
            api_error_status: report.is_error.then_some(report.api_error_status).flatten(),
            rate_limit: report.rate_limit,
            is_error: report.is_error,
            stdout,
            stderr,
        }
    }
}

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
        let generator = ClaudeGenerator {
            model: "haiku".into(),
            effort: None,
            binary,
            bypass_permissions: false,
        };
        let generated = generator.generate("SYS", "hello").await;
        let action = generated
            .action
            .expect("the stand-in answers a next action");
        assert_eq!(action.reply, NO_CONNECTORS.1);
    }

    #[test]
    fn args_turn_every_tool_off_and_carry_the_schema() {
        let generator = ClaudeGenerator {
            model: "opus".into(),
            effort: Some("xhigh".into()),
            binary: PathBuf::from("claude"),
            bypass_permissions: false,
        };
        let args = generator.args("SYS");
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at("--tools"), Some(String::new()));
        assert_eq!(at("--max-turns"), Some("1".into()));
        assert_eq!(at("--output-format"), Some("stream-json".into()));
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
        let generator = ClaudeGenerator {
            model: "claude-opus-5-5".into(),
            effort: None,
            binary: PathBuf::from("claude"),
            bypass_permissions: false,
        }
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
