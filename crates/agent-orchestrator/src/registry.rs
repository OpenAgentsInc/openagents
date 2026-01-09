use crate::agents::builtin_agents;
use crate::config::{AgentConfig, AgentMode, AgentOverride, OrchestratorConfig, PermissionLevel};
use crate::error::{Error, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, warn};

// DSPy pipeline integration (native-only)
#[cfg(not(target_arch = "wasm32"))]
use crate::dspy_delegation::TargetAgent;
#[cfg(not(target_arch = "wasm32"))]
use crate::dspy_pipelines::{DelegationInput, DelegationPipeline, DelegationResult};

pub struct AgentRegistry {
    agents: HashMap<String, AgentConfig>,
    disabled: HashSet<String>,
    overrides: HashMap<String, AgentOverride>,
    /// DSPy delegation pipeline (native-only)
    #[cfg(not(target_arch = "wasm32"))]
    delegation_pipeline: DelegationPipeline,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: builtin_agents(),
            disabled: HashSet::new(),
            overrides: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            delegation_pipeline: DelegationPipeline::new(),
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

    // =========================================================================
    // DSPy Delegation Methods (native-only)
    // =========================================================================

    /// Delegate a task to the best subagent using DSPy (native-only).
    ///
    /// Returns the delegation result with assigned agent, refined task,
    /// expected deliverables, and fallback agent.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn delegate_task(&self, task: &str) -> DelegationResult {
        let input = DelegationInput {
            task_description: task.to_string(),
            available_agents: self.get_available_agents_json(),
            current_workload: self.get_current_workload_json(),
        };

        match self.delegation_pipeline.delegate(&input).await {
            Ok(result) if result.confidence > 0.5 => {
                debug!(
                    "DSPy delegation: {} -> {} (confidence: {:.2})",
                    task.chars().take(50).collect::<String>(),
                    result.assigned_agent,
                    result.confidence
                );
                result
            }
            Ok(result) => {
                debug!(
                    "DSPy delegation confidence too low: {:.2}, using legacy",
                    result.confidence
                );
                self.delegate_legacy(task)
            }
            Err(e) => {
                warn!("DSPy delegation failed: {}, using legacy", e);
                self.delegate_legacy(task)
            }
        }
    }

    /// Get available agents as JSON for DSPy input.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_available_agents_json(&self) -> String {
        let available: Vec<_> = self
            .agents
            .iter()
            .filter(|(name, _)| !self.disabled.contains(*name))
            .map(|(name, config)| {
                serde_json::json!({
                    "name": name,
                    "model": config.model,
                    "mode": format!("{:?}", config.mode),
                    "can_edit": config.permission.edit == PermissionLevel::Allow,
                })
            })
            .collect();

        serde_json::to_string(&available).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get current workload as JSON for DSPy input.
    #[cfg(not(target_arch = "wasm32"))]
    fn get_current_workload_json(&self) -> String {
        // For now, return empty workload - in future, track active tasks per agent
        "{}".to_string()
    }

    /// Legacy rule-based delegation fallback.
    #[cfg(not(target_arch = "wasm32"))]
    fn delegate_legacy(&self, task: &str) -> DelegationResult {
        let task_lower = task.to_lowercase();

        // Simple keyword-based routing
        let assigned_agent = if task_lower.contains("ui")
            || task_lower.contains("css")
            || task_lower.contains("styling")
            || task_lower.contains("visual")
            || task_lower.contains("tailwind")
        {
            TargetAgent::Frontend
        } else if task_lower.contains("readme")
            || task_lower.contains("documentation")
            || task_lower.contains("docs")
            || task_lower.contains("api doc")
        {
            TargetAgent::DocWriter
        } else if task_lower.contains("find")
            || task_lower.contains("where is")
            || task_lower.contains("locate")
            || task_lower.contains("search")
            || task_lower.contains("how does")
        {
            TargetAgent::Explore
        } else if task_lower.contains("library")
            || task_lower.contains("external")
            || task_lower.contains("npm")
            || task_lower.contains("crate")
            || task_lower.contains("package")
        {
            TargetAgent::Librarian
        } else if task_lower.contains("architecture")
            || task_lower.contains("design")
            || task_lower.contains("security")
            || task_lower.contains("debug")
        {
            TargetAgent::Oracle
        } else if task_lower.contains("image")
            || task_lower.contains("pdf")
            || task_lower.contains("screenshot")
            || task_lower.contains("diagram")
        {
            TargetAgent::Multimodal
        } else {
            TargetAgent::Direct
        };

        DelegationResult {
            assigned_agent,
            task_refinement: task.to_string(),
            expected_deliverables: "Complete the requested task.".to_string(),
            fallback_agent: TargetAgent::Direct,
            confidence: 0.7, // Legacy always has moderate confidence
        }
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
