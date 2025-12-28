//! Agent Registry
//!
//! Persists agent configurations to disk at ~/.openagents/agents/
//! Provides lookup by npub or name.

use crate::config::AgentConfig;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during registry operations
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("no config directory found")]
    NoConfigDir,

    #[error("agent not found: {0}")]
    NotFound(String),

    #[error("agent already exists: {0}")]
    AlreadyExists(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Registry for managing agent configurations on disk
pub struct AgentRegistry {
    agents_dir: PathBuf,
}

impl AgentRegistry {
    /// Create a new registry with default path (~/.openagents/agents/)
    pub fn new() -> Result<Self, RegistryError> {
        let agents_dir = Self::default_agents_dir()?;
        std::fs::create_dir_all(&agents_dir)?;
        Ok(Self { agents_dir })
    }

    /// Create a registry with a custom path (useful for testing)
    pub fn with_path(path: PathBuf) -> Result<Self, RegistryError> {
        std::fs::create_dir_all(&path)?;
        Ok(Self { agents_dir: path })
    }

    /// Get the default agents directory
    pub fn default_agents_dir() -> Result<PathBuf, RegistryError> {
        dirs::config_dir()
            .map(|p| p.join("openagents").join("agents"))
            .ok_or(RegistryError::NoConfigDir)
    }

    /// Get the path to an agent's config file
    fn agent_path(&self, npub: &str) -> PathBuf {
        // Sanitize npub for filesystem
        let safe_name = npub.replace(['/', '\\', ':'], "_");
        self.agents_dir.join(format!("{}.toml", safe_name))
    }

    /// Save an agent configuration
    pub fn save(&self, config: &AgentConfig) -> Result<(), RegistryError> {
        let path = self.agent_path(&config.npub);
        let content = toml::to_string_pretty(config)
            .map_err(|e| RegistryError::Serialization(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load an agent configuration by npub or name
    pub fn load(&self, identifier: &str) -> Result<AgentConfig, RegistryError> {
        // Try loading by npub first
        let path = self.agent_path(identifier);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            return toml::from_str(&content)
                .map_err(|e| RegistryError::Serialization(e.to_string()));
        }

        // Search by name
        for entry in std::fs::read_dir(&self.agents_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "toml") {
                let content = std::fs::read_to_string(&path)?;
                let config: AgentConfig = toml::from_str(&content)
                    .map_err(|e| RegistryError::Serialization(e.to_string()))?;

                if config.name == identifier {
                    return Ok(config);
                }
            }
        }

        Err(RegistryError::NotFound(identifier.to_string()))
    }

    /// Check if an agent exists by npub or name
    pub fn exists(&self, identifier: &str) -> bool {
        self.load(identifier).is_ok()
    }

    /// List all agent configurations
    pub fn list(&self) -> Result<Vec<AgentConfig>, RegistryError> {
        let mut agents = Vec::new();

        if !self.agents_dir.exists() {
            return Ok(agents);
        }

        for entry in std::fs::read_dir(&self.agents_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|e| e == "toml") {
                let content = std::fs::read_to_string(&path)?;
                match toml::from_str::<AgentConfig>(&content) {
                    Ok(config) => agents.push(config),
                    Err(e) => {
                        eprintln!("Warning: failed to parse {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sort by creation time (newest first)
        agents.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(agents)
    }

    /// Update an agent's state
    pub fn update_state(
        &self,
        identifier: &str,
        state: crate::config::LifecycleState,
    ) -> Result<(), RegistryError> {
        let mut config = self.load(identifier)?;
        config.state = state;
        config.last_active_at = chrono::Utc::now().timestamp() as u64;
        self.save(&config)
    }

    /// Increment tick count
    pub fn record_tick(&self, identifier: &str) -> Result<(), RegistryError> {
        let mut config = self.load(identifier)?;
        config.tick_count += 1;
        config.last_active_at = chrono::Utc::now().timestamp() as u64;
        self.save(&config)
    }

    /// Delete an agent configuration
    pub fn delete(&self, identifier: &str) -> Result<(), RegistryError> {
        let config = self.load(identifier)?;
        let path = self.agent_path(&config.npub);
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Get agents by state
    pub fn list_by_state(
        &self,
        state: crate::config::LifecycleState,
    ) -> Result<Vec<AgentConfig>, RegistryError> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|a| a.state == state)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LifecycleState;

    #[test]
    fn test_registry_save_load() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::with_path(temp_dir.path().to_path_buf()).unwrap();

        let config = AgentConfig::new(
            "TestAgent".to_string(),
            "abc123".to_string(),
            "npub1test".to_string(),
            "encrypted_mnemonic".to_string(),
            "sp1address".to_string(),
        );

        registry.save(&config).unwrap();

        // Load by npub
        let loaded = registry.load("npub1test").unwrap();
        assert_eq!(loaded.name, "TestAgent");

        // Load by name
        let loaded = registry.load("TestAgent").unwrap();
        assert_eq!(loaded.npub, "npub1test");
    }

    #[test]
    fn test_registry_list() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::with_path(temp_dir.path().to_path_buf()).unwrap();

        // Create multiple agents
        for i in 0..3 {
            let config = AgentConfig::new(
                format!("Agent{}", i),
                format!("pubkey{}", i),
                format!("npub{}", i),
                "encrypted".to_string(),
                format!("sp1addr{}", i),
            );
            registry.save(&config).unwrap();
        }

        let agents = registry.list().unwrap();
        assert_eq!(agents.len(), 3);
    }

    #[test]
    fn test_registry_update_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::with_path(temp_dir.path().to_path_buf()).unwrap();

        let config = AgentConfig::new(
            "TestAgent".to_string(),
            "abc123".to_string(),
            "npub1test".to_string(),
            "encrypted".to_string(),
            "sp1addr".to_string(),
        );
        registry.save(&config).unwrap();

        registry
            .update_state("npub1test", LifecycleState::Active)
            .unwrap();

        let loaded = registry.load("npub1test").unwrap();
        assert_eq!(loaded.state, LifecycleState::Active);
    }

    #[test]
    fn test_registry_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::with_path(temp_dir.path().to_path_buf()).unwrap();

        let result = registry.load("nonexistent");
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn test_registry_delete() {
        let temp_dir = tempfile::tempdir().unwrap();
        let registry = AgentRegistry::with_path(temp_dir.path().to_path_buf()).unwrap();

        let config = AgentConfig::new(
            "TestAgent".to_string(),
            "abc123".to_string(),
            "npub1test".to_string(),
            "encrypted".to_string(),
            "sp1addr".to_string(),
        );
        registry.save(&config).unwrap();
        assert!(registry.exists("npub1test"));

        registry.delete("npub1test").unwrap();
        assert!(!registry.exists("npub1test"));
    }
}
