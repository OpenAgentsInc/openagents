//! Agent selection configuration types
//!
//! Defines types for agent selection and settings persistence.

use serde::{Deserialize, Serialize};

use crate::app::agents::AgentKind;

/// Agent selection state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSelection {
    /// Selected agent backend
    pub agent: AgentKindConfig,
    /// Selected model ID (None uses agent default)
    pub model_id: Option<String>,
}

impl Default for AgentSelection {
    fn default() -> Self {
        Self {
            agent: AgentKindConfig::Claude,
            model_id: None,
        }
    }
}

impl AgentSelection {
    /// Create a new agent selection
    pub fn new(agent: AgentKindConfig) -> Self {
        Self {
            agent,
            model_id: None,
        }
    }

    /// Create with a specific model
    pub fn with_model(agent: AgentKindConfig, model_id: impl Into<String>) -> Self {
        Self {
            agent,
            model_id: Some(model_id.into()),
        }
    }

    /// Get the agent kind
    pub fn kind(&self) -> AgentKind {
        self.agent.into()
    }

    /// Get display name for the current selection
    pub fn display_name(&self) -> String {
        let agent_name = match self.agent {
            AgentKindConfig::Claude => "Claude",
            AgentKindConfig::Codex => "Codex",
        };
        if let Some(model) = &self.model_id {
            format!("{} ({})", agent_name, shorten_model_id(model))
        } else {
            agent_name.to_string()
        }
    }
}

/// Serializable agent kind (for settings persistence)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKindConfig {
    Claude,
    Codex,
}

impl Default for AgentKindConfig {
    fn default() -> Self {
        AgentKindConfig::Claude
    }
}

impl From<AgentKindConfig> for AgentKind {
    fn from(config: AgentKindConfig) -> Self {
        match config {
            AgentKindConfig::Claude => AgentKind::Claude,
            AgentKindConfig::Codex => AgentKind::Codex,
        }
    }
}

impl From<AgentKind> for AgentKindConfig {
    fn from(kind: AgentKind) -> Self {
        match kind {
            AgentKind::Claude => AgentKindConfig::Claude,
            AgentKind::Codex => AgentKindConfig::Codex,
        }
    }
}

/// Per-agent settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Default model for this agent
    pub default_model: Option<String>,
    /// Default permission mode
    pub default_mode: Option<String>,
    /// Favorite models
    pub favorite_models: Vec<String>,
    /// Extra configuration options
    pub config_options: std::collections::HashMap<String, String>,
}

/// All agent settings (for persistence)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AllAgentSettings {
    /// Currently selected agent
    pub selected: AgentSelection,
    /// Claude-specific settings
    pub claude: AgentSettings,
    /// Codex-specific settings
    pub codex: AgentSettings,
}

/// Shorten a model ID for display
fn shorten_model_id(model_id: &str) -> String {
    // "claude-opus-4-5-20251101" -> "Opus 4.5"
    // "gpt-4o" -> "GPT-4o"
    if model_id.contains("opus") {
        "Opus".to_string()
    } else if model_id.contains("sonnet") {
        "Sonnet".to_string()
    } else if model_id.contains("haiku") {
        "Haiku".to_string()
    } else if model_id.starts_with("gpt-4o") {
        if model_id.contains("mini") {
            "4o-mini".to_string()
        } else {
            "4o".to_string()
        }
    } else if model_id.starts_with("o1") || model_id.starts_with("o3") {
        model_id.to_uppercase()
    } else {
        // Return as-is but truncated
        if model_id.len() > 15 {
            format!("{}...", &model_id[..12])
        } else {
            model_id.to_string()
        }
    }
}

/// Migrate from legacy ModelOption to AgentSelection
pub fn migrate_from_model_option(model_id: &str) -> AgentSelection {
    // All legacy models are Claude models
    AgentSelection {
        agent: AgentKindConfig::Claude,
        model_id: Some(map_legacy_model_id(model_id)),
    }
}

/// Map legacy model IDs to current format
fn map_legacy_model_id(legacy: &str) -> String {
    match legacy {
        "opus" | "Opus" => "claude-opus-4-5-20251101".to_string(),
        "sonnet" | "Sonnet" => "claude-sonnet-4-5-20250929".to_string(),
        "haiku" | "Haiku" => "claude-haiku-4-5-20251001".to_string(),
        // Already a full model ID
        id if id.starts_with("claude-") => id.to_string(),
        // Unknown, return as-is
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_selection() {
        let selection = AgentSelection::default();
        assert_eq!(selection.agent, AgentKindConfig::Claude);
        assert!(selection.model_id.is_none());
    }

    #[test]
    fn test_shorten_model_id() {
        assert_eq!(shorten_model_id("claude-opus-4-5-20251101"), "Opus");
        assert_eq!(shorten_model_id("claude-sonnet-4-5-20250929"), "Sonnet");
        assert_eq!(shorten_model_id("gpt-4o"), "4o");
        assert_eq!(shorten_model_id("gpt-4o-mini"), "4o-mini");
    }

    #[test]
    fn test_migrate_from_model_option() {
        let selection = migrate_from_model_option("opus");
        assert_eq!(selection.agent, AgentKindConfig::Claude);
        assert_eq!(selection.model_id, Some("claude-opus-4-5-20251101".to_string()));
    }
}
