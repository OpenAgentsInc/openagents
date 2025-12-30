//! Query options for configuring Claude Code sessions.

use crate::hooks::{HookCallbackMatcher, HookEvent};
use crate::protocol::PermissionMode;
use crate::transport::ExecutableConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub max_thinking_tokens: Option<u32>,

    /// Additional directories Claude can access.
    pub additional_directories: Vec<PathBuf>,

    /// Tools configuration (list of names or preset).
    /// Use this to specify the base set of available built-in tools.
    pub tools: Option<ToolsConfig>,

    /// Allowed tool names (additional filtering on top of tools config).
    pub allowed_tools: Option<Vec<String>>,

    /// Disallowed tool names.
    pub disallowed_tools: Option<Vec<String>>,

    /// System prompt configuration.
    pub system_prompt: Option<SystemPromptConfig>,

    /// Output format for structured responses.
    pub output_format: Option<OutputFormat>,

    /// MCP server configurations.
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Strict validation for MCP server configurations.
    pub strict_mcp_config: bool,

    /// Route permission prompts to an MCP tool.
    pub permission_prompt_tool_name: Option<String>,

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

    /// Sandbox settings.
    pub sandbox: Option<SandboxSettings>,

    /// Plugins to load.
    pub plugins: Vec<PluginConfig>,

    /// Hook callbacks for responding to events during execution.
    /// Keys are hook events, values are lists of callback matchers.
    pub hooks: Option<HashMap<HookEvent, Vec<HookCallbackMatcher>>>,
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
    /// In-process SDK MCP server.
    ///
    /// **Note:** Full in-process MCP server support requires implementing
    /// the MCP protocol. For now, use Stdio with a local process instead.
    ///
    /// This variant exists for API compatibility with the Node.js SDK.
    #[serde(rename = "sdk")]
    Sdk {
        /// Server name identifier.
        name: String,
    },
}

/// Agent definition for custom subagents.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Critical reminder added to system prompt (experimental).
    #[serde(
        rename = "criticalSystemReminder_EXPERIMENTAL",
        skip_serializing_if = "Option::is_none"
    )]
    pub critical_system_reminder_experimental: Option<String>,
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

/// Tools configuration for specifying which tools are available.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolsConfig {
    /// List of specific tool names.
    List(Vec<String>),
    /// Use a preset configuration.
    Preset {
        #[serde(rename = "type")]
        config_type: String,
        preset: ToolPreset,
    },
}

impl ToolsConfig {
    /// Create a tools config with a list of tool names.
    pub fn list(tools: Vec<String>) -> Self {
        Self::List(tools)
    }

    /// Create a tools config with the claude_code preset (all default tools).
    pub fn claude_code_preset() -> Self {
        Self::Preset {
            config_type: "preset".to_string(),
            preset: ToolPreset::ClaudeCode,
        }
    }

    /// Create an empty tools config (no tools).
    pub fn none() -> Self {
        Self::List(vec![])
    }
}

/// Tool preset options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPreset {
    /// Use all default Claude Code tools.
    ClaudeCode,
}

/// Beta features that can be enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SdkBeta {
    /// Enable 1M token context window (Sonnet 4/4.5 only).
    /// Maps to "context-1m-2025-08-07"
    #[serde(rename = "context-1m-2025-08-07")]
    Context1M,
}

impl SdkBeta {
    /// Get the string representation for CLI arguments.
    pub fn as_str(&self) -> &'static str {
        match self {
            SdkBeta::Context1M => "context-1m-2025-08-07",
        }
    }
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

    /// Allow unsandboxed commands.
    #[serde(
        rename = "allowUnsandboxedCommands",
        skip_serializing_if = "Option::is_none"
    )]
    pub allow_unsandboxed_commands: Option<bool>,

    /// Network configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<SandboxNetworkConfig>,

    /// Ignore violations by command name and pattern.
    #[serde(rename = "ignoreViolations", skip_serializing_if = "Option::is_none")]
    pub ignore_violations: Option<HashMap<String, Vec<String>>>,

    /// Enable weaker nested sandbox.
    #[serde(
        rename = "enableWeakerNestedSandbox",
        skip_serializing_if = "Option::is_none"
    )]
    pub enable_weaker_nested_sandbox: Option<bool>,

    /// Commands excluded from sandboxing.
    #[serde(rename = "excludedCommands", skip_serializing_if = "Option::is_none")]
    pub excluded_commands: Option<Vec<String>>,

    /// Custom ripgrep configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ripgrep: Option<RipgrepConfig>,
}

/// Ripgrep configuration for sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipgrepConfig {
    /// Command to use for ripgrep.
    pub command: String,
    /// Additional arguments for ripgrep.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
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

    /// Allow all Unix sockets.
    #[serde(
        rename = "allowAllUnixSockets",
        skip_serializing_if = "Option::is_none"
    )]
    pub allow_all_unix_sockets: Option<bool>,

    /// Allowed network domains.
    #[serde(rename = "allowedDomains", skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,

    /// HTTP proxy port.
    #[serde(rename = "httpProxyPort", skip_serializing_if = "Option::is_none")]
    pub http_proxy_port: Option<u16>,

    /// SOCKS proxy port.
    #[serde(rename = "socksProxyPort", skip_serializing_if = "Option::is_none")]
    pub socks_proxy_port: Option<u16>,
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

    /// Enable dangerous permission bypass (skips all permission checks).
    pub fn dangerously_skip_permissions(mut self, skip: bool) -> Self {
        self.allow_dangerously_skip_permissions = skip;
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

    /// Enable strict validation for MCP server configurations.
    pub fn strict_mcp_config(mut self, strict: bool) -> Self {
        self.strict_mcp_config = strict;
        self
    }

    /// Route permission prompts to an MCP tool.
    pub fn permission_prompt_tool_name(mut self, tool_name: impl Into<String>) -> Self {
        self.permission_prompt_tool_name = Some(tool_name.into());
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

    /// Resume a session at a specific message ID.
    pub fn resume_session_at(mut self, message_id: impl Into<String>) -> Self {
        self.resume_session_at = Some(message_id.into());
        self
    }

    /// Fork a session when resuming.
    pub fn fork_session(mut self, fork: bool) -> Self {
        self.fork_session = fork;
        self
    }

    /// Set setting sources to load (user, project, local).
    /// Required to load skills from `.claude/skills/`.
    pub fn setting_sources(mut self, sources: Vec<SettingSource>) -> Self {
        self.setting_sources = sources;
        self
    }

    /// Add a single setting source.
    pub fn setting_source(mut self, source: SettingSource) -> Self {
        self.setting_sources.push(source);
        self
    }

    /// Set the tools configuration.
    ///
    /// This specifies which built-in tools are available:
    /// - `ToolsConfig::list(vec!["Bash", "Read", "Edit"])` - only these tools
    /// - `ToolsConfig::none()` - disable all built-in tools
    /// - `ToolsConfig::claude_code_preset()` - use all default tools
    pub fn tools(mut self, config: ToolsConfig) -> Self {
        self.tools = Some(config);
        self
    }

    /// Disallow specific tools by name.
    pub fn disallowed_tools(mut self, tools: Vec<String>) -> Self {
        self.disallowed_tools = Some(tools);
        self
    }

    /// Enable a beta feature.
    pub fn beta(mut self, beta: SdkBeta) -> Self {
        self.betas.push(beta.as_str().to_string());
        self
    }

    /// Set hooks for the session.
    ///
    /// Hooks allow you to intercept and modify behavior at various points during execution.
    ///
    /// # Example
    /// ```rust,no_run
    /// use claude_agent_sdk::{QueryOptions, HookEvent, HookCallbackMatcher};
    /// use std::collections::HashMap;
    ///
    /// let mut hooks = HashMap::new();
    /// hooks.insert(HookEvent::PreToolUse, vec![
    ///     HookCallbackMatcher::with_matcher("Bash").timeout(30),
    /// ]);
    ///
    /// let options = QueryOptions::new().hooks(hooks);
    /// ```
    pub fn hooks(mut self, hooks: HashMap<HookEvent, Vec<HookCallbackMatcher>>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// Add a hook for a specific event.
    pub fn hook(mut self, event: HookEvent, matcher: HookCallbackMatcher) -> Self {
        self.hooks
            .get_or_insert_with(HashMap::new)
            .entry(event)
            .or_default()
            .push(matcher);
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

        if let Some(ref mode) = self.permission_mode {
            let mode_str = match mode {
                PermissionMode::Default => "default",
                PermissionMode::AcceptEdits => "acceptEdits",
                PermissionMode::BypassPermissions => "bypassPermissions",
                PermissionMode::Plan => "plan",
                PermissionMode::DontAsk => "dontAsk",
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

        if let Some(tokens) = self.max_thinking_tokens {
            args.push("--max-thinking-tokens".to_string());
            args.push(tokens.to_string());
        }

        for dir in &self.additional_directories {
            args.push("--add-dir".to_string());
            args.push(dir.display().to_string());
        }

        // Handle tools configuration
        if let Some(ref tools_config) = self.tools {
            match tools_config {
                ToolsConfig::List(tools) if tools.is_empty() => {
                    // Empty list disables all tools
                    args.push("--tools".to_string());
                    args.push("".to_string());
                }
                ToolsConfig::List(tools) => {
                    for tool in tools {
                        args.push("--tools".to_string());
                        args.push(tool.clone());
                    }
                }
                ToolsConfig::Preset { preset, .. } => {
                    match preset {
                        ToolPreset::ClaudeCode => {
                            // claude_code preset uses all default tools (no flag needed)
                        }
                    }
                }
            }
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
            args.push("--no-session-persistence".to_string());
        }

        if self.include_partial_messages {
            args.push("--include-partial-messages".to_string());
        }

        if !self.setting_sources.is_empty() {
            let sources: Vec<&str> = self
                .setting_sources
                .iter()
                .map(|source| match source {
                    SettingSource::User => "user",
                    SettingSource::Project => "project",
                    SettingSource::Local => "local",
                })
                .collect();
            args.push("--setting-sources".to_string());
            args.push(sources.join(","));
        }

        for beta in &self.betas {
            args.push("--beta".to_string());
            args.push(beta.clone());
        }

        // Note: MCP servers are passed via Initialize control request, not CLI args

        // Extra args
        for (key, value) in &self.extra_args {
            args.push(format!("--{}", key));
            if let Some(v) = value {
                args.push(v.clone());
            }
        }

        args
    }

    /// Build the SDK MCP servers configuration for the Initialize request.
    ///
    /// Returns a Vec of JSON-serialized MCP server configurations.
    pub fn build_sdk_mcp_servers(&self) -> Option<Vec<String>> {
        if self.mcp_servers.is_empty() {
            return None;
        }

        Some(
            self.mcp_servers
                .iter()
                .map(|(name, config)| {
                    let config_json = serde_json::json!({
                        name: config
                    });
                    config_json.to_string()
                })
                .collect(),
        )
    }
}
