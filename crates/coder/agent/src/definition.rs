//! Agent definition types.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Agent mode - determines when this agent can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Can only be used as a subagent (spawned by primary agents).
    Subagent,
    /// Primary agent that can be selected by the user.
    #[default]
    Primary,
    /// Can be used in any context.
    All,
}

/// Agent definition - describes an agent's capabilities and configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Unique identifier for this agent.
    pub name: String,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When this agent can be used.
    #[serde(default)]
    pub mode: AgentMode,
    /// Whether this is a built-in agent.
    #[serde(default)]
    pub built_in: bool,
    /// Model configuration override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<AgentModelConfig>,
    /// Custom system prompt (appended to base prompt).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Tool availability (tool_name -> enabled).
    #[serde(default)]
    pub tools: IndexMap<String, bool>,
    /// Permission configuration for this agent.
    #[serde(default)]
    pub permission: AgentPermission,
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p sampling parameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maximum steps/turns for this agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<u32>,
    /// UI color for this agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Additional provider-specific options.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub options: IndexMap<String, serde_json::Value>,
}

impl AgentDefinition {
    /// Create a new agent definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            mode: AgentMode::Primary,
            built_in: false,
            model: None,
            prompt: None,
            tools: IndexMap::new(),
            permission: AgentPermission::default(),
            temperature: None,
            top_p: None,
            max_steps: None,
            color: None,
            options: IndexMap::new(),
        }
    }

    /// Builder method to set description.
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builder method to set mode.
    pub fn mode(mut self, mode: AgentMode) -> Self {
        self.mode = mode;
        self
    }

    /// Builder method to mark as built-in.
    pub fn built_in(mut self) -> Self {
        self.built_in = true;
        self
    }

    /// Builder method to set model.
    pub fn model(mut self, provider: impl Into<String>, model: impl Into<String>) -> Self {
        self.model = Some(AgentModelConfig {
            provider_id: provider.into(),
            model_id: model.into(),
        });
        self
    }

    /// Builder method to set custom prompt.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Builder method to enable a tool.
    pub fn enable_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.insert(tool.into(), true);
        self
    }

    /// Builder method to disable a tool.
    pub fn disable_tool(mut self, tool: impl Into<String>) -> Self {
        self.tools.insert(tool.into(), false);
        self
    }

    /// Builder method to set permission.
    pub fn permission(mut self, permission: AgentPermission) -> Self {
        self.permission = permission;
        self
    }

    /// Builder method to set temperature.
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Builder method to set max steps.
    pub fn max_steps(mut self, steps: u32) -> Self {
        self.max_steps = Some(steps);
        self
    }

    /// Builder method to set color.
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Check if a tool is enabled for this agent.
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        self.tools.get(tool_name).copied().unwrap_or(true)
    }
}

/// Model configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelConfig {
    /// Provider ID (e.g., "anthropic", "openai").
    pub provider_id: String,
    /// Model ID (e.g., "claude-sonnet-4-20250514").
    pub model_id: String,
}

/// Permission configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermission {
    /// Permission for file editing.
    #[serde(default = "default_permission")]
    pub edit: Permission,
    /// Permission for bash commands (pattern -> permission).
    #[serde(default)]
    pub bash: IndexMap<String, Permission>,
    /// Permission for web fetching.
    #[serde(default = "default_permission")]
    pub webfetch: Permission,
    /// Permission for doom loop (many consecutive tool calls).
    #[serde(default = "default_ask_permission")]
    pub doom_loop: Permission,
    /// Permission for accessing directories outside the project.
    #[serde(default = "default_ask_permission")]
    pub external_directory: Permission,
}

impl Default for AgentPermission {
    fn default() -> Self {
        let mut bash = IndexMap::new();
        bash.insert("*".to_string(), Permission::Allow);

        Self {
            edit: Permission::Allow,
            bash,
            webfetch: Permission::Allow,
            doom_loop: Permission::Ask,
            external_directory: Permission::Ask,
        }
    }
}

impl AgentPermission {
    /// Create permissive permissions (allow everything).
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Create restrictive permissions (read-only).
    pub fn read_only() -> Self {
        let mut bash = IndexMap::new();
        // Read-only commands allowed
        bash.insert("ls*".to_string(), Permission::Allow);
        bash.insert("cat*".to_string(), Permission::Allow);
        bash.insert("head*".to_string(), Permission::Allow);
        bash.insert("tail*".to_string(), Permission::Allow);
        bash.insert("grep*".to_string(), Permission::Allow);
        bash.insert("rg*".to_string(), Permission::Allow);
        bash.insert("find*".to_string(), Permission::Allow);
        bash.insert("tree*".to_string(), Permission::Allow);
        bash.insert("git diff*".to_string(), Permission::Allow);
        bash.insert("git log*".to_string(), Permission::Allow);
        bash.insert("git show*".to_string(), Permission::Allow);
        bash.insert("git status*".to_string(), Permission::Allow);
        bash.insert("git branch".to_string(), Permission::Allow);
        bash.insert("*".to_string(), Permission::Deny);

        Self {
            edit: Permission::Deny,
            bash,
            webfetch: Permission::Allow,
            doom_loop: Permission::Ask,
            external_directory: Permission::Ask,
        }
    }

    /// Create plan-mode permissions (read + limited commands).
    pub fn plan_mode() -> Self {
        let mut bash = IndexMap::new();
        // Allow read-only operations
        bash.insert("cut*".to_string(), Permission::Allow);
        bash.insert("diff*".to_string(), Permission::Allow);
        bash.insert("du*".to_string(), Permission::Allow);
        bash.insert("file *".to_string(), Permission::Allow);
        bash.insert("find * -delete*".to_string(), Permission::Ask);
        bash.insert("find * -exec*".to_string(), Permission::Ask);
        bash.insert("find *".to_string(), Permission::Allow);
        bash.insert("git diff*".to_string(), Permission::Allow);
        bash.insert("git log*".to_string(), Permission::Allow);
        bash.insert("git show*".to_string(), Permission::Allow);
        bash.insert("git status*".to_string(), Permission::Allow);
        bash.insert("git branch".to_string(), Permission::Allow);
        bash.insert("git branch -v".to_string(), Permission::Allow);
        bash.insert("grep*".to_string(), Permission::Allow);
        bash.insert("head*".to_string(), Permission::Allow);
        bash.insert("less*".to_string(), Permission::Allow);
        bash.insert("ls*".to_string(), Permission::Allow);
        bash.insert("more*".to_string(), Permission::Allow);
        bash.insert("pwd*".to_string(), Permission::Allow);
        bash.insert("rg*".to_string(), Permission::Allow);
        bash.insert("sort*".to_string(), Permission::Allow);
        bash.insert("stat*".to_string(), Permission::Allow);
        bash.insert("tail*".to_string(), Permission::Allow);
        bash.insert("tree*".to_string(), Permission::Allow);
        bash.insert("uniq*".to_string(), Permission::Allow);
        bash.insert("wc*".to_string(), Permission::Allow);
        bash.insert("whereis*".to_string(), Permission::Allow);
        bash.insert("which*".to_string(), Permission::Allow);
        bash.insert("*".to_string(), Permission::Ask);

        Self {
            edit: Permission::Deny,
            bash,
            webfetch: Permission::Allow,
            doom_loop: Permission::Ask,
            external_directory: Permission::Ask,
        }
    }

    /// Check permission for a bash command.
    pub fn check_bash(&self, command: &str) -> Permission {
        // Check patterns in order
        for (pattern, permission) in &self.bash {
            if pattern == "*" {
                continue; // Check wildcard last
            }
            if glob_match(pattern, command) {
                return *permission;
            }
        }
        // Fall back to wildcard or allow
        self.bash.get("*").copied().unwrap_or(Permission::Allow)
    }
}

/// Permission level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Always allow without asking.
    #[default]
    Allow,
    /// Always ask for permission.
    Ask,
    /// Always deny.
    Deny,
}

fn default_permission() -> Permission {
    Permission::Allow
}

fn default_ask_permission() -> Permission {
    Permission::Ask
}

/// Simple glob matching for permission patterns.
fn glob_match(pattern: &str, input: &str) -> bool {
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        input.starts_with(prefix)
    } else {
        pattern == input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_definition_builder() {
        let agent = AgentDefinition::new("test")
            .description("A test agent")
            .mode(AgentMode::Subagent)
            .built_in()
            .enable_tool("read")
            .disable_tool("write");

        assert_eq!(agent.name, "test");
        assert_eq!(agent.description, Some("A test agent".to_string()));
        assert_eq!(agent.mode, AgentMode::Subagent);
        assert!(agent.built_in);
        assert!(agent.is_tool_enabled("read"));
        assert!(!agent.is_tool_enabled("write"));
        assert!(agent.is_tool_enabled("unknown")); // Default is enabled
    }

    #[test]
    fn test_permission_check_bash() {
        let permission = AgentPermission::plan_mode();

        assert_eq!(permission.check_bash("ls -la"), Permission::Allow);
        assert_eq!(permission.check_bash("git status"), Permission::Allow);
        assert_eq!(permission.check_bash("rm -rf /"), Permission::Ask);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("ls*", "ls"));
        assert!(glob_match("ls*", "ls -la"));
        assert!(glob_match("git diff*", "git diff HEAD"));
        assert!(!glob_match("ls*", "cat"));
        assert!(glob_match("*", "anything"));
    }
}
