//! Wallet configuration management

use crate::storage::identities::{DEFAULT_IDENTITY_NAME, current_identity};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WalletConfig {
    pub network: NetworkConfig,
    pub nostr: NostrConfig,
    pub storage: StorageConfig,
    pub security: SecurityConfig,
}

impl WalletConfig {
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if path.exists() {
            let contents = fs::read_to_string(&path)?;
            let config: WalletConfig = toml::from_str(&contents)?;
            Ok(config)
        } else {
            // Create default config
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)?;
        fs::write(&path, contents)?;

        Ok(())
    }

    /// Get configuration file path
    fn config_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".openagents").join("wallet.toml"))
    }

    /// Get database path (expanded)
    #[allow(dead_code)]
    pub fn db_path(&self) -> Result<PathBuf> {
        let path = shellexpand::tilde(&self.storage.db_path);
        Ok(PathBuf::from(path.as_ref()))
    }

    /// Get profile path
    pub fn profile_path(&self) -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let identity = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
        Ok(home
            .join(".openagents")
            .join("profiles")
            .join(format!("{}.json", identity)))
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Bitcoin network (mainnet, testnet, signet, regtest)
    pub bitcoin: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bitcoin: "mainnet".to_string(),
        }
    }
}

/// Nostr configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrConfig {
    /// List of relay URLs
    pub relays: Vec<String>,
}

impl Default for NostrConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
            ],
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Database path
    pub db_path: String,
    /// Enable backups
    pub backup_enabled: bool,
    /// Backup directory path
    pub backup_path: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "~/.openagents/wallet.db".to_string(),
            backup_enabled: false,
            backup_path: None,
        }
    }
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Max sats allowed per outgoing payment (None disables limit)
    pub max_send_sats: Option<u64>,
    /// Require confirmation prompt for payments >= this amount (None disables)
    pub confirm_large_sats: Option<u64>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_send_sats: None,
            confirm_large_sats: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WalletConfig::default();
        assert_eq!(config.network.bitcoin, "mainnet");
        assert!(!config.nostr.relays.is_empty());
        assert!(config.security.max_send_sats.is_none());
        assert!(config.security.confirm_large_sats.is_none());
    }

    #[test]
    fn test_serialize_config() {
        let config = WalletConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("bitcoin"));
        assert!(toml_str.contains("relays"));
        assert!(toml_str.contains("security"));
    }
}
