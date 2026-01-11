//! Authentication management for OpenAgents
//!
//! Supports importing credentials from OpenCode and managing local auth state.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// OpenCode auth file location (XDG data dir)
pub fn opencode_auth_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());

    // XDG_DATA_HOME or default
    let data_home =
        std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));

    PathBuf::from(data_home).join("opencode").join("auth.json")
}

/// OpenAgents auth file location
pub fn openagents_auth_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".openagents").join("auth.json")
}

/// Auth entry types matching OpenCode's format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthEntry {
    Oauth {
        refresh: String,
        access: String,
        expires: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "enterpriseUrl")]
        enterprise_url: Option<String>,
    },
    Api {
        key: String,
    },
    Wellknown {
        key: String,
        token: String,
    },
}

/// Collection of auth entries by provider
pub type AuthStore = HashMap<String, AuthEntry>;

/// Status of auth check
#[derive(Debug, Clone)]
pub enum AuthStatus {
    NotFound,
    Found { providers: Vec<String> },
    Copied { providers: Vec<String> },
    Error(String),
}

/// Check if OpenCode auth exists and return its status
pub fn check_opencode_auth() -> AuthStatus {
    let path = opencode_auth_path();
    info!("Checking OpenCode auth at {:?}", path);

    if !path.exists() {
        warn!("OpenCode auth not found at {:?}", path);
        return AuthStatus::NotFound;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            debug!("Read {} bytes from OpenCode auth.json", content.len());
            match serde_json::from_str::<AuthStore>(&content) {
                Ok(store) => {
                    let providers: Vec<String> = store.keys().cloned().collect();
                    if providers.is_empty() {
                        warn!("OpenCode auth.json is empty");
                        AuthStatus::NotFound
                    } else {
                        info!("Found OpenCode auth with providers: {:?}", providers);
                        AuthStatus::Found { providers }
                    }
                }
                Err(e) => {
                    error!("Failed to parse OpenCode auth.json: {}", e);
                    AuthStatus::Error(format!("Invalid auth.json: {}", e))
                }
            }
        }
        Err(e) => {
            error!("Failed to read OpenCode auth.json: {}", e);
            AuthStatus::Error(format!("Failed to read auth.json: {}", e))
        }
    }
}

/// Check if OpenAgents auth exists
pub fn check_openagents_auth() -> AuthStatus {
    let path = openagents_auth_path();
    info!("Checking OpenAgents auth at {:?}", path);

    if !path.exists() {
        warn!("OpenAgents auth not found at {:?}", path);
        return AuthStatus::NotFound;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            debug!("Read {} bytes from OpenAgents auth.json", content.len());
            match serde_json::from_str::<AuthStore>(&content) {
                Ok(store) => {
                    let providers: Vec<String> = store.keys().cloned().collect();
                    if providers.is_empty() {
                        warn!("OpenAgents auth.json is empty");
                        AuthStatus::NotFound
                    } else {
                        info!("Found OpenAgents auth with providers: {:?}", providers);
                        AuthStatus::Found { providers }
                    }
                }
                Err(e) => {
                    error!("Failed to parse OpenAgents auth.json: {}", e);
                    AuthStatus::Error(format!("Invalid auth.json: {}", e))
                }
            }
        }
        Err(e) => {
            error!("Failed to read OpenAgents auth.json: {}", e);
            AuthStatus::Error(format!("Failed to read auth.json: {}", e))
        }
    }
}

/// Copy OpenCode auth to OpenAgents
pub fn copy_opencode_auth() -> Result<AuthStatus> {
    let src_path = opencode_auth_path();
    let dst_path = openagents_auth_path();

    info!("Copying auth from {:?} to {:?}", src_path, dst_path);

    // Read source
    let content =
        std::fs::read_to_string(&src_path).context("Failed to read OpenCode auth.json")?;

    let store: AuthStore =
        serde_json::from_str(&content).context("Failed to parse OpenCode auth.json")?;

    let providers: Vec<String> = store.keys().cloned().collect();

    if providers.is_empty() {
        warn!("No providers found in OpenCode auth");
        return Ok(AuthStatus::NotFound);
    }

    // Create destination directory
    if let Some(parent) = dst_path.parent() {
        debug!("Creating directory {:?}", parent);
        std::fs::create_dir_all(parent).context("Failed to create ~/.openagents directory")?;
    }

    // Write destination with restricted permissions
    let json = serde_json::to_string_pretty(&store).context("Failed to serialize auth")?;

    std::fs::write(&dst_path, &json).context("Failed to write OpenAgents auth.json")?;

    // Set file permissions to 600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&dst_path, perms)
            .context("Failed to set auth.json permissions")?;
    }

    info!("Copied {} providers to OpenAgents auth", providers.len());
    Ok(AuthStatus::Copied { providers })
}

/// Get a specific provider's auth entry from OpenAgents store
pub fn get_provider_auth(provider: &str) -> Result<Option<AuthEntry>> {
    let path = openagents_auth_path();
    debug!("Getting auth for provider '{}' from {:?}", provider, path);

    if !path.exists() {
        debug!("Auth file does not exist");
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let store: AuthStore = serde_json::from_str(&content)?;

    let entry = store.get(provider).cloned();
    if entry.is_some() {
        debug!("Found auth entry for provider '{}'", provider);
    } else {
        debug!("No auth entry for provider '{}'", provider);
    }

    Ok(entry)
}

/// Check if we have valid Anthropic auth
pub fn has_anthropic_auth() -> bool {
    let has_auth = matches!(get_provider_auth("anthropic"), Ok(Some(_)));
    if has_auth {
        info!("Anthropic auth is configured");
    } else {
        warn!("Anthropic auth is NOT configured");
    }
    has_auth
}

/// Get the Anthropic API key if available
pub fn get_anthropic_api_key() -> Result<Option<String>> {
    match get_provider_auth("anthropic")? {
        Some(AuthEntry::Api { key }) => {
            debug!("Got Anthropic API key");
            Ok(Some(key))
        }
        Some(AuthEntry::Oauth { access, .. }) => {
            debug!("Got Anthropic OAuth access token");
            Ok(Some(access))
        }
        Some(AuthEntry::Wellknown { token, .. }) => {
            debug!("Got Anthropic wellknown token");
            Ok(Some(token))
        }
        None => {
            debug!("No Anthropic auth found");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opencode_auth_path() {
        let path = opencode_auth_path();
        assert!(path.to_string_lossy().contains("opencode"));
        assert!(path.to_string_lossy().ends_with("auth.json"));
    }

    #[test]
    fn test_openagents_auth_path() {
        let path = openagents_auth_path();
        assert!(path.to_string_lossy().contains(".openagents"));
        assert!(path.to_string_lossy().ends_with("auth.json"));
    }

    #[test]
    fn test_auth_entry_deserialization() {
        let json = r#"{
            "anthropic": {
                "type": "oauth",
                "refresh": "test-refresh",
                "access": "test-access",
                "expires": 1234567890
            },
            "openai": {
                "type": "api",
                "key": "sk-test"
            }
        }"#;

        let store: AuthStore = serde_json::from_str(json).unwrap();
        assert!(store.contains_key("anthropic"));
        assert!(store.contains_key("openai"));
    }
}
