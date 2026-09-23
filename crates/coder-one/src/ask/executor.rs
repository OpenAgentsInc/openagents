//! Runs an ask's executor: Codex on GPT-6 Luna or Claude Code on Opus,
//! inside the ask's filesystem boundary, with the briefing on standard
//! input and the two ask tools as its only tools.
//!
//! The CLI, its credential, its stream, and how a session ended are read
//! the way delegate mode reads them ([`crate::delegate`]): the same binary
//! resolution, the same [`SummaryReader`], and the same five statuses. What
//! differs is the tool surface. Claude Code runs with `--tools ""` and an
//! MCP config naming only `coder-one ask tools`, allowed through
//! `--allowedTools` under `--permission-mode dontAsk`. Codex runs with its
//! user configuration ignored, its shell tools and connectors disabled, and
//! the same server added. Both see `read` and `answer` and nothing else
//! that runs a command; the boundary denies writes to the rest.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::delegate::{self, Agent, Credential, Status, Summary, SummaryReader};

use super::Progress;
use super::tools::{ANSWER_FILE, CALLS_FILE};

/// Which executor answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// Codex CLI on GPT-6 Luna: fast and cheap.
    Luna,
    /// Claude Code on Opus 5.5.
    Opus,
}

impl Which {
    /// Parses `luna` or `opus`.
    ///
    /// # Errors
    ///
    /// Returns a message naming the choices.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "luna" => Ok(Which::Luna),
            "opus" => Ok(Which::Opus),
            other => Err(format!("--executor takes luna or opus, not {other}")),
        }
    }

    /// The word the record uses.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Which::Luna => "luna",
            Which::Opus => "opus",
        }
    }

    /// The CLI that runs it.
    #[must_use]
    pub fn agent(self) -> Agent {
        match self {
            Which::Luna => Agent::Codex,
            Which::Opus => Agent::ClaudeCode,
        }
    }
}

/// The Codex features an ask turns off: connectors, browsers, image and
/// goal tools, plugins, sub-agents, and every shell. Each tool left on is
/// a prompt the model reads on every turn.
pub const CODEX_DISABLED: [&str; 17] = [
    "apps",
    "browser_use",
    "browser_use_external",
    "computer_use",
    "goals",
    "hooks",
    "image_generation",
    "multi_agent",
    "plugins",
    "remote_plugin",
    "shell_tool",
    "skill_search",
    "sleep_tool",
    "tool_suggest",
    "unified_exec",
    "view_image",
    "workspace_dependencies",
];

/// What one executor run needs.
pub struct Setup<'a> {
    pub which: Which,
    pub model: String,
    pub binary: Option<PathBuf>,
    pub credential: Credential,
    /// This binary, which serves the tools.
    pub coder_one: PathBuf,
    pub gym: PathBuf,
    /// Where commands run: the repository.
    pub cwd: PathBuf,
    /// The writable scratch directory the tools write to.
    pub scratch: PathBuf,
    pub briefing: PathBuf,
    /// What the executor may spend; Claude Code enforces it.
    pub budget_usd: f64,
    pub deadline: Duration,
    pub rank: bool,
    pub boundary: &'a coder_boundary::Boundary,
}

/// How the executor ran.
#[derive(Clone, Debug)]
pub struct Ran {
    pub status: Status,
    pub summary: Summary,
    /// The `answer` tool's arguments, when it was called.
    pub answer: Option<Value>,
    /// Every tool call the server recorded.
    pub calls: Vec<Value>,
    /// The raw stream.
    pub stream: String,
    pub stderr: String,
    pub milliseconds: u64,
    /// The argument list, credentials excluded, for the record.
    pub args: Vec<String>,
}

impl Setup<'_> {
    /// The server's command line: `coder-one ask tools …`.
    fn server_args(&self) -> Vec<String> {
        let mut args = vec![
            "ask".to_string(),
            "tools".to_string(),
            "--gym".to_string(),
            self.gym.to_string_lossy().into_owned(),
            "--scratch".to_string(),
            self.scratch.to_string_lossy().into_owned(),
            "--cwd".to_string(),
            self.cwd.to_string_lossy().into_owned(),
        ];
        if self.rank {
            args.push("--rank".to_string());
        }
        args
    }

    /// The CLI's arguments.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        let toml = |value: &Value| value.to_string();
        match self.which.agent() {
            Agent::Codex => {
                let mut args: Vec<String> = [
                    "exec",
                    "--json",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "--ignore-user-config",
                    "--ignore-rules",
                    "-m",
                ]
                .map(str::to_string)
                .to_vec();
                args.push(self.model.clone());
                for feature in CODEX_DISABLED {
                    args.extend(["--disable".to_string(), feature.to_string()]);
                }
                args.extend([
                    "-c".to_string(),
                    "web_search=\"disabled\"".to_string(),
                    "-c".to_string(),
                    format!(
                        "mcp_servers.ask.command={}",
                        toml(&json!(self.coder_one.to_string_lossy()))
                    ),
                    "-c".to_string(),
                    format!("mcp_servers.ask.args={}", toml(&json!(self.server_args()))),
                    "-c".to_string(),
                    "mcp_servers.ask.tool_timeout_sec=120".to_string(),
                    "--dangerously-bypass-approvals-and-sandbox".to_string(),
                    "-".to_string(),
                ]);
                args
            }
            Agent::ClaudeCode => {
                let config = json!({
                    "mcpServers": {
                        "ask": {
                            "type": "stdio",
                            "command": self.coder_one.to_string_lossy(),
                            "args": self.server_args(),
                        }
                    }
                });
                vec![
                    "-p".to_string(),
                    "--output-format".to_string(),
                    "stream-json".to_string(),
                    "--verbose".to_string(),
                    "--model".to_string(),
                    self.model.clone(),
                    "--mcp-config".to_string(),
                    config.to_string(),
                    "--strict-mcp-config".to_string(),
                    "--tools".to_string(),
                    String::new(),
                    "--allowedTools".to_string(),
                    "mcp__ask__read,mcp__ask__answer".to_string(),
                    "--permission-mode".to_string(),
                    "dontAsk".to_string(),
                    "--max-budget-usd".to_string(),
                    format!("{:.4}", self.budget_usd.max(0.01)),
                ]
            }
        }
    }

    /// Runs the executor to its end, printing each tool call as the
    /// server records it.
    pub async fn run(&self, progress: &Progress) -> Ran {
        let started = Instant::now();
        let args = self.args();
        let harness = |why: String| Ran {
            status: Status::Harness(why),
            summary: Summary::default(),
            answer: None,
            calls: Vec::new(),
            stream: String::new(),
            stderr: String::new(),
            milliseconds: 0,
            args: args.clone(),
        };
        let Some(binary) = &self.binary else {
            let agent = self.which.agent();
            return harness(format!(
                "no {} binary: set {} or put it on PATH",
                agent.program(),
                agent.binary_variable()
            ));
        };
        if self.credential == Credential::Missing {
            return harness(format!(
                "{} has no credential: sign in to it first",
                self.which.agent().program()
            ));
        }
        let Some(sh) = super::allow::which("sh") else {
            return harness("sh is not on PATH".to_string());
        };
        let stream_path = self.scratch.join("stream.jsonl");
        let mut script_args = vec![
            "-c".to_string(),
            "exec \"$@\" < \"$ASK_BRIEFING\" > \"$ASK_STREAM\"".to_string(),
            "sh".to_string(),
            binary.to_string_lossy().into_owned(),
        ];
        script_args.extend(args.iter().cloned());
        let mut command = match self.boundary.command(&sh, &script_args) {
            Ok(command) => command,
            Err(error) => return harness(format!("cannot bound the executor: {error}")),
        };
        command
            .current_dir(&self.cwd)
            .env("ASK_BRIEFING", &self.briefing)
            .env("ASK_STREAM", &stream_path)
            // A parent Claude Code session's marker makes the CLI refuse to
            // start; the executor is its own session.
            .env_remove("CLAUDECODE")
            .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
            // The executor needs neither of Coder One's own credentials.
            .env_remove("TYPESAFE_API_KEY")
            .env_remove("OPENAGENTS_API_KEY");
        if self.credential == Credential::OauthToken {
            command
                .env_remove("ANTHROPIC_API_KEY")
                .env_remove("ANTHROPIC_AUTH_TOKEN");
        }
        progress.line(&format!(
            "executor ▸ {} ({}) · deadline {}s · budget ${:.2}",
            self.which.agent().word(),
            self.model,
            self.deadline.as_secs(),
            self.budget_usd
        ));
        let job = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(self.deadline).keeping(256 * 1024))
            .run();
        tokio::pin!(job);
        let mut seen = 0usize;
        let ended = loop {
            tokio::select! {
                ended = &mut job => break ended,
                () = tokio::time::sleep(Duration::from_millis(400)) => {
                    seen = self.announce(progress, seen);
                }
            }
        };
        self.announce(progress, seen);
        let stream = std::fs::read_to_string(&stream_path).unwrap_or_default();
        let mut reader = SummaryReader::of(self.which.agent(), &self.model);
        for line in stream.lines() {
            reader.line(line);
        }
        let mut summary = reader.finish();
        if self.which == Which::Opus && summary.total_cost_usd.is_some() {
            summary.cost_provenance = Some(self.credential.cost_provenance());
        }
        let stderr = ended.stderr.marked();
        let status = delegate::classify(&ended.ending, &summary, &stderr);
        Ran {
            status,
            summary,
            answer: read_json(&self.scratch.join(ANSWER_FILE)),
            calls: read_lines(&self.scratch.join(CALLS_FILE)),
            stream,
            stderr,
            milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            args,
        }
    }

    /// Prints the tool calls recorded since the first `seen`, and returns
    /// how many there are now.
    fn announce(&self, progress: &Progress, seen: usize) -> usize {
        let calls = read_lines(&self.scratch.join(CALLS_FILE));
        for call in calls.iter().skip(seen) {
            match call["tool"].as_str() {
                Some("read") => progress.line(&format!(
                    "executor ▸ read: {}{}",
                    call["command"].as_str().unwrap_or("?"),
                    match call["refused"].as_str() {
                        Some(why) => format!(" (refused: {why})"),
                        None => format!(" ({})", call["ending"].as_str().unwrap_or("?")),
                    }
                )),
                Some("answer") => {
                    progress.line(&format!("executor ▸ answer with {} claims", call["claims"]))
                }
                _ => {}
            }
        }
        calls.len()
    }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn read_lines(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_executor_sees_only_the_ask_tools() {
        let boundary = coder_boundary::Boundary::readonly().build();
        let Ok(boundary) = boundary else {
            return;
        };
        let mut setup = Setup {
            which: Which::Luna,
            model: "gpt-6-luna".to_string(),
            binary: None,
            credential: Credential::CodexAuthFile,
            coder_one: PathBuf::from("/opt/coder-one"),
            gym: PathBuf::from("/opt/gym"),
            cwd: PathBuf::from("/repo"),
            scratch: PathBuf::from("/scratch"),
            briefing: PathBuf::from("/b.md"),
            budget_usd: 0.5,
            deadline: Duration::from_secs(60),
            rank: false,
            boundary: &boundary,
        };
        let codex = setup.args().join(" ");
        for needle in [
            "--disable shell_tool",
            "--disable unified_exec",
            "--ignore-user-config",
            "mcp_servers.ask.command=\"/opt/coder-one\"",
            "mcp_servers.ask.args=[\"ask\",\"tools\",\"--gym\",\"/opt/gym\"",
        ] {
            assert!(codex.contains(needle), "{needle}: {codex}");
        }
        assert!(!codex.contains("--rank"), "{codex}");
        setup.which = Which::Opus;
        setup.rank = true;
        let claude = setup.args();
        let at = |flag: &str| claude.iter().position(|a| a == flag).unwrap();
        assert_eq!(claude[at("--tools") + 1], "");
        assert_eq!(
            claude[at("--allowedTools") + 1],
            "mcp__ask__read,mcp__ask__answer"
        );
        assert_eq!(claude[at("--permission-mode") + 1], "dontAsk");
        assert_eq!(claude[at("--max-budget-usd") + 1], "0.5000");
        assert!(claude[at("--mcp-config") + 1].contains("\"--rank\""));
        assert!(claude.contains(&"--strict-mcp-config".to_string()));
    }
}
