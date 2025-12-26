//! FM-Bridge agent wrapper
//!
//! Provides configuration and connection helpers for Apple Foundation Models
//! via the fm-bridge local inference backend.

use std::path::{Path, PathBuf};

use crate::AgentCommand;
use crate::connection::AcpAgentConnection;
use crate::error::{AcpError, Result};

/// Configuration for FM-Bridge agent
#[derive(Debug, Clone)]
pub struct FmBridgeAgentConfig {
    /// Path to openagents executable (auto-detected if None)
    pub executable_path: Option<PathBuf>,

    /// Model to use (e.g., "gpt-4o-mini-2024-07-18")
    pub model: Option<String>,

    /// Base URL for FM bridge server
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

impl Default for FmBridgeAgentConfig {
    fn default() -> Self {
        Self {
            executable_path: None,
            model: Some("gpt-4o-mini-2024-07-18".to_string()),
            server_url: Some("http://localhost:3030".to_string()),
            max_turns: Some(100),
            permission_mode: Some("default".to_string()),
            system_prompt: None,
            max_budget_usd: None,
            record_trajectory: false,
        }
    }
}

impl FmBridgeAgentConfig {
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

/// Connect to FM-Bridge agent
///
/// Spawns the openagents CLI with FM-Bridge as a subprocess and establishes an ACP connection.
pub async fn connect_fm_bridge(
    config: FmBridgeAgentConfig,
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

    // Set agent type to fm-bridge
    cmd = cmd.arg("--agent").arg("fm-bridge");

    // Add model if specified
    if let Some(model) = config.model {
        cmd = cmd.arg("--model").arg(model);
    }

    // Add server URL
    if let Some(url) = config.server_url {
        cmd = cmd.env("FM_BRIDGE_URL", url);
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
    AcpAgentConnection::stdio("fm-bridge", cmd, root_dir).await
}

/// Find the openagents executable
fn find_openagents_executable() -> Option<PathBuf> {
    let release_path = PathBuf::from("target/release/openagents");
    if release_path.exists() {
        return Some(release_path);
    }

    let debug_path = PathBuf::from("target/debug/openagents");
    if debug_path.exists() {
        return Some(debug_path);
    }

    which::which("openagents").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FmBridgeAgentConfig::default();
        assert_eq!(config.model, Some("gpt-4o-mini-2024-07-18".to_string()));
        assert_eq!(config.server_url, Some("http://localhost:3030".to_string()));
        assert_eq!(config.max_turns, Some(100));
        assert!(!config.record_trajectory);
    }

    #[test]
    fn test_config_builder() {
        let config = FmBridgeAgentConfig::new()
            .model("apple-model")
            .server_url("http://localhost:3031")
            .max_turns(50)
            .permission_mode("plan")
            .system_prompt("system prompt")
            .max_budget_usd(0.5)
            .record_trajectory(true);

        assert_eq!(config.model, Some("apple-model".to_string()));
        assert_eq!(config.server_url, Some("http://localhost:3031".to_string()));
        assert_eq!(config.max_turns, Some(50));
        assert_eq!(config.permission_mode, Some("plan".to_string()));
        assert_eq!(config.system_prompt, Some("system prompt".to_string()));
        assert_eq!(config.max_budget_usd, Some(0.5));
        assert!(config.record_trajectory);
    }
}
