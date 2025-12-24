use crate::agents::builtin_agents;
use crate::config::{AgentConfig, AgentMode, AgentOverride, OrchestratorConfig};
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct AgentRegistry {
    agents: HashMap<String, AgentConfig>,
    disabled: HashSet<String>,
    overrides: HashMap<String, AgentOverride>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: builtin_agents(),
            disabled: HashSet::new(),
            overrides: HashMap::new(),
        }
    }

    pub fn load_config(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let config: OrchestratorConfig = serde_json::from_str(&content)?;

        for agent_name in config.disabled_agents {
            self.disabled.insert(agent_name);
        }

        for (name, override_config) in config.agents {
            self.overrides.insert(name, override_config);
        }

        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<AgentConfig> {
        if self.disabled.contains(name) {
            return Err(Error::AgentDisabled {
                name: name.to_string(),
            });
        }

        let mut config = self
            .agents
            .get(name)
            .cloned()
            .ok_or_else(|| Error::AgentNotFound {
                name: name.to_string(),
            })?;

        if let Some(override_config) = self.overrides.get(name) {
            config.apply_override(override_config);
        }

        Ok(config)
    }

    pub fn list(&self) -> Vec<&str> {
        self.agents
            .keys()
            .filter(|name| !self.disabled.contains(*name))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn list_all(&self) -> Vec<&str> {
        self.agents.keys().map(|s| s.as_str()).collect()
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.agents.contains_key(name) && !self.disabled.contains(name)
    }

    pub fn primary(&self) -> Result<AgentConfig> {
        for (name, config) in &self.agents {
            if config.mode == AgentMode::Primary && !self.disabled.contains(name) {
                return self.get(name);
            }
        }
        Err(Error::AgentNotFound {
            name: "primary".to_string(),
        })
    }

    pub fn subagents(&self) -> Vec<&str> {
        self.agents
            .iter()
            .filter(|(name, config)| {
                (config.mode == AgentMode::Subagent || config.mode == AgentMode::All)
                    && !self.disabled.contains(*name)
            })
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn disable(&mut self, name: &str) {
        self.disabled.insert(name.to_string());
    }

    pub fn enable(&mut self, name: &str) {
        self.disabled.remove(name);
    }

    pub fn override_agent(&mut self, name: &str, config: AgentOverride) {
        self.overrides.insert(name.to_string(), config);
    }

    pub fn register(&mut self, name: &str, config: AgentConfig) {
        self.agents.insert(name.to_string(), config);
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_builtin_agents() {
        let registry = AgentRegistry::new();
        assert!(registry.is_enabled("Sisyphus"));
        assert!(registry.is_enabled("Oracle"));
        assert!(registry.is_enabled("Librarian"));
        assert!(registry.is_enabled("Explore"));
    }

    #[test]
    fn get_primary_agent() {
        let registry = AgentRegistry::new();
        let primary = registry.primary().unwrap();
        assert!(primary.model.contains("claude"));
    }

    #[test]
    fn disable_agent() {
        let mut registry = AgentRegistry::new();
        assert!(registry.is_enabled("Oracle"));
        registry.disable("Oracle");
        assert!(!registry.is_enabled("Oracle"));
        assert!(registry.get("Oracle").is_err());
    }

    #[test]
    fn override_agent_config() {
        let mut registry = AgentRegistry::new();
        registry.override_agent(
            "Sisyphus",
            AgentOverride {
                temperature: Some(0.5),
                ..Default::default()
            },
        );
        let config = registry.get("Sisyphus").unwrap();
        assert_eq!(config.temperature, 0.5);
    }

    #[test]
    fn list_subagents() {
        let registry = AgentRegistry::new();
        let subagents = registry.subagents();
        assert!(subagents.contains(&"Oracle"));
        assert!(subagents.contains(&"Librarian"));
        assert!(!subagents.contains(&"Sisyphus"));
    }
}
