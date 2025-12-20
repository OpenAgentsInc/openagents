//! Configuration types for the Codex Agent SDK.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level SDK configuration options.
///
/// These options control how the SDK connects to and communicates with the Codex CLI.
#[derive(Debug, Clone, Default)]
pub struct CodexOptions {
    /// Override path to the codex executable.
    /// If not specified, the SDK will search PATH and common installation locations.
    pub codex_path_override: Option<PathBuf>,

    /// OpenAI-compatible API base URL.
    /// Maps to the `OPENAI_BASE_URL` environment variable.
    pub base_url: Option<String>,

    /// API key for authentication.
    /// Maps to the `CODEX_API_KEY` or `OPENAI_API_KEY` environment variable.
    pub api_key: Option<String>,

    /// Additional environment variables to set for the codex process.
    pub env: Option<HashMap<String, String>>,
}

/// Sandbox mode for file system access control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    /// Read-only access to the file system.
    ReadOnly,
    /// Write access limited to the workspace directory.
    WorkspaceWrite,
    /// Full access to the file system (dangerous).
    DangerFullAccess,
}

impl SandboxMode {
    /// Returns the CLI argument value for this sandbox mode.
    pub fn as_arg(&self) -> &'static str {
        match self {
            SandboxMode::ReadOnly => "read-only",
            SandboxMode::WorkspaceWrite => "workspace-write",
            SandboxMode::DangerFullAccess => "danger-full-access",
        }
    }
}

/// Approval mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    /// Never ask for approval (auto-deny).
    Never,
    /// Ask for approval on request (default).
    OnRequest,
    /// Auto-approve but ask on failure.
    OnFailure,
    /// Only auto-approve trusted operations.
    Untrusted,
}

impl ApprovalMode {
    /// Returns the CLI config value for this approval mode.
    pub fn as_config_value(&self) -> &'static str {
        match self {
            ApprovalMode::Never => "never",
            ApprovalMode::OnRequest => "on-request",
            ApprovalMode::OnFailure => "on-failure",
            ApprovalMode::Untrusted => "untrusted",
        }
    }
}

/// Model reasoning effort level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelReasoningEffort {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra high reasoning effort.
    Xhigh,
}

impl ModelReasoningEffort {
    /// Returns the CLI config value for this reasoning effort.
    pub fn as_config_value(&self) -> &'static str {
        match self {
            ModelReasoningEffort::Minimal => "minimal",
            ModelReasoningEffort::Low => "low",
            ModelReasoningEffort::Medium => "medium",
            ModelReasoningEffort::High => "high",
            ModelReasoningEffort::Xhigh => "xhigh",
        }
    }
}

/// Thread-level configuration options.
///
/// These options are applied when creating a new thread or resuming an existing one.
#[derive(Debug, Clone, Default)]
pub struct ThreadOptions {
    /// Model to use (e.g., "gpt-4o", "gpt-4o-with-reasoning").
    pub model: Option<String>,

    /// Sandbox mode for file system access.
    pub sandbox_mode: Option<SandboxMode>,

    /// Working directory for the agent.
    pub working_directory: Option<PathBuf>,

    /// Skip the check that requires being inside a git repository.
    pub skip_git_repo_check: bool,

    /// Reasoning effort level for reasoning models.
    pub model_reasoning_effort: Option<ModelReasoningEffort>,

    /// Whether to enable network access in sandbox mode.
    pub network_access_enabled: Option<bool>,

    /// Whether to enable web search capability.
    pub web_search_enabled: Option<bool>,

    /// Tool approval policy.
    pub approval_policy: Option<ApprovalMode>,

    /// Additional directories to allow write access to.
    pub additional_directories: Vec<PathBuf>,
}

impl ThreadOptions {
    /// Create a new ThreadOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the sandbox mode.
    pub fn sandbox_mode(mut self, mode: SandboxMode) -> Self {
        self.sandbox_mode = Some(mode);
        self
    }

    /// Set the working directory.
    pub fn working_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_directory = Some(path.into());
        self
    }

    /// Skip the git repository check.
    pub fn skip_git_repo_check(mut self, skip: bool) -> Self {
        self.skip_git_repo_check = skip;
        self
    }

    /// Set the reasoning effort level.
    pub fn model_reasoning_effort(mut self, effort: ModelReasoningEffort) -> Self {
        self.model_reasoning_effort = Some(effort);
        self
    }

    /// Enable or disable network access.
    pub fn network_access_enabled(mut self, enabled: bool) -> Self {
        self.network_access_enabled = Some(enabled);
        self
    }

    /// Enable or disable web search.
    pub fn web_search_enabled(mut self, enabled: bool) -> Self {
        self.web_search_enabled = Some(enabled);
        self
    }

    /// Set the approval policy.
    pub fn approval_policy(mut self, policy: ApprovalMode) -> Self {
        self.approval_policy = Some(policy);
        self
    }

    /// Add an additional directory for write access.
    pub fn add_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.additional_directories.push(path.into());
        self
    }
}

/// Turn-level configuration options.
///
/// These options are applied to individual turns within a thread.
#[derive(Debug, Clone, Default)]
pub struct TurnOptions {
    /// JSON schema for structured output.
    /// When provided, the agent's response will conform to this schema.
    pub output_schema: Option<serde_json::Value>,
}

impl TurnOptions {
    /// Create a new TurnOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the output schema for structured responses.
    pub fn output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_mode_as_arg() {
        assert_eq!(SandboxMode::ReadOnly.as_arg(), "read-only");
        assert_eq!(SandboxMode::WorkspaceWrite.as_arg(), "workspace-write");
        assert_eq!(SandboxMode::DangerFullAccess.as_arg(), "danger-full-access");
    }

    #[test]
    fn test_thread_options_builder() {
        let options = ThreadOptions::new()
            .model("gpt-4o")
            .sandbox_mode(SandboxMode::ReadOnly)
            .skip_git_repo_check(true);

        assert_eq!(options.model, Some("gpt-4o".to_string()));
        assert_eq!(options.sandbox_mode, Some(SandboxMode::ReadOnly));
        assert!(options.skip_git_repo_check);
    }
}
