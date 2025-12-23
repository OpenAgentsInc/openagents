//! Claude Code agent wrapper
//!
//! Provides configuration and connection helpers for Claude Code.

use std::path::{Path, PathBuf};

use crate::connection::AcpAgentConnection;
use crate::error::{AcpError, Result};
use crate::AgentCommand;

/// Configuration for Claude Code agent
#[derive(Debug, Clone)]
pub struct ClaudeAgentConfig {
    /// Path to claude executable (auto-detected if None)
    pub executable_path: Option<PathBuf>,

    /// Model to use (e.g., "claude-sonnet-4-20250514")
    pub model: Option<String>,

    /// Maximum turns before stopping
    pub max_turns: Option<u32>,

    /// Permission mode ("default", "plan", "bypassPermissions")
    pub permission_mode: Option<String>,

    /// System prompt override
    pub system_prompt: Option<String>,

    /// Maximum budget in USD
    pub max_budget_usd: Option<f64>,
}

impl Default for ClaudeAgentConfig {
    fn default() -> Self {
        Self {
            executable_path: None,
            model: None,
            max_turns: Some(100),
            permission_mode: Some("default".to_string()),
            system_prompt: None,
            max_budget_usd: None,
        }
    }
}

impl ClaudeAgentConfig {
    /// Create a new config with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the executable path
    pub fn executable_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_path = Some(path.into());
        self
    }

    /// Set the model
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set max turns
    pub fn max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = Some(max_turns);
        self
    }

    /// Set permission mode
    pub fn permission_mode(mut self, mode: impl Into<String>) -> Self {
        self.permission_mode = Some(mode.into());
        self
    }

    /// Set system prompt
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set max budget
    pub fn max_budget_usd(mut self, budget: f64) -> Self {
        self.max_budget_usd = Some(budget);
        self
    }
}

/// Connect to Claude Code agent
///
/// Spawns the Claude Code CLI as a subprocess and establishes an ACP connection.
pub async fn connect_claude(config: ClaudeAgentConfig, root_dir: &Path) -> Result<AcpAgentConnection> {
    let executable = config
        .executable_path
        .unwrap_or_else(|| find_claude_executable().unwrap_or_else(|| PathBuf::from("claude")));

    if !executable.exists() && which::which("claude").is_err() {
        return Err(AcpError::AgentNotFound(
            "Claude Code executable not found. Install with: npm install -g @anthropic-ai/claude-code".to_string(),
        ));
    }

    // Build command arguments
    let mut command = AgentCommand::new(&executable);

    // Required: output format for machine-readable output
    command = command.arg("--output-format").arg("stream-json");

    // Optional: model
    if let Some(model) = &config.model {
        command = command.arg("--model").arg(model);
    }

    // Optional: max turns
    if let Some(max_turns) = config.max_turns {
        command = command.arg("--max-turns").arg(max_turns.to_string());
    }

    // Optional: permission mode
    if let Some(mode) = &config.permission_mode {
        command = command.arg("--permission-mode").arg(mode);
    }

    // Optional: system prompt
    if let Some(prompt) = &config.system_prompt {
        command = command.arg("--system-prompt").arg(prompt);
    }

    // Optional: max budget
    if let Some(budget) = config.max_budget_usd {
        command = command.arg("--max-budget-usd").arg(budget.to_string());
    }

    tracing::info!(
        executable = %executable.display(),
        "Connecting to Claude Code"
    );

    AcpAgentConnection::stdio("Claude Code", command, root_dir).await
}

/// Find the Claude Code executable
///
/// Searches common installation locations.
fn find_claude_executable() -> Option<PathBuf> {
    // Try which first
    if let Ok(path) = which::which("claude") {
        return Some(path);
    }

    // Try common locations
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.claude/local/claude", home),
        format!("{}/.npm-global/bin/claude", home),
        format!("{}/.local/bin/claude", home),
        "/usr/local/bin/claude".to_string(),
        "/opt/homebrew/bin/claude".to_string(),
    ];

    for path in &paths {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = ClaudeAgentConfig::new()
            .model("claude-sonnet-4")
            .max_turns(50)
            .permission_mode("plan");

        assert_eq!(config.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(config.max_turns, Some(50));
        assert_eq!(config.permission_mode, Some("plan".to_string()));
    }
}
