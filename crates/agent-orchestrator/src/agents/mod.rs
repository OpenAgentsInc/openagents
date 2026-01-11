use crate::config::{AgentConfig, AgentMode, AgentPermission, BashPermission, PermissionLevel};
use std::collections::HashMap;

pub const SISYPHUS_PROMPT: &str = include_str!("sisyphus_prompt.md");
pub const ORACLE_PROMPT: &str = include_str!("oracle_prompt.md");
pub const LIBRARIAN_PROMPT: &str = include_str!("librarian_prompt.md");
pub const EXPLORE_PROMPT: &str = include_str!("explore_prompt.md");
pub const FRONTEND_PROMPT: &str = include_str!("frontend_prompt.md");
pub const DOCWRITER_PROMPT: &str = include_str!("docwriter_prompt.md");
pub const MULTIMODAL_PROMPT: &str = include_str!("multimodal_prompt.md");

pub fn builtin_agents() -> HashMap<String, AgentConfig> {
    let mut agents = HashMap::new();

    agents.insert("Sisyphus".to_string(), sisyphus_config());
    agents.insert("Oracle".to_string(), oracle_config());
    agents.insert("Librarian".to_string(), librarian_config());
    agents.insert("Explore".to_string(), explore_config());
    agents.insert("Frontend".to_string(), frontend_config());
    agents.insert("DocWriter".to_string(), docwriter_config());
    agents.insert("Multimodal".to_string(), multimodal_config());

    agents
}

pub fn sisyphus_config() -> AgentConfig {
    AgentConfig {
        model: "openai/codex-opus-4-5".to_string(),
        prompt: SISYPHUS_PROMPT.to_string(),
        temperature: 0.1,
        description: "Primary orchestrator - delegates to specialists, manages workflow"
            .to_string(),
        mode: AgentMode::Primary,
        color: "#FF6B6B".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Allow,
            bash: BashPermission {
                mode: PermissionLevel::Allow,
                allowed_commands: vec![
                    "cargo".to_string(),
                    "git".to_string(),
                    "npm".to_string(),
                    "rustc".to_string(),
                ],
                denied_commands: vec!["sudo".to_string()],
            },
            webfetch: PermissionLevel::Allow,
        },
    }
}

pub fn oracle_config() -> AgentConfig {
    AgentConfig {
        model: "openai/gpt-5.2".to_string(),
        prompt: ORACLE_PROMPT.to_string(),
        temperature: 0.3,
        description: "Architecture advisor - consult for complex decisions".to_string(),
        mode: AgentMode::Subagent,
        color: "#9B59B6".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Deny,
            bash: BashPermission {
                mode: PermissionLevel::Deny,
                ..Default::default()
            },
            webfetch: PermissionLevel::Allow,
        },
    }
}

pub fn librarian_config() -> AgentConfig {
    AgentConfig {
        model: "openai/codex-sonnet-4".to_string(),
        prompt: LIBRARIAN_PROMPT.to_string(),
        temperature: 0.2,
        description: "External docs expert - GitHub search, OSS reference".to_string(),
        mode: AgentMode::Subagent,
        color: "#3498DB".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Deny,
            bash: BashPermission {
                mode: PermissionLevel::Deny,
                ..Default::default()
            },
            webfetch: PermissionLevel::Allow,
        },
    }
}

pub fn explore_config() -> AgentConfig {
    AgentConfig {
        model: "xai/grok-3".to_string(),
        prompt: EXPLORE_PROMPT.to_string(),
        temperature: 0.1,
        description: "Fast codebase explorer - pattern search, navigation".to_string(),
        mode: AgentMode::Subagent,
        color: "#2ECC71".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Deny,
            bash: BashPermission {
                mode: PermissionLevel::Ask,
                allowed_commands: vec!["git".to_string()],
                denied_commands: vec![],
            },
            webfetch: PermissionLevel::Deny,
        },
    }
}

pub fn frontend_config() -> AgentConfig {
    AgentConfig {
        model: "google/gemini-2.5-pro".to_string(),
        prompt: FRONTEND_PROMPT.to_string(),
        temperature: 0.5,
        description: "UI/UX specialist - visual design, styling, layout".to_string(),
        mode: AgentMode::Subagent,
        color: "#E74C3C".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Allow,
            bash: BashPermission {
                mode: PermissionLevel::Ask,
                allowed_commands: vec!["npm".to_string(), "yarn".to_string()],
                denied_commands: vec![],
            },
            webfetch: PermissionLevel::Allow,
        },
    }
}

pub fn docwriter_config() -> AgentConfig {
    AgentConfig {
        model: "google/gemini-2.5-pro".to_string(),
        prompt: DOCWRITER_PROMPT.to_string(),
        temperature: 0.4,
        description: "Technical writer - README, API docs, guides".to_string(),
        mode: AgentMode::Subagent,
        color: "#F39C12".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Allow,
            bash: BashPermission {
                mode: PermissionLevel::Deny,
                ..Default::default()
            },
            webfetch: PermissionLevel::Allow,
        },
    }
}

pub fn multimodal_config() -> AgentConfig {
    AgentConfig {
        model: "google/gemini-2.5-flash".to_string(),
        prompt: MULTIMODAL_PROMPT.to_string(),
        temperature: 0.3,
        description: "Media analyzer - PDFs, images, diagrams".to_string(),
        mode: AgentMode::Subagent,
        color: "#1ABC9C".to_string(),
        tools: HashMap::new(),
        permission: AgentPermission {
            edit: PermissionLevel::Deny,
            bash: BashPermission {
                mode: PermissionLevel::Deny,
                ..Default::default()
            },
            webfetch: PermissionLevel::Deny,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_agents_has_all_seven() {
        let agents = builtin_agents();
        assert_eq!(agents.len(), 7);
        assert!(agents.contains_key("Sisyphus"));
        assert!(agents.contains_key("Oracle"));
        assert!(agents.contains_key("Librarian"));
        assert!(agents.contains_key("Explore"));
        assert!(agents.contains_key("Frontend"));
        assert!(agents.contains_key("DocWriter"));
        assert!(agents.contains_key("Multimodal"));
    }

    #[test]
    fn sisyphus_is_primary() {
        let config = sisyphus_config();
        assert_eq!(config.mode, AgentMode::Primary);
    }

    #[test]
    fn oracle_is_readonly() {
        let config = oracle_config();
        assert_eq!(config.permission.edit, PermissionLevel::Deny);
        assert_eq!(config.permission.bash.mode, PermissionLevel::Deny);
    }

    #[test]
    fn explore_has_low_temperature() {
        let config = explore_config();
        assert!(config.temperature <= 0.2);
    }

    #[test]
    fn frontend_can_edit() {
        let config = frontend_config();
        assert_eq!(config.permission.edit, PermissionLevel::Allow);
    }
}
