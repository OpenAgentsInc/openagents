//! Stub keyring store implementation
//!
//! This is a simplified stub that uses environment variables instead of
//! system keyrings. For production use, consider implementing proper
//! keyring support via the `keyring` crate.

use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

/// Keyring store error
#[derive(Debug, Error)]
pub enum KeyringError {
    #[error("keyring access error: {0}")]
    Access(String),
    #[error("lock error: {0}")]
    Lock(String),
}

impl KeyringError {
    pub fn into_error(self) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(self)
    }

    pub fn message(&self) -> &str {
        match self {
            KeyringError::Access(msg) => msg,
            KeyringError::Lock(msg) => msg,
        }
    }
}

/// A simple in-memory keyring store that falls back to environment variables
pub struct DefaultKeyringStore {
    memory: RwLock<HashMap<String, String>>,
}

impl Default for DefaultKeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultKeyringStore {
    pub fn new() -> Self {
        Self {
            memory: RwLock::new(HashMap::new()),
        }
    }
}

/// Trait for keyring operations
pub trait KeyringStore: Send + Sync {
    /// Get a value from the keyring
    fn get(&self, service: &str, key: &str) -> Option<String>;

    /// Set a value in the keyring
    fn set(&self, service: &str, key: &str, value: &str) -> Result<(), KeyringError>;

    /// Delete a value from the keyring
    fn delete(&self, service: &str, key: &str) -> Result<(), KeyringError>;

    /// Load a value from the keyring
    fn load(&self, service: &str, key: &str) -> Result<Option<String>, KeyringError> {
        Ok(self.get(service, key))
    }

    /// Save a value to the keyring
    fn save(&self, service: &str, key: &str, value: &str) -> Result<(), KeyringError> {
        self.set(service, key, value)
    }
}

impl KeyringStore for DefaultKeyringStore {
    fn get(&self, service: &str, key: &str) -> Option<String> {
        // First try environment variable
        let env_key = format!("{}_{}", service.to_uppercase(), key.to_uppercase());
        if let Ok(value) = std::env::var(&env_key) {
            return Some(value);
        }

        // Then try memory store
        let store = self.memory.read().ok()?;
        let full_key = format!("{}:{}", service, key);
        store.get(&full_key).cloned()
    }

    fn set(&self, service: &str, key: &str, value: &str) -> Result<(), KeyringError> {
        let mut store = self
            .memory
            .write()
            .map_err(|e| KeyringError::Lock(e.to_string()))?;
        let full_key = format!("{}:{}", service, key);
        store.insert(full_key, value.to_string());
        Ok(())
    }

    fn delete(&self, service: &str, key: &str) -> Result<(), KeyringError> {
        let mut store = self
            .memory
            .write()
            .map_err(|e| KeyringError::Lock(e.to_string()))?;
        let full_key = format!("{}:{}", service, key);
        store.remove(&full_key);
        Ok(())
    }
}

/// Get API key from environment
pub fn get_api_key() -> Option<String> {
    std::env::var("OPENAI_API_KEY").ok()
}

/// Get API key for a specific provider
pub fn get_provider_api_key(provider: &str) -> Option<String> {
    let key_name = format!("{}_API_KEY", provider.to_uppercase());
    std::env::var(&key_name).ok()
}
