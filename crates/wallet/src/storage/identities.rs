//! Identity registry for managing multiple wallet identities.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub use super::keychain::DEFAULT_IDENTITY_NAME;
use super::keychain::SecureKeychain;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityRegistry {
    current: String,
    identities: Vec<String>,
}

impl IdentityRegistry {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let contents = fs::read_to_string(&path).context("Failed to read identity registry")?;
            let registry: IdentityRegistry =
                serde_json::from_str(&contents).context("Failed to parse identity registry")?;
            return Ok(registry);
        }

        let mut registry = IdentityRegistry {
            current: DEFAULT_IDENTITY_NAME.to_string(),
            identities: Vec::new(),
        };

        if SecureKeychain::has_mnemonic_for(DEFAULT_IDENTITY_NAME) {
            registry.identities.push(DEFAULT_IDENTITY_NAME.to_string());
        }

        registry.save()?;
        Ok(registry)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create identity registry directory")?;
        }
        let contents =
            serde_json::to_string_pretty(self).context("Failed to serialize identity registry")?;
        fs::write(&path, contents).context("Failed to write identity registry")?;
        Ok(())
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn identities(&self) -> &[String] {
        &self.identities
    }

    pub fn contains(&self, name: &str) -> bool {
        self.identities.iter().any(|entry| entry == name)
    }

    pub fn add_identity(&mut self, name: &str) -> Result<()> {
        if self.contains(name) {
            anyhow::bail!("Identity '{}' already exists.", name);
        }
        self.identities.push(name.to_string());
        Ok(())
    }

    pub fn remove_identity(&mut self, name: &str) -> Result<()> {
        if !self.contains(name) {
            anyhow::bail!("Identity '{}' does not exist.", name);
        }
        self.identities.retain(|entry| entry != name);
        if self.current == name {
            if let Some(next) = self.identities.first() {
                self.current = next.clone();
            } else {
                self.current = DEFAULT_IDENTITY_NAME.to_string();
            }
        }
        Ok(())
    }

    pub fn set_current(&mut self, name: &str) -> Result<()> {
        if !self.contains(name) {
            anyhow::bail!("Identity '{}' does not exist.", name);
        }
        self.current = name.to_string();
        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home.join(".openagents").join("identities.json"))
    }
}

pub fn current_identity() -> Result<String> {
    Ok(IdentityRegistry::load()?.current)
}

pub fn register_identity(name: &str, set_current: bool) -> Result<()> {
    let mut registry = IdentityRegistry::load()?;
    if !registry.contains(name) {
        registry.add_identity(name)?;
    }
    if set_current {
        registry.current = name.to_string();
    }
    registry.save()
}

pub fn set_current_identity(name: &str) -> Result<()> {
    let mut registry = IdentityRegistry::load()?;
    registry.set_current(name)?;
    registry.save()
}

pub fn remove_identity(name: &str) -> Result<()> {
    let mut registry = IdentityRegistry::load()?;
    registry.remove_identity(name)?;
    registry.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::keychain::SecureKeychain;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_identity_registry_add_switch_remove() {
        let _guard = ENV_LOCK.lock().unwrap();
        let temp = tempfile::TempDir::new().expect("temp dir");
        let keychain_path = temp.path().join("keychain.txt");
        let original_home = std::env::var("HOME").ok();
        let original_keychain = std::env::var("OPENAGENTS_KEYCHAIN_FILE").ok();

        unsafe {
            std::env::set_var("HOME", temp.path());
            std::env::set_var("OPENAGENTS_KEYCHAIN_FILE", &keychain_path);
        }

        let registry = IdentityRegistry::load().expect("load registry");
        assert!(registry.identities().is_empty());
        assert_eq!(registry.current(), DEFAULT_IDENTITY_NAME);

        SecureKeychain::store_mnemonic_for(
            "work",
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        )
        .expect("store mnemonic");
        register_identity("work", true).expect("register identity");
        let registry = IdentityRegistry::load().expect("reload registry");
        assert_eq!(registry.current(), "work");
        assert!(registry.identities().contains(&"work".to_string()));

        remove_identity("work").expect("remove identity");
        let registry = IdentityRegistry::load().expect("reload registry");
        assert!(!registry.identities().contains(&"work".to_string()));
        assert_eq!(registry.current(), DEFAULT_IDENTITY_NAME);

        if let Some(value) = original_home {
            unsafe {
                std::env::set_var("HOME", value);
            }
        } else {
            unsafe {
                std::env::remove_var("HOME");
            }
        }

        if let Some(value) = original_keychain {
            unsafe {
                std::env::set_var("OPENAGENTS_KEYCHAIN_FILE", value);
            }
        } else {
            unsafe {
                std::env::remove_var("OPENAGENTS_KEYCHAIN_FILE");
            }
        }
    }
}
