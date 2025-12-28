//! Pylon configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Pylon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PylonConfig {
    /// Provider name shown in NIP-89 handler info
    pub name: String,
    /// Provider description
    pub description: Option<String>,
    /// Nostr relays to connect to
    pub relays: Vec<String>,
    /// Minimum price in millisats per job
    pub min_price_msats: u64,
    /// Whether to require payment before processing
    pub require_payment: bool,
    /// Default model to use if not specified in job
    pub default_model: String,
    /// Backend preferences (order of preference)
    pub backend_preference: Vec<String>,
    /// Data directory for storage
    pub data_dir: Option<PathBuf>,
    /// Network for Lightning payments (mainnet, testnet, signet, regtest)
    #[serde(default = "default_network")]
    pub network: String,
    /// Whether payments are enabled (requires wallet config)
    #[serde(default)]
    pub enable_payments: bool,
    /// Spark wallet URL (e.g., "https://localhost:9737")
    #[serde(default)]
    pub spark_url: Option<String>,
    /// Spark wallet auth token
    #[serde(default)]
    pub spark_token: Option<String>,
}

fn default_network() -> String {
    "regtest".to_string()
}

impl Default for PylonConfig {
    fn default() -> Self {
        Self {
            name: "Pylon Provider".to_string(),
            description: None,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
            min_price_msats: 1000, // 1 sat minimum
            require_payment: true,
            default_model: "llama3.2".to_string(),
            backend_preference: vec![
                "ollama".to_string(),
                "llamacpp".to_string(),
                "apple_fm".to_string(),
            ],
            data_dir: None,
            network: "regtest".to_string(),
            enable_payments: true, // Enabled by default
            spark_url: None,
            spark_token: None,
        }
    }
}

impl PylonConfig {
    /// Load config from file or create default
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: PylonConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to file
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// Get config file path
    pub fn config_path() -> anyhow::Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("pylon").join("config.toml"))
    }

    /// Get data directory path
    pub fn data_path(&self) -> anyhow::Result<PathBuf> {
        if let Some(ref path) = self.data_dir {
            Ok(path.clone())
        } else {
            let data_dir = dirs::data_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
            Ok(data_dir.join("pylon"))
        }
    }

    /// Set provider name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set relays
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.relays = relays;
        self
    }

    /// Set minimum price
    pub fn with_min_price(mut self, msats: u64) -> Self {
        self.min_price_msats = msats;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PylonConfig::default();
        assert_eq!(config.min_price_msats, 1000);
        assert!(!config.relays.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = PylonConfig::default();
        let toml_str = toml::to_string(&config).expect("should serialize");
        let parsed: PylonConfig = toml::from_str(&toml_str).expect("should deserialize");
        assert_eq!(config.name, parsed.name);
    }
}
