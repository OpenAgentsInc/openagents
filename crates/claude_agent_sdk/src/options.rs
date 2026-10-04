//! Query options for configuring Claude Code sessions.

use crate::protocol::PermissionMode;
use crate::transport::ExecutableConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Default wait for a control-request response (matches TS `initializeTimeoutMs`).
pub const DEFAULT_CONTROL_TIMEOUT: Duration = Duration::from_secs(60);

/// Options for configuring a query.
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    /// Current working directory for the session.
    pub cwd: Option<PathBuf>,

    /// Claude model to use.
    pub model: Option<String>,

    /// Fallback model if primary fails.
    pub fallback_model: Option<String>,

    /// Permission mode for tool execution.
    pub permission_mode: Option<PermissionMode>,

    /// Allow bypassing all permissions (dangerous).
    pub allow_dangerously_skip_permissions: bool,

    /// Maximum conversation turns.
    pub max_turns: Option<u32>,

    /// Maximum budget in USD.
    pub max_budget_usd: Option<f64>,

    /// Maximum thinking tokens.
    ///
    /// Deprecated by the CLI in favor of [`thinking`]. Ignored when `thinking`
    /// is set.
    pub max_thinking_tokens: Option<u32>,

    /// Thinking/reasoning mode (`--thinking`, `--thinking-display`).
    pub thinking: Option<ThinkingConfig>,

    /// Effort level (`--effort`).
    pub effort: Option<EffortLevel>,

    /// Additional directories Claude can access.
    pub additional_directories: Vec<PathBuf>,

    /// Allowed tool names.
    pub allowed_tools: Option<Vec<String>>,

    /// Disallowed tool names.
    pub disallowed_tools: Option<Vec<String>>,

    /// Base set of built-in tools (`--tools`). Distinct from `allowed_tools`.
    pub tools: Option<ToolsConfig>,

    /// System prompt configuration.
    pub system_prompt: Option<SystemPromptConfig>,

    /// Output format for structured responses.
    ///
    /// The SDK transport is always `--output-format stream-json`. A JSON
    /// Schema here is emitted as `--json-schema`, not a second output format.
    pub output_format: Option<OutputFormat>,

    /// MCP server configurations.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Custom agents.
    pub agents: HashMap<String, AgentDefinition>,

    /// Include partial/streaming messages.
    pub include_partial_messages: bool,

    /// Continue most recent conversation.
    pub continue_session: bool,

    /// Resume a specific session.
    pub resume: Option<String>,

    /// Resume session at a specific message.
    pub resume_session_at: Option<String>,

    /// Fork when resuming.
    pub fork_session: bool,

    /// Enable file checkpointing.
    pub enable_file_checkpointing: bool,

    /// Persist session to disk.
    pub persist_session: bool,

    /// Settings sources to load.
    pub setting_sources: Vec<SettingSource>,

    /// Beta features to enable.
    pub betas: Vec<String>,

    /// Executable configuration.
    pub executable: ExecutableConfig,

    /// Environment variables.
    pub env: Option<HashMap<String, String>>,

    /// Extra CLI arguments.
    pub extra_args: HashMap<String, Option<String>>,

    /// How long to wait for a control-request response.
    ///
    /// `None` uses [`DEFAULT_CONTROL_TIMEOUT`] (60 seconds). A hung CLI
    /// must not park the caller forever; [`crate::Error::ControlTimeout`]
    /// is the named failure.
    pub control_timeout: Option<Duration>,

    /// Sandbox settings.
    pub sandbox: Option<SandboxSettings>,

    /// Plugins to load.
    pub plugins: Vec<PluginConfig>,
}

/// System prompt configuration.
#[derive(Debug, Clone)]
pub enum SystemPromptConfig {
    /// Custom system prompt.
    Custom(String),
    /// Use Claude Code's default prompt.
    Preset {
        /// Additional text to append.
        append: Option<String>,
    },
}

/// Output format configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    pub schema: Value,
}

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpServerConfig {
    /// Stdio-based MCP server.
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        args: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
    },
    /// SSE-based MCP server.
    #[serde(rename = "sse")]
    Sse {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
    /// HTTP-based MCP server.
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
}

/// Agent definition for custom subagents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    /// Description of when to use this agent.
    pub description: String,
    /// System prompt for the agent.
    pub prompt: String,
    /// Allowed tool names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// Disallowed tool names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disallowed_tools: Option<Vec<String>>,
    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModel>,
}

/// Base set of built-in tools (`--tools`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsConfig {
    /// Specific built-in tool names. An empty list disables all tools.
    Names(Vec<String>),
    /// All default Claude Code tools (`--tools default`).
    Default,
}

/// Effort level (`--effort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl EffortLevel {
    fn as_cli_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// How thinking content appears (`--thinking-display`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingDisplay {
    Summarized,
    Omitted,
}

impl ThinkingDisplay {
    fn as_cli_str(self) -> &'static str {
        match self {
            Self::Summarized => "summarized",
            Self::Omitted => "omitted",
        }
    }
}

/// Thinking/reasoning behavior (`--thinking`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ThinkingConfig {
    /// Claude decides when and how much to think.
    Adaptive {
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
    /// Fixed thinking token budget (older models).
    Enabled {
        #[serde(rename = "budgetTokens", skip_serializing_if = "Option::is_none")]
        budget_tokens: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
    /// No extended thinking.
    Disabled,
}

/// Model selection for agents.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentModel {
    Sonnet,
    Opus,
    Haiku,
    Inherit,
}

/// Settings source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingSource {
    User,
    Project,
    Local,
}

/// Sandbox settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxSettings {
    /// Enable sandboxing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Auto-allow bash if sandboxed.
    #[serde(
        rename = "autoAllowBashIfSandboxed",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_allow_bash_if_sandboxed: Option<bool>,
    /// Network configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<SandboxNetworkConfig>,
}

/// Sandbox network configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxNetworkConfig {
    /// Allow local binding.
    #[serde(rename = "allowLocalBinding", skip_serializing_if = "Option::is_none")]
    pub allow_local_binding: Option<bool>,
    /// Allowed Unix sockets.
    #[serde(rename = "allowUnixSockets", skip_serializing_if = "Option::is_none")]
    pub allow_unix_sockets: Option<Vec<String>>,
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginConfig {
    /// Local plugin.
    #[serde(rename = "local")]
    Local { path: String },
}

impl QueryOptions {
    /// Create new options with default settings.
    pub fn new() -> Self {
        Self {
            persist_session: true,
            ..Default::default()
        }
    }

    /// Set the working directory.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set the model to use.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the permission mode.
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_mode = Some(mode);
        self
    }

    /// Set maximum turns.
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Set maximum budget in USD.
    pub fn max_budget_usd(mut self, budget: f64) -> Self {
        self.max_budget_usd = Some(budget);
        self
    }

    /// Add an MCP server.
    pub fn mcp_server(mut self, name: impl Into<String>, config: McpServerConfig) -> Self {
        self.mcp_servers.insert(name.into(), config);
        self
    }

    /// Add a custom agent.
    pub fn agent(mut self, name: impl Into<String>, definition: AgentDefinition) -> Self {
        self.agents.insert(name.into(), definition);
        self
    }

    /// Include partial messages in the stream.
    pub fn include_partial_messages(mut self, include: bool) -> Self {
        self.include_partial_messages = include;
        self
    }

    /// Set the control-request timeout. Tests use a short value so a hung
    /// CLI fails fast; production callers can leave the default.
    pub fn control_timeout(mut self, timeout: Duration) -> Self {
        self.control_timeout = Some(timeout);
        self
    }

    /// Resolved control-request timeout.
    pub fn control_timeout_or_default(&self) -> Duration {
        self.control_timeout.unwrap_or(DEFAULT_CONTROL_TIMEOUT)
    }

    /// Continue the most recent session.
    pub fn continue_session(mut self) -> Self {
        self.continue_session = true;
        self
    }

    /// Resume a specific session by ID.
    pub fn resume(mut self, session_id: impl Into<String>) -> Self {
        self.resume = Some(session_id.into());
        self
    }

    /// Build CLI arguments from options.
    pub fn build_args(&self) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--permission-prompt-tool".to_string(),
            "stdio".to_string(),
        ];

        if let Some(ref model) = self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        if let Some(ref model) = self.fallback_model {
            args.push("--fallback-model".to_string());
            args.push(model.clone());
        }

        if let Some(ref mode) = self.permission_mode {
            let mode_str = match mode {
                PermissionMode::Default => "default",
                PermissionMode::AcceptEdits => "acceptEdits",
                PermissionMode::BypassPermissions => "bypassPermissions",
                PermissionMode::Plan => "plan",
                PermissionMode::DontAsk => "dontAsk",
                PermissionMode::Auto => "auto",
            };
            args.push("--permission-mode".to_string());
            args.push(mode_str.to_string());
        }

        if self.allow_dangerously_skip_permissions {
            args.push("--dangerously-skip-permissions".to_string());
        }

        if let Some(turns) = self.max_turns {
            args.push("--max-turns".to_string());
            args.push(turns.to_string());
        }

        if let Some(budget) = self.max_budget_usd {
            args.push("--max-budget-usd".to_string());
            args.push(budget.to_string());
        }

        if let Some(ref thinking) = self.thinking {
            match thinking {
                ThinkingConfig::Adaptive { display } => {
                    args.push("--thinking".to_string());
                    args.push("adaptive".to_string());
                    if let Some(display) = display {
                        args.push("--thinking-display".to_string());
                        args.push(display.as_cli_str().to_string());
                    }
                }
                ThinkingConfig::Enabled {
                    budget_tokens,
                    display,
                } => {
                    if let Some(tokens) = budget_tokens {
                        args.push("--max-thinking-tokens".to_string());
                        args.push(tokens.to_string());
                    } else {
                        args.push("--thinking".to_string());
                        args.push("adaptive".to_string());
                    }
                    if let Some(display) = display {
                        args.push("--thinking-display".to_string());
                        args.push(display.as_cli_str().to_string());
                    }
                }
                ThinkingConfig::Disabled => {
                    args.push("--thinking".to_string());
                    args.push("disabled".to_string());
                }
            }
        } else if let Some(tokens) = self.max_thinking_tokens {
            args.push("--max-thinking-tokens".to_string());
            args.push(tokens.to_string());
        }

        if let Some(effort) = self.effort {
            args.push("--effort".to_string());
            args.push(effort.as_cli_str().to_string());
        }

        for dir in &self.additional_directories {
            args.push("--add-dir".to_string());
            args.push(dir.display().to_string());
        }

        if let Some(ref tools) = self.allowed_tools {
            for tool in tools {
                args.push("--allowed-tools".to_string());
                args.push(tool.clone());
            }
        }

        if let Some(ref tools) = self.disallowed_tools {
            for tool in tools {
                args.push("--disallowed-tools".to_string());
                args.push(tool.clone());
            }
        }

        if let Some(ref tools) = self.tools {
            match tools {
                ToolsConfig::Default => {
                    args.push("--tools".to_string());
                    args.push("default".to_string());
                }
                ToolsConfig::Names(names) => {
                    args.push("--tools".to_string());
                    args.push(names.join(","));
                }
            }
        }

        match &self.system_prompt {
            Some(SystemPromptConfig::Custom(prompt)) => {
                args.push("--system-prompt".to_string());
                args.push(prompt.clone());
            }
            Some(SystemPromptConfig::Preset {
                append: Some(append),
            }) => {
                args.push("--append-system-prompt".to_string());
                args.push(append.clone());
            }
            Some(SystemPromptConfig::Preset { append: None }) | None => {}
        }

        if !self.mcp_servers.is_empty() {
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "mcpServers": self.mcp_servers,
            })) {
                args.push("--mcp-config".to_string());
                args.push(json);
            }
        }

        if !self.agents.is_empty() {
            if let Ok(json) = serde_json::to_string(&self.agents) {
                args.push("--agents".to_string());
                args.push(json);
            }
        }

        if let Some(ref sandbox) = self.sandbox {
            // Main `claude` rejects `--sandbox` (`unknown option`). The TS SDK
            // writes sandbox settings into `--settings` JSON.
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "sandbox": sandbox,
            })) {
                args.push("--settings".to_string());
                args.push(json);
            }
        }

        if let Some(ref format) = self.output_format {
            if !format.schema.is_null() {
                if let Ok(schema) = serde_json::to_string(&format.schema) {
                    args.push("--json-schema".to_string());
                    args.push(schema);
                }
            }
        }

        if self.continue_session {
            args.push("--continue".to_string());
        }

        if let Some(ref session_id) = self.resume {
            args.push("--resume".to_string());
            args.push(session_id.clone());
        }

        if let Some(ref at) = self.resume_session_at {
            args.push("--resume-session-at".to_string());
            args.push(at.clone());
        }

        if self.fork_session {
            args.push("--fork-session".to_string());
        }

        if self.enable_file_checkpointing {
            args.push("--enable-file-checkpointing".to_string());
        }

        if !self.persist_session {
            args.push("--no-persist-session".to_string());
        }

        if self.include_partial_messages {
            args.push("--include-partial-messages".to_string());
        }

        for source in &self.setting_sources {
            let source_str = match source {
                SettingSource::User => "user",
                SettingSource::Project => "project",
                SettingSource::Local => "local",
            };
            args.push("--setting-source".to_string());
            args.push(source_str.to_string());
        }

        for beta in &self.betas {
            args.push("--beta".to_string());
            args.push(beta.clone());
        }

        for plugin in &self.plugins {
            match plugin {
                PluginConfig::Local { path } => {
                    args.push("--plugin-dir".to_string());
                    args.push(path.clone());
                }
            }
        }

        // Extra args
        for (key, value) in &self.extra_args {
            args.push(format!("--{}", key));
            if let Some(v) = value {
                args.push(v.clone());
            }
        }

        args
    }
}
