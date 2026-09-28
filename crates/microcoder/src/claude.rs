//! Generation through the operator's Claude Code login (`--provider
//! claude`).
//!
//! Each step runs the `claude` binary once in print mode with every tool
//! off, one turn, no settings files, and no saved session, so the call is
//! one model request that answers under the `next_action` JSON schema
//! (`--json-schema`). The binary reads the operator's OAuth login itself;
//! Microcoder never reads the credential file.
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
        })
    }

    /// The arguments one step passes, before the prompt on stdin.
    #[must_use]
    pub fn args(&self, system: &str) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
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

/// The fields of Claude Code's `--output-format json` result this reads.
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
    #[serde(default)]
    pub structured_output: Option<Value>,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub usage: Usage,
    /// Keyed by the canonical model name, one entry per model that served.
    #[serde(default, rename = "modelUsage")]
    pub model_usage: serde_json::Map<String, Value>,
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

impl Report {
    /// The report in the binary's stdout: its last JSON object, since a
    /// warning may precede it.
    ///
    /// # Errors
    ///
    /// When there is no JSON object.
    pub fn parse(stdout: &str) -> Result<Report, String> {
        let start = stdout
            .rfind("\n{")
            .map(|i| i + 1)
            .or_else(|| stdout.trim_start().starts_with('{').then_some(0))
            .ok_or_else(|| {
                let excerpt: String = stdout.trim().chars().take(300).collect();
                format!("claude printed no JSON result: {excerpt}")
            })?;
        serde_json::from_str(stdout[start..].trim())
            .map_err(|e| format!("claude's result isn't the expected JSON: {e}"))
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
    pub stdout: String,
    pub stderr: String,
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
            stdout: String::new(),
            stderr: String::new(),
        };
        let args = self.args(system);
        let mut child = match tokio::process::Command::new(&self.binary)
            .args(&args)
            .current_dir(std::env::temp_dir())
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
            Err(format!(
                "claude reported an error: {}",
                report.result.trim()
            ))
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

    #[test]
    fn args_turn_every_tool_off_and_carry_the_schema() {
        let generator = ClaudeGenerator {
            model: "opus".into(),
            effort: Some("xhigh".into()),
            binary: PathBuf::from("claude"),
        };
        let args = generator.args("SYS");
        let at = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(at("--tools"), Some(String::new()));
        assert_eq!(at("--max-turns"), Some("1".into()));
        assert_eq!(at("--setting-sources"), Some(String::new()));
        assert_eq!(at("--model"), Some("opus".into()));
        assert_eq!(at("--effort"), Some("xhigh".into()));
        assert_eq!(at("--system-prompt"), Some("SYS".into()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        let schema: Value = serde_json::from_str(&at("--json-schema").unwrap()).unwrap();
        assert_eq!(schema, next_action_schema());
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
        assert_eq!(report.result, "Not logged in · Please run /login");
        assert!(Report::parse("nothing here").is_err());
    }
}
