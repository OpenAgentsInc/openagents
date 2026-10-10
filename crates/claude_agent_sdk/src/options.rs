//! Query options for configuring Claude Code sessions.
//!
//! [`QueryOptions::build_args`] follows the argument builder of
//! `@anthropic-ai/claude-agent-sdk` 0.3.296, checked against
//! `claude --help` for Claude Code 2.1.295.

use crate::callbacks::{ElicitationHandler, HookMatcher, UserDialogHandler};
use crate::error::{Error, Result};
use crate::protocol::{HookEvent, InitializeRequest, PermissionMode, SdkHookCallbackMatcher};
use crate::transport::ExecutableConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
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

    /// Fallback model if primary fails. Must differ from `model`.
    pub fallback_model: Option<String>,

    /// Agent for the session (`--agent`).
    pub agent: Option<String>,

    /// Permission mode for tool execution.
    pub permission_mode: Option<PermissionMode>,

    /// Make `bypassPermissions` available without turning it on
    /// (`--allow-dangerously-skip-permissions`).
    pub allow_dangerously_skip_permissions: bool,

    /// MCP tool that answers permission prompts instead of the SDK host
    /// (`--permission-prompt-tool`). Cannot be combined with a permission
    /// handler.
    pub permission_prompt_tool_name: Option<String>,

    /// Who answers permission prompts (`--permission-prompts`).
    pub permission_prompts: Option<PermissionPrompts>,

    /// Maximum conversation turns.
    pub max_turns: Option<u32>,

    /// Maximum budget in USD.
    pub max_budget_usd: Option<f64>,

    /// Total task token budget (`--task-budget`).
    pub task_budget: Option<u64>,

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

    /// Allowed tool names (`--allowedTools`, comma-joined).
    pub allowed_tools: Option<Vec<String>>,

    /// Disallowed tool names (`--disallowedTools`, comma-joined).
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

    /// Use only the MCP servers in `mcp_servers` (`--strict-mcp-config`).
    pub strict_mcp_config: bool,

    /// Custom agents.
    pub agents: HashMap<String, AgentDefinition>,

    /// Host hook callbacks, registered in `initialize`.
    pub hooks: HashMap<HookEvent, Vec<HookMatcher>>,

    /// Emit hook lifecycle messages (`--include-hook-events`).
    pub include_hook_events: bool,

    /// Answers MCP elicitation. Without one, elicitation is declined.
    pub on_elicitation: Option<Arc<dyn ElicitationHandler>>,

    /// Answers `request_user_dialog`. Without one, the request gets no
    /// response, as in the TS SDK.
    pub on_user_dialog: Option<Arc<dyn UserDialogHandler>>,

    /// Dialog kinds the host can show (`initialize.supportedDialogKinds`).
    pub supported_dialog_kinds: Option<Vec<String>>,

    /// The host renders a per-task stop control
    /// (`initialize.perTaskStopAffordance`).
    pub per_task_stop_affordance: bool,

    /// Emit predicted next prompts (`initialize.promptSuggestions`).
    pub prompt_suggestions: bool,

    /// Summarize subagent progress (`initialize.agentProgressSummaries`).
    pub agent_progress_summaries: bool,

    /// Forward subagent text (`initialize.forwardSubagentText`).
    pub forward_subagent_text: bool,

    /// Session title (`initialize.title`).
    pub title: Option<String>,

    /// Skills to enable (`initialize.skills`).
    pub skills: Option<Vec<String>>,

    /// Plan-mode instructions (`initialize.planModeInstructions`).
    pub plan_mode_instructions: Option<String>,

    /// Tool name aliases (`initialize.toolAliases`).
    pub tool_aliases: Option<HashMap<String, String>>,

    /// Send every prompt with `client_composed: true`: no `@path`
    /// expansion and no slash-command dispatch.
    pub verbatim_prompts: bool,

    /// Include partial/streaming messages.
    pub include_partial_messages: bool,

    /// Continue most recent conversation.
    pub continue_session: bool,

    /// Resume a specific session.
    pub resume: Option<String>,

    /// Resume session at a specific message.
    pub resume_session_at: Option<String>,

    /// Drop a turn when resuming (`--resume-drops-turn`).
    pub resume_drops_turn: Option<String>,

    /// Use a specific session ID (`--session-id`).
    pub session_id: Option<String>,

    /// Fork when resuming.
    pub fork_session: bool,

    /// Enable file checkpointing (sets
    /// `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING`; there is no flag).
    pub enable_file_checkpointing: bool,

    /// Persist session to disk. `false` emits `--no-session-persistence`.
    pub persist_session: bool,

    /// Settings sources to load. `None` leaves the CLI default; an empty
    /// list loads none.
    pub setting_sources: Option<Vec<SettingSource>>,

    /// Settings: a file path (string) or a settings object (`--settings`).
    pub settings: Option<Value>,

    /// Managed settings (`--managed-settings`).
    pub managed_settings: Option<String>,

    /// Trusted checkout that a worktree `cwd` belongs to
    /// (`--project-config-root`).
    pub project_config_root: Option<PathBuf>,

    /// Beta features to enable.
    pub betas: Vec<String>,

    /// Debug mode (`--debug`).
    pub debug: bool,

    /// Debug log file (`--debug-file`).
    pub debug_file: Option<PathBuf>,

    /// Executable configuration.
    pub executable: ExecutableConfig,

    /// Environment variables.
    pub env: Option<HashMap<String, String>>,

    /// Variables the CLI does not inherit from this process, such as
    /// `ANTHROPIC_API_KEY` when the session must use the Claude Code login.
    pub env_remove: Vec<String>,

    /// Extra CLI arguments.
    pub extra_args: HashMap<String, Option<String>>,

    /// How long to wait for a control-request response.
    ///
    /// `None` uses [`DEFAULT_CONTROL_TIMEOUT`] (60 seconds). A hung CLI
    /// must not park the caller forever; [`crate::Error::ControlTimeout`]
    /// is the named failure.
    pub control_timeout: Option<Duration>,

    /// Sandbox settings, merged into `--settings`.
    pub sandbox: Option<SandboxSettings>,

    /// Plugins to load.
    pub plugins: Vec<PluginConfig>,

    /// How plugins reach the CLI.
    pub plugin_delivery: PluginDelivery,
}

/// Who answers permission prompts (`--permission-prompts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPrompts {
    /// The SDK host or the permission prompt tool (CLI default).
    Host,
    /// Nobody: anything that would prompt is denied.
    None,
}

/// How plugins reach the CLI (TS `pluginDelivery`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PluginDelivery {
    /// One `--plugin-dir` flag per plugin.
    #[default]
    Argv,
    /// The list goes in the `initialize` request and the CLI starts with
    /// `--await-initialize` (Claude Code 2.1.261 or later).
    Initialize,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Token count at which the agent compacts its own conversation when
    /// it runs as a subagent; it only lowers the inherited window
    /// (`autoCompactWindow`, 0.3.296).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_compact_window: Option<u64>,
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

impl SettingSource {
    fn as_cli_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

/// Sandbox settings (the TS `sandbox` option).
///
/// Merged into `--settings`. When `settings` is inline and has its own
/// `sandbox` block, the two merge as in the TS SDK 0.3.296: a value set
/// here replaces the value at the same path, a value not set here is kept,
/// and the restriction lists from both sides are combined. Fields this
/// struct does not name go in `extra`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxSettings {
    /// Enable sandboxing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Exit at startup when an enabled sandbox cannot start. Defaults to
    /// `true` when `enabled` is set and neither side sets it.
    #[serde(rename = "failIfUnavailable", skip_serializing_if = "Option::is_none")]
    pub fail_if_unavailable: Option<bool>,
    /// Auto-allow bash if sandboxed.
    #[serde(
        rename = "autoAllowBashIfSandboxed",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_allow_bash_if_sandboxed: Option<bool>,
    /// Network configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<SandboxNetworkConfig>,
    /// Filesystem restrictions (`denyRead`, `denyWrite`, `disabled`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<Value>,
    /// Credential restrictions (`files`, `envVars`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Value>,
    /// Any other sandbox setting, passed through as written.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
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
    /// Domains commands may reach.
    #[serde(rename = "allowedDomains", skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    /// Domains commands may not reach.
    #[serde(rename = "deniedDomains", skip_serializing_if = "Option::is_none")]
    pub denied_domains: Option<Vec<String>>,
    /// Allow only `allowed_domains`.
    #[serde(rename = "strictAllowlist", skip_serializing_if = "Option::is_none")]
    pub strict_allowlist: Option<bool>,
    /// Any other network setting, passed through as written.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PluginConfig {
    /// Local plugin.
    #[serde(rename = "local")]
    Local {
        path: String,
        /// Load the plugin without its MCP servers (`--plugin-dir-no-mcp`).
        #[serde(
            rename = "skipMcpDiscovery",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        skip_mcp_discovery: Option<bool>,
    },
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

    /// Register a hook matcher for an event.
    pub fn hook(mut self, event: HookEvent, matcher: HookMatcher) -> Self {
        self.hooks.entry(event).or_default().push(matcher);
        self
    }

    /// Include partial messages in the stream.
    pub fn include_partial_messages(mut self, include: bool) -> Self {
        self.include_partial_messages = include;
        self
    }

    /// Keep the session out of the CLI's saved history
    /// (`--no-session-persistence`).
    pub fn no_session_persistence(mut self) -> Self {
        self.persist_session = false;
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

    /// Reject option combinations the TS SDK rejects.
    pub fn validate(&self, has_permission_handler: bool) -> Result<()> {
        if let (Some(model), Some(fallback)) = (&self.model, &self.fallback_model)
            && model == fallback
        {
            return Err(Error::InvalidOptions(
                "fallback model cannot be the same as the main model".to_string(),
            ));
        }
        if has_permission_handler && self.permission_prompt_tool_name.is_some() {
            return Err(Error::InvalidOptions(
                "a permission handler cannot be combined with permission_prompt_tool_name"
                    .to_string(),
            ));
        }
        if self.sandbox.is_some()
            && matches!(&self.settings, Some(Value::String(path)) if !is_inline_json(path))
        {
            return Err(Error::InvalidOptions(
                "cannot use both a settings file path and the sandbox option".to_string(),
            ));
        }
        Ok(())
    }

    /// Build CLI arguments, assuming no SDK permission handler.
    pub fn build_args(&self) -> Vec<String> {
        self.build_args_for(false)
    }

    /// Build CLI arguments. With `has_permission_handler`, permission
    /// prompts come to the SDK (`--permission-prompt-tool stdio`), as the
    /// TS SDK does when `canUseTool` is set.
    pub fn build_args_for(&self, has_permission_handler: bool) -> Vec<String> {
        let mut args: Vec<String> = [
            "--output-format",
            "stream-json",
            "--verbose",
            "--input-format",
            "stream-json",
        ]
        .map(String::from)
        .to_vec();

        if let Some(ref thinking) = self.thinking {
            match thinking {
                ThinkingConfig::Adaptive { .. } => {
                    push_pair(&mut args, "--thinking", "adaptive".into())
                }
                ThinkingConfig::Enabled { budget_tokens, .. } => match budget_tokens {
                    Some(tokens) => {
                        push_pair(&mut args, "--max-thinking-tokens", tokens.to_string())
                    }
                    None => push_pair(&mut args, "--thinking", "adaptive".into()),
                },
                ThinkingConfig::Disabled => push_pair(&mut args, "--thinking", "disabled".into()),
            }
            let display = match thinking {
                ThinkingConfig::Adaptive { display } | ThinkingConfig::Enabled { display, .. } => {
                    *display
                }
                ThinkingConfig::Disabled => None,
            };
            if let Some(display) = display {
                push_pair(&mut args, "--thinking-display", display.as_cli_str().into());
            }
        } else if let Some(tokens) = self.max_thinking_tokens {
            push_pair(&mut args, "--max-thinking-tokens", tokens.to_string());
        }
        if let Some(effort) = self.effort {
            push_pair(&mut args, "--effort", effort.as_cli_str().into());
        }
        if let Some(turns) = self.max_turns {
            push_pair(&mut args, "--max-turns", turns.to_string());
        }
        if let Some(budget) = self.max_budget_usd {
            push_pair(&mut args, "--max-budget-usd", budget.to_string());
        }
        if let Some(total) = self.task_budget {
            push_pair(&mut args, "--task-budget", total.to_string());
        }
        if let Some(ref model) = self.model {
            push_pair(&mut args, "--model", model.clone());
        }
        if let Some(ref agent) = self.agent {
            push_pair(&mut args, "--agent", agent.clone());
        }
        if !self.betas.is_empty() {
            push_pair(&mut args, "--betas", self.betas.join(","));
        }
        if let Some(ref format) = self.output_format
            && !format.schema.is_null()
            && let Ok(schema) = serde_json::to_string(&format.schema)
        {
            push_pair(&mut args, "--json-schema", schema);
        }
        if let Some(ref file) = self.debug_file {
            push_pair(&mut args, "--debug-file", file.display().to_string());
        } else if self.debug {
            args.push("--debug".into());
        }
        if has_permission_handler {
            push_pair(&mut args, "--permission-prompt-tool", "stdio".into());
        } else if let Some(ref tool) = self.permission_prompt_tool_name {
            push_pair(&mut args, "--permission-prompt-tool", tool.clone());
        }
        if let Some(prompts) = self.permission_prompts {
            let value = match prompts {
                PermissionPrompts::Host => "host",
                PermissionPrompts::None => "none",
            };
            push_pair(&mut args, "--permission-prompts", value.into());
        }
        if self.continue_session {
            args.push("--continue".into());
        }
        if let Some(ref session_id) = self.resume {
            push_pair(&mut args, "--resume", session_id.clone());
        }
        if let Some(ref tools) = self.allowed_tools
            && !tools.is_empty()
        {
            push_pair(&mut args, "--allowedTools", tools.join(","));
        }
        if let Some(ref tools) = self.disallowed_tools
            && !tools.is_empty()
        {
            push_pair(&mut args, "--disallowedTools", tools.join(","));
        }
        if let Some(ref tools) = self.tools {
            let value = match tools {
                ToolsConfig::Default => "default".to_string(),
                ToolsConfig::Names(names) => names.join(","),
            };
            push_pair(&mut args, "--tools", value);
        }
        if !self.mcp_servers.is_empty()
            && let Ok(json) = serde_json::to_string(&serde_json::json!({
                "mcpServers": self.mcp_servers,
            }))
        {
            push_split(&mut args, "--mcp-config", json);
        }
        if let Some(ref sources) = self.setting_sources {
            let joined: Vec<&str> = sources.iter().map(|s| s.as_cli_str()).collect();
            args.push(format!("--setting-sources={}", joined.join(",")));
        }
        if self.strict_mcp_config {
            args.push("--strict-mcp-config".into());
        }
        if let Some(mode) = self.permission_mode {
            push_pair(&mut args, "--permission-mode", mode.as_str().into());
        }
        if self.allow_dangerously_skip_permissions {
            args.push("--allow-dangerously-skip-permissions".into());
        }
        if let Some(ref model) = self.fallback_model {
            push_pair(&mut args, "--fallback-model", model.clone());
        }
        if self.include_hook_events {
            args.push("--include-hook-events".into());
        }
        if self.include_partial_messages {
            args.push("--include-partial-messages".into());
        }
        if let Some(ref root) = self.project_config_root {
            args.push(format!("--project-config-root={}", root.display()));
        }
        for dir in &self.additional_directories {
            push_pair(&mut args, "--add-dir", dir.display().to_string());
        }
        match self.plugin_delivery {
            PluginDelivery::Initialize => args.push("--await-initialize".into()),
            PluginDelivery::Argv => {
                for plugin in &self.plugins {
                    match plugin {
                        PluginConfig::Local {
                            path,
                            skip_mcp_discovery,
                        } => {
                            let flag = if *skip_mcp_discovery == Some(true) {
                                "--plugin-dir-no-mcp"
                            } else {
                                "--plugin-dir"
                            };
                            push_pair(&mut args, flag, path.clone());
                        }
                    }
                }
            }
        }
        if self.fork_session {
            args.push("--fork-session".into());
        }
        if let Some(ref at) = self.resume_session_at {
            args.push(format!("--resume-session-at={at}"));
        }
        if let Some(ref drops) = self.resume_drops_turn {
            args.push(format!("--resume-drops-turn={drops}"));
        }
        if let Some(ref session_id) = self.session_id {
            args.push(format!("--session-id={session_id}"));
        }
        if !self.persist_session {
            args.push("--no-session-persistence".into());
        }
        if let Some(ref managed) = self.managed_settings {
            push_split(&mut args, "--managed-settings", managed.clone());
        }

        // Flags the TS SDK sends in `initialize` instead; the CLI accepts
        // both, and these are verified against `claude --help`.
        match &self.system_prompt {
            Some(SystemPromptConfig::Custom(prompt)) => {
                push_pair(&mut args, "--system-prompt", prompt.clone());
            }
            Some(SystemPromptConfig::Preset {
                append: Some(append),
            }) => {
                push_pair(&mut args, "--append-system-prompt", append.clone());
            }
            Some(SystemPromptConfig::Preset { append: None }) | None => {}
        }
        if !self.agents.is_empty()
            && let Ok(json) = serde_json::to_string(&self.agents)
        {
            push_pair(&mut args, "--agents", json);
        }

        if let Some(settings) = self.settings_arg() {
            push_split(&mut args, "--settings", settings);
        }

        for (key, value) in &self.extra_args {
            match value {
                None => args.push(format!("--{key}")),
                Some(v) => push_split(&mut args, &format!("--{key}"), v.clone()),
            }
        }

        args
    }

    /// `--settings` value: the settings path or object, with the sandbox
    /// option merged into its `sandbox` block as the TS SDK 0.3.296 does.
    fn settings_arg(&self) -> Option<String> {
        let Some(sandbox) = self.sandbox.as_ref() else {
            return match &self.settings {
                None => None,
                Some(Value::String(path)) => Some(path.clone()),
                Some(settings) => serde_json::to_string(settings).ok(),
            };
        };
        let mut settings = match &self.settings {
            Some(Value::Object(map)) => map.clone(),
            Some(Value::String(text)) if is_inline_json(text) => {
                match serde_json::from_str::<Value>(text) {
                    Ok(Value::Object(map)) => map,
                    _ => serde_json::Map::new(),
                }
            }
            _ => serde_json::Map::new(),
        };
        let option = serde_json::to_value(sandbox).unwrap_or(Value::Null);
        let merged = merge_sandbox(settings.get("sandbox"), &option);
        settings.insert("sandbox".into(), merged);
        serde_json::to_string(&Value::Object(settings)).ok()
    }

    /// Environment the CLI runs with, on top of the inherited environment.
    pub fn env_vars(&self) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = self
            .env
            .clone()
            .map(|e| e.into_iter().collect())
            .unwrap_or_default();
        if self.enable_file_checkpointing {
            env.push((
                "CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING".into(),
                "true".into(),
            ));
        }
        env
    }

    /// The `initialize` request for these options and hook registrations.
    pub fn initialize_request(
        &self,
        hooks: Option<HashMap<HookEvent, Vec<SdkHookCallbackMatcher>>>,
    ) -> InitializeRequest {
        let flag = |on: bool| on.then_some(true);
        InitializeRequest {
            hooks,
            title: self.title.clone(),
            skills: self.skills.clone(),
            plan_mode_instructions: self.plan_mode_instructions.clone(),
            tool_aliases: self.tool_aliases.clone(),
            prompt_suggestions: flag(self.prompt_suggestions),
            agent_progress_summaries: flag(self.agent_progress_summaries),
            forward_subagent_text: flag(self.forward_subagent_text),
            supported_dialog_kinds: self.supported_dialog_kinds.clone(),
            per_task_stop_affordance: flag(self.per_task_stop_affordance),
            plugins: (self.plugin_delivery == PluginDelivery::Initialize)
                .then(|| serde_json::to_value(&self.plugins).ok())
                .flatten(),
            ..InitializeRequest::default()
        }
    }
}

/// `--flag=value`: since 0.3.295 the TS SDK sends a named option's value in
/// the same argument as its flag, so a value that starts with `-` is never
/// read as another flag.
fn push_pair(args: &mut Vec<String>, flag: &str, value: String) {
    args.push(format!("{flag}={value}"));
}

/// `--flag value`, or `--flag=value` when the value starts with `-`. The TS
/// SDK keeps this form for `--mcp-config`, `--managed-settings`,
/// `--settings`, and extra arguments.
fn push_split(args: &mut Vec<String>, flag: &str, value: String) {
    if value.len() > 1 && value.starts_with('-') {
        args.push(format!("{flag}={value}"));
    } else {
        args.push(flag.to_string());
        args.push(value);
    }
}

/// A `settings` string that is inline JSON rather than a file path.
fn is_inline_json(text: &str) -> bool {
    let text = text.trim();
    text.starts_with('{') && text.ends_with('}')
}

/// Restriction lists combined from both sides instead of replaced.
const SANDBOX_UNION_PATHS: &[&str] = &[
    "filesystem.denyRead",
    "filesystem.denyWrite",
    "network.deniedDomains",
    "credentials.files",
    "credentials.envVars",
];

/// Objects the sandbox option replaces whole instead of merging.
const SANDBOX_REPLACE_PATHS: &[&str] = &["ripgrep", "network.tlsTerminate"];

/// Proxy ports dropped when the option sets a domain restriction.
const SANDBOX_PROXY_PORTS: &[&str] = &["httpProxyPort", "socksProxyPort"];

fn merge_sandbox_at(base: Option<&Value>, option: &Value, prefix: &str) -> Value {
    let mut merged = match base {
        Some(Value::Object(map)) => map.clone(),
        _ => serde_json::Map::new(),
    };
    let Value::Object(option) = option else {
        return Value::Object(merged);
    };
    for (key, value) in option {
        if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
            continue;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let next = match (merged.get(key), value) {
            (Some(Value::Array(old)), Value::Array(new))
                if SANDBOX_UNION_PATHS.contains(&path.as_str()) =>
            {
                let mut combined = old.clone();
                for item in new {
                    if !combined.contains(item) {
                        combined.push(item.clone());
                    }
                }
                Value::Array(combined)
            }
            (Some(old @ Value::Object(_)), Value::Object(_))
                if !SANDBOX_REPLACE_PATHS.contains(&path.as_str()) =>
            {
                merge_sandbox_at(Some(old), value, &path)
            }
            _ => value.clone(),
        };
        merged.insert(key.clone(), next);
    }
    Value::Object(merged)
}

/// Merge the sandbox option into a settings `sandbox` block (TS SDK
/// 0.3.296): the option's values win, restriction lists combine,
/// `filesystem.disabled: true` is dropped when the option restricts the
/// filesystem, proxy ports are dropped when the option restricts domains,
/// and an enabled sandbox fails closed unless either side says otherwise.
fn merge_sandbox(base: Option<&Value>, option: &Value) -> Value {
    let mut merged = merge_sandbox_at(base, option, "");
    let restricts_files = option.get("filesystem").is_some_and(|v| !v.is_null())
        || option
            .pointer("/credentials/files")
            .and_then(Value::as_array)
            .is_some_and(|files| files.iter().any(|f| f["mode"] == "deny"));
    if restricts_files
        && option.pointer("/filesystem/disabled").is_none()
        && let Some(filesystem) = merged.get_mut("filesystem").and_then(Value::as_object_mut)
        && filesystem.get("disabled") == Some(&Value::Bool(true))
    {
        filesystem.remove("disabled");
    }
    let network = option.get("network");
    let restricts_domains = network.is_some_and(|n| {
        n.get("allowedDomains").is_some_and(|v| !v.is_null())
            || n["deniedDomains"].as_array().is_some_and(|d| !d.is_empty())
            || n["strictAllowlist"] == Value::Bool(true)
    });
    if restricts_domains
        && let Some(merged_network) = merged.get_mut("network").and_then(Value::as_object_mut)
    {
        for port in SANDBOX_PROXY_PORTS {
            let set_by_option = network
                .and_then(|n| n.get(*port))
                .is_some_and(|v| !v.is_null());
            if !set_by_option {
                merged_network.remove(*port);
            }
        }
    }
    if option["enabled"] == Value::Bool(true)
        && let Some(map) = merged.as_object_mut()
    {
        map.entry("failIfUnavailable").or_insert(Value::Bool(true));
    }
    merged
}
