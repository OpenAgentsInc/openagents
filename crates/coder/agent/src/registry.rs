//! Agent registry with built-in agents.

use crate::{AgentDefinition, AgentMode, AgentPermission};
use indexmap::IndexMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur with the agent registry.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    NotFound(String),

    #[error("Agent already exists: {0}")]
    AlreadyExists(String),

    #[error("Cannot modify built-in agent: {0}")]
    BuiltInModification(String),
}

/// Registry of available agents.
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: IndexMap<String, Arc<AgentDefinition>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::with_builtin_agents()
    }
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            agents: IndexMap::new(),
        }
    }

    /// Create a registry with built-in agents.
    pub fn with_builtin_agents() -> Self {
        let mut registry = Self::new();

        // General purpose agent
        registry.register(builtin_general());

        // Explore agent (file search specialist)
        registry.register(builtin_explore());

        // Plan agent (read-only planning)
        registry.register(builtin_plan());

        // Build agent (full capabilities)
        registry.register(builtin_build());

        registry
    }

    /// Register an agent.
    pub fn register(&mut self, agent: AgentDefinition) {
        let name = agent.name.clone();
        self.agents.insert(name, Arc::new(agent));
    }

    /// Get an agent by name.
    pub fn get(&self, name: &str) -> Option<Arc<AgentDefinition>> {
        self.agents.get(name).cloned()
    }

    /// Get an agent or return error.
    pub fn get_or_err(&self, name: &str) -> Result<Arc<AgentDefinition>, AgentError> {
        self.get(name)
            .ok_or_else(|| AgentError::NotFound(name.to_string()))
    }

    /// List all agents.
    pub fn list(&self) -> Vec<Arc<AgentDefinition>> {
        self.agents.values().cloned().collect()
    }

    /// List agents by mode.
    pub fn list_by_mode(&self, mode: AgentMode) -> Vec<Arc<AgentDefinition>> {
        self.agents
            .values()
            .filter(|a| a.mode == mode || a.mode == AgentMode::All)
            .cloned()
            .collect()
    }

    /// List primary agents (user-selectable).
    pub fn list_primary(&self) -> Vec<Arc<AgentDefinition>> {
        self.list_by_mode(AgentMode::Primary)
    }

    /// List subagents (spawnable by other agents).
    pub fn list_subagents(&self) -> Vec<Arc<AgentDefinition>> {
        self.agents
            .values()
            .filter(|a| a.mode == AgentMode::Subagent || a.mode == AgentMode::All)
            .cloned()
            .collect()
    }

    /// Remove a custom agent.
    pub fn remove(&mut self, name: &str) -> Result<(), AgentError> {
        if let Some(agent) = self.agents.get(name) {
            if agent.built_in {
                return Err(AgentError::BuiltInModification(name.to_string()));
            }
        }
        self.agents.shift_remove(name);
        Ok(())
    }
}

// ============================================================================
// Built-in agents
// ============================================================================

/// General purpose agent for complex tasks.
pub fn builtin_general() -> AgentDefinition {
    AgentDefinition::new("general")
        .description(
            "General-purpose agent for researching complex questions and executing multi-step tasks. \
             Use this agent to execute multiple units of work in parallel."
        )
        .mode(AgentMode::Subagent)
        .built_in()
        .disable_tool("todoread")
        .disable_tool("todowrite")
        .permission(AgentPermission::permissive())
}

/// Explore agent for file/code search.
pub fn builtin_explore() -> AgentDefinition {
    AgentDefinition::new("explore")
        .description(
            "Fast agent specialized for exploring codebases. Use this when you need to quickly find \
             files by patterns (eg. \"src/components/**/*.tsx\"), search code for keywords \
             (eg. \"API endpoints\"), or answer questions about the codebase (eg. \"how do API endpoints work?\"). \
             When calling this agent, specify the desired thoroughness level: \"quick\" for basic searches, \
             \"medium\" for moderate exploration, or \"very thorough\" for comprehensive analysis \
             across multiple locations and naming conventions."
        )
        .mode(AgentMode::Subagent)
        .built_in()
        .disable_tool("todoread")
        .disable_tool("todowrite")
        .disable_tool("edit")
        .disable_tool("write")
        .prompt(
            "You are a file search specialist. You excel at thoroughly navigating and exploring codebases.\n\n\
             Your strengths:\n\
             - Rapidly finding files using glob patterns\n\
             - Searching code and text with powerful regex patterns\n\
             - Reading and analyzing file contents\n\n\
             Guidelines:\n\
             - Use Glob for broad file pattern matching\n\
             - Use Grep for searching file contents with regex\n\
             - Use Read when you know the specific file path you need to read\n\
             - Use Bash for file operations like copying, moving, or listing directory contents\n\
             - Adapt your search approach based on the thoroughness level specified by the caller\n\
             - Return file paths as absolute paths in your final response\n\
             - For clear communication, avoid using emojis\n\
             - Do not create any files, or run bash commands that modify the user's system state in any way\n\n\
             Complete the user's search request efficiently and report your findings clearly."
        )
        .permission(AgentPermission::read_only())
}

/// Plan agent for designing implementation approaches.
pub fn builtin_plan() -> AgentDefinition {
    AgentDefinition::new("plan")
        .description(
            "Software architect agent for designing implementation plans. Use this when you need to \
             plan the implementation strategy for a task. Returns step-by-step plans, identifies \
             critical files, and considers architectural trade-offs."
        )
        .mode(AgentMode::Primary)
        .built_in()
        .permission(AgentPermission::plan_mode())
}

/// Build agent with full capabilities.
pub fn builtin_build() -> AgentDefinition {
    AgentDefinition::new("build")
        .description(
            "Full-capability agent for implementing features and fixing bugs. Has access to all tools \
             and can modify files, run tests, and execute commands."
        )
        .mode(AgentMode::Primary)
        .built_in()
        .permission(AgentPermission::permissive())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_with_builtins() {
        let registry = AgentRegistry::with_builtin_agents();

        assert!(registry.get("general").is_some());
        assert!(registry.get("explore").is_some());
        assert!(registry.get("plan").is_some());
        assert!(registry.get("build").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_by_mode() {
        let registry = AgentRegistry::with_builtin_agents();

        let subagents = registry.list_subagents();
        assert!(subagents.iter().any(|a| a.name == "general"));
        assert!(subagents.iter().any(|a| a.name == "explore"));

        let primary = registry.list_primary();
        assert!(primary.iter().any(|a| a.name == "plan"));
        assert!(primary.iter().any(|a| a.name == "build"));
    }

    #[test]
    fn test_cannot_remove_builtin() {
        let mut registry = AgentRegistry::with_builtin_agents();

        assert!(matches!(
            registry.remove("general"),
            Err(AgentError::BuiltInModification(_))
        ));
    }

    #[test]
    fn test_custom_agent() {
        let mut registry = AgentRegistry::with_builtin_agents();

        let custom = AgentDefinition::new("custom")
            .description("A custom agent")
            .mode(AgentMode::Primary);

        registry.register(custom);
        assert!(registry.get("custom").is_some());

        // Can remove custom agents
        assert!(registry.remove("custom").is_ok());
        assert!(registry.get("custom").is_none());
    }
}
