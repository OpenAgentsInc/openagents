//! Codex agent wrapper
//!
//! Provides configuration and connection helpers for OpenAI Codex.

use std::path::{Path, PathBuf};

use crate::AgentCommand;
use crate::connection::AcpAgentConnection;
use crate::error::{AcpError, Result};

/// Configuration for Codex agent
#[derive(Debug, Clone)]
pub struct CodexAgentConfig {
    /// Path to codex executable (auto-detected if None)
    pub executable_path: Option<PathBuf>,

    /// Model to use
    pub model: Option<String>,

    /// Sandbox mode ("workspace-read", "workspace-write", "full-access")
    pub sandbox_mode: Option<String>,

    /// Maximum thinking budget (tokens)
    pub max_thinking_budget: Option<u32>,
}

impl Default for CodexAgentConfig {
    fn default() -> Self {
        Self {
            executable_path: None,
            model: None,
            sandbox_mode: Some("workspace-write".to_string()),
            max_thinking_budget: None,
        }
    }
}

impl CodexAgentConfig {
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

    /// Set sandbox mode
    pub fn sandbox_mode(mut self, mode: impl Into<String>) -> Self {
        self.sandbox_mode = Some(mode.into());
        self
    }

    /// Set max thinking budget
    pub fn max_thinking_budget(mut self, budget: u32) -> Self {
        self.max_thinking_budget = Some(budget);
        self
    }
}

/// Connect to Codex agent
///
/// Spawns the Codex CLI as a subprocess and establishes an ACP connection.
pub async fn connect_codex(
    config: CodexAgentConfig,
    root_dir: &Path,
) -> Result<AcpAgentConnection> {
    let executable = config
        .executable_path
        .unwrap_or_else(|| find_codex_executable().unwrap_or_else(|| PathBuf::from("codex")));

    if !executable.exists() && which::which("codex").is_err() {
        return Err(AcpError::AgentNotFound(
            "Codex executable not found. Install with: npm install -g @openai/codex".to_string(),
        ));
    }

    // Build command arguments
    // Note: Codex uses a different CLI structure than Codex
    let mut command = AgentCommand::new(&executable);

    // Use exec subcommand for interactive mode
    command = command.arg("exec");

    // Enable experimental JSON output for machine-readable communication
    command = command.arg("--experimental-json");

    // Optional: model
    if let Some(model) = &config.model {
        command = command.arg("--model").arg(model);
    }

    // Optional: sandbox mode
    if let Some(mode) = &config.sandbox_mode {
        command = command.arg("--sandbox").arg(mode);
    }

    // Optional: max thinking budget
    if let Some(budget) = config.max_thinking_budget {
        command = command.arg("--max-thinking-budget").arg(budget.to_string());
    }

    tracing::info!(
        executable = %executable.display(),
        "Connecting to Codex"
    );

    AcpAgentConnection::stdio("Codex", command, root_dir).await
}

/// Find the Codex executable
///
/// Searches common installation locations.
fn find_codex_executable() -> Option<PathBuf> {
    // Try which first
    if let Ok(path) = which::which("codex") {
        return Some(path);
    }

    // Try common locations
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.npm-global/bin/codex", home),
        format!("{}/.local/bin/codex", home),
        "/usr/local/bin/codex".to_string(),
        "/opt/homebrew/bin/codex".to_string(),
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
        let config = CodexAgentConfig::new()
            .model("gpt-4")
            .sandbox_mode("full-access")
            .max_thinking_budget(10000);

        assert_eq!(config.model, Some("gpt-4".to_string()));
        assert_eq!(config.sandbox_mode, Some("full-access".to_string()));
        assert_eq!(config.max_thinking_budget, Some(10000));
    }
}
