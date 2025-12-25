//! GPT-OSS agent wrapper
//!
//! Provides configuration and connection helpers for GPT-OSS local inference.

use std::path::{Path, PathBuf};

use crate::AgentCommand;
use crate::connection::AcpAgentConnection;
use crate::error::{AcpError, Result};

/// Configuration for GPT-OSS agent
#[derive(Debug, Clone)]
pub struct GptOssAgentConfig {
    /// Path to openagents executable (auto-detected if None)
    pub executable_path: Option<PathBuf>,

    /// Model to use (e.g., "gpt-oss-20b", "gpt-oss-120b")
    pub model: Option<String>,

    /// Base URL for llama.cpp server
    pub server_url: Option<String>,

    /// Maximum turns before stopping
    pub max_turns: Option<u32>,

    /// Permission mode ("default", "plan", "bypassPermissions")
    pub permission_mode: Option<String>,

    /// System prompt override
    pub system_prompt: Option<String>,

    /// Maximum budget in USD
    pub max_budget_usd: Option<f64>,

    /// Enable trajectory recording
    pub record_trajectory: bool,
}

impl Default for GptOssAgentConfig {
    fn default() -> Self {
        Self {
            executable_path: None,
            model: Some("gpt-oss-20b".to_string()),
            server_url: Some("http://localhost:8000".to_string()),
            max_turns: Some(100),
            permission_mode: Some("default".to_string()),
            system_prompt: None,
            max_budget_usd: None,
            record_trajectory: false,
        }
    }
}

impl GptOssAgentConfig {
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

    /// Set the server URL
    pub fn server_url(mut self, url: impl Into<String>) -> Self {
        self.server_url = Some(url.into());
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

    /// Enable trajectory recording
    pub fn record_trajectory(mut self, enabled: bool) -> Self {
        self.record_trajectory = enabled;
        self
    }
}

/// Connect to GPT-OSS agent
///
/// Spawns the openagents CLI with GPT-OSS as a subprocess and establishes an ACP connection.
pub async fn connect_gpt_oss(
    config: GptOssAgentConfig,
    root_dir: &Path,
) -> Result<AcpAgentConnection> {
    let executable = config.executable_path.unwrap_or_else(|| {
        find_openagents_executable().unwrap_or_else(|| PathBuf::from("openagents"))
    });

    if !executable.exists() && which::which("openagents").is_err() {
        return Err(AcpError::AgentNotFound(
            "openagents executable not found. Install with: cargo install --path . from the repository root".to_string(),
        ));
    }

    let mut cmd = AgentCommand::new(executable);

    // Add autopilot subcommand
    cmd = cmd.arg("autopilot").arg("run");

    // Set working directory
    cmd = cmd.arg("--workdir").arg(root_dir.display().to_string());

    // Set agent type to gpt-oss
    cmd = cmd.arg("--agent").arg("gpt-oss");

    // Add model if specified
    if let Some(model) = config.model {
        cmd = cmd.arg("--model").arg(model);
    }

    // Add server URL
    if let Some(url) = config.server_url {
        cmd = cmd.env("GPT_OSS_URL", &url);
        cmd = cmd.env("GPT_OSS_SERVER_URL", url);
    }

    // Add max turns
    if let Some(max_turns) = config.max_turns {
        cmd = cmd.arg("--max-turns").arg(max_turns.to_string());
    }

    // Add permission mode
    if let Some(mode) = config.permission_mode {
        cmd = cmd.arg("--permission-mode").arg(mode);
    }

    // Add system prompt if specified
    if let Some(prompt) = config.system_prompt {
        cmd = cmd.env("SYSTEM_PROMPT_OVERRIDE", prompt);
    }

    // Add budget if specified
    if let Some(budget) = config.max_budget_usd {
        cmd = cmd.arg("--max-budget-usd").arg(budget.to_string());
    }

    // Enable trajectory recording
    if config.record_trajectory {
        cmd = cmd.arg("--record-trajectory");
    }

    // Enable ACP mode
    cmd = cmd.arg("--acp");

    // Create the connection
    AcpAgentConnection::stdio("gpt-oss", cmd, root_dir).await
}

/// Find the openagents executable
///
/// Searches in common locations:
/// 1. Current directory's target/release
/// 2. Current directory's target/debug
/// 3. System PATH
fn find_openagents_executable() -> Option<PathBuf> {
    // Try release build
    let release_path = PathBuf::from("target/release/openagents");
    if release_path.exists() {
        return Some(release_path);
    }

    // Try debug build
    let debug_path = PathBuf::from("target/debug/openagents");
    if debug_path.exists() {
        return Some(debug_path);
    }

    // Try system PATH
    which::which("openagents").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GptOssAgentConfig::default();
        assert_eq!(config.model, Some("gpt-oss-20b".to_string()));
        assert_eq!(config.server_url, Some("http://localhost:8000".to_string()));
        assert_eq!(config.max_turns, Some(100));
        assert!(!config.record_trajectory);
    }

    #[test]
    fn test_config_builder() {
        let config = GptOssAgentConfig::new()
            .model("gpt-oss-120b")
            .server_url("http://localhost:8081")
            .max_turns(50)
            .record_trajectory(true);

        assert_eq!(config.model, Some("gpt-oss-120b".to_string()));
        assert_eq!(config.server_url, Some("http://localhost:8081".to_string()));
        assert_eq!(config.max_turns, Some(50));
        assert!(config.record_trajectory);
    }
}
