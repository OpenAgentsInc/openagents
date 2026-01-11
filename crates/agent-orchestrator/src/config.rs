use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    pub description: String,
    #[serde(default)]
    pub mode: AgentMode,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub tools: HashMap<String, bool>,
    #[serde(default)]
    pub permission: AgentPermission,
}

fn default_temperature() -> f32 {
    0.7
}

fn default_color() -> String {
    "#808080".to_string()
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "openai/codex-sonnet-4".to_string(),
            prompt: String::new(),
            temperature: default_temperature(),
            description: String::new(),
            mode: AgentMode::default(),
            color: default_color(),
            tools: HashMap::new(),
            permission: AgentPermission::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    #[default]
    Primary,
    Subagent,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPermission {
    #[serde(default)]
    pub edit: PermissionLevel,
    #[serde(default)]
    pub bash: BashPermission,
    #[serde(default)]
    pub webfetch: PermissionLevel,
}

impl Default for AgentPermission {
    fn default() -> Self {
        Self {
            edit: PermissionLevel::Ask,
            bash: BashPermission::default(),
            webfetch: PermissionLevel::Allow,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Allow,
    #[default]
    Ask,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashPermission {
    #[serde(default)]
    pub mode: PermissionLevel,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub denied_commands: Vec<String>,
}

impl Default for BashPermission {
    fn default() -> Self {
        Self {
            mode: PermissionLevel::Ask,
            allowed_commands: vec![
                "cargo".to_string(),
                "git".to_string(),
                "npm".to_string(),
                "node".to_string(),
                "rustc".to_string(),
            ],
            denied_commands: vec!["rm -rf /".to_string(), "sudo".to_string()],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<AgentPermission>,
}

impl AgentConfig {
    pub fn apply_override(&mut self, override_config: &AgentOverride) {
        if let Some(model) = &override_config.model {
            self.model = model.clone();
        }
        if let Some(temp) = override_config.temperature {
            self.temperature = temp;
        }
        if let Some(tools) = &override_config.tools {
            for (tool, enabled) in tools {
                self.tools.insert(tool.clone(), *enabled);
            }
        }
        if let Some(permission) = &override_config.permission {
            self.permission = permission.clone();
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    #[serde(default)]
    pub disabled_agents: Vec<String>,
    #[serde(default)]
    pub disabled_hooks: Vec<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentOverride>,
    #[serde(default)]
    pub integrations: IntegrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    #[serde(default = "default_true")]
    pub directives: bool,
    #[serde(default = "default_true")]
    pub autopilot: bool,
    #[serde(default)]
    pub marketplace: bool,
    #[serde(default = "default_true")]
    pub trajectory: bool,
}

fn default_true() -> bool {
    true
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            directives: true,
            autopilot: true,
            marketplace: false,
            trajectory: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_agent_config() {
        let config = AgentConfig::default();
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.mode, AgentMode::Primary);
    }

    #[test]
    fn agent_config_serialize() {
        let config = AgentConfig {
            model: "openai/codex-opus-4".to_string(),
            prompt: "You are a helpful assistant".to_string(),
            description: "Test agent".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("codex-opus-4"));
    }

    #[test]
    fn agent_config_deserialize() {
        let json = r#"{
            "model": "openai/gpt-5",
            "prompt": "Test prompt",
            "description": "Test",
            "temperature": 0.5,
            "mode": "subagent"
        }"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, "openai/gpt-5");
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.mode, AgentMode::Subagent);
    }

    #[test]
    fn apply_override() {
        let mut config = AgentConfig::default();
        let override_config = AgentOverride {
            model: Some("openai/gpt-4".to_string()),
            temperature: Some(0.3),
            ..Default::default()
        };
        config.apply_override(&override_config);
        assert_eq!(config.model, "openai/gpt-4");
        assert_eq!(config.temperature, 0.3);
    }

    #[test]
    fn permission_level_serialize() {
        assert_eq!(
            serde_json::to_string(&PermissionLevel::Allow).unwrap(),
            r#""allow""#
        );
        assert_eq!(
            serde_json::to_string(&PermissionLevel::Ask).unwrap(),
            r#""ask""#
        );
        assert_eq!(
            serde_json::to_string(&PermissionLevel::Deny).unwrap(),
            r#""deny""#
        );
    }
}
