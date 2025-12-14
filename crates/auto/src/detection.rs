//! Backend and credential detection.

use crate::config::AutoConfig;
use crate::Result;
use mechacoder::router::{Backend, Router, RouterConfig};
use std::collections::HashMap;
use std::path::Path;

/// Detection results for available backends and credentials.
#[derive(Debug, Clone)]
pub struct Detection {
    /// Available backends.
    available: Vec<Backend>,
    /// Selected backend.
    selected: Option<Backend>,
    /// Reason for backend selection.
    selection_reason: String,
    /// Detected credentials (env var name -> source).
    credentials: HashMap<String, CredentialSource>,
}

/// Source of a detected credential.
#[derive(Debug, Clone)]
pub enum CredentialSource {
    /// From environment variable.
    Environment,
    /// From .env.local file.
    EnvLocal,
    /// From system keychain.
    Keychain,
}

impl Detection {
    /// Detect available backends and credentials.
    pub fn detect(config: &AutoConfig) -> Result<Self> {
        let mut credentials = HashMap::new();

        // Check environment variables
        for key in &[
            "ANTHROPIC_API_KEY",
            "OPENROUTER_API_KEY",
            "OPENAI_API_KEY",
        ] {
            if std::env::var(key).is_ok() {
                credentials.insert(key.to_string(), CredentialSource::Environment);
            }
        }

        // Check .env.local file
        if let Some(env_local_path) = &config.env_local_path {
            Self::load_env_local(env_local_path, &mut credentials);
        }

        // Also check .env.local in working directory
        let working_env_local = config.working_directory.join(".env.local");
        if working_env_local.exists() {
            Self::load_env_local(&working_env_local, &mut credentials);
        }

        // Use mechacoder router for backend detection
        let router_config = RouterConfig::default();
        let mut router = Router::new(router_config);
        router.detect_sync();

        let available = router.detected_backends().to_vec();

        // Select backend based on priority and preferences
        let (selected, reason) = Self::select_backend(&available, config);

        Ok(Self {
            available,
            selected,
            selection_reason: reason,
            credentials,
        })
    }

    /// Load credentials from .env.local file.
    fn load_env_local(path: &Path, credentials: &mut HashMap<String, CredentialSource>) {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();

                // Skip comments and empty lines
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }

                // Parse KEY=value
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').trim_matches('\'');

                    // Only care about API keys
                    if key.ends_with("_API_KEY") && !value.is_empty() {
                        // Set env var if not already set
                        if std::env::var(key).is_err() {
                            // SAFETY: Called during initialization
                            unsafe {
                                std::env::set_var(key, value);
                            }
                        }

                        // Record that we found this credential
                        credentials
                            .entry(key.to_string())
                            .or_insert(CredentialSource::EnvLocal);
                    }
                }
            }
        }
    }

    /// Select the best backend based on priority and preferences.
    fn select_backend(
        available: &[Backend],
        config: &AutoConfig,
    ) -> (Option<Backend>, String) {
        // If user specified a preference, try to use it
        if let Some(preferred) = config.preferred_backend {
            if available.contains(&preferred) {
                return (
                    Some(preferred),
                    format!("User preferred: {}", preferred.display_name()),
                );
            }
        }

        // Priority order
        let priority = [
            (Backend::ClaudeCode, "Most capable, uses Claude CLI"),
            (Backend::Anthropic, "Direct Anthropic API"),
            (Backend::OpenRouter, "OpenRouter API (multi-model)"),
            (Backend::OpenAI, "OpenAI API"),
            (Backend::Ollama, "Local Ollama"),
            (Backend::Pi, "Built-in Pi agent"),
        ];

        for (backend, reason) in priority {
            if available.contains(&backend) {
                return (Some(backend), reason.to_string());
            }
        }

        (None, "No backend available".to_string())
    }

    /// Check if any backend is available.
    pub fn has_backend(&self) -> bool {
        self.selected.is_some()
    }

    /// Get available backends.
    pub fn available_backends(&self) -> &[Backend] {
        &self.available
    }

    /// Get the selected backend.
    pub fn selected_backend(&self) -> Option<Backend> {
        self.selected
    }

    /// Get the reason for backend selection.
    pub fn selection_reason(&self) -> &str {
        &self.selection_reason
    }

    /// Get detected credentials.
    pub fn credentials(&self) -> &HashMap<String, CredentialSource> {
        &self.credentials
    }

    /// Check if a specific credential is available.
    pub fn has_credential(&self, key: &str) -> bool {
        self.credentials.contains_key(key)
    }

    /// Get the model to use for the selected backend.
    pub fn default_model(&self) -> Option<&'static str> {
        self.selected.and_then(|b| b.default_model())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_with_no_credentials() {
        // Clear relevant env vars for test
        // Note: Can't actually clear because of threading, just test the logic
        let config = AutoConfig::default();
        let detection = Detection::detect(&config).unwrap();

        // Should at least have Pi available
        assert!(detection.available.contains(&Backend::Pi));
    }
}
