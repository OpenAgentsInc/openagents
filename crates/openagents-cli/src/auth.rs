//! Authentication, credential store, OS keychain / secret-tool adapter, and persistent state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileConfig {
    pub api_url: Option<String>,
    pub token: Option<String>,
    pub identity_name: Option<String>,
}

pub struct CredentialStore {
    config_path: PathBuf,
}

impl CredentialStore {
    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".openagents").join("config.json")
    }

    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            config_path: path.unwrap_or_else(Self::default_path),
        }
    }

    pub fn load(&self) -> Result<AuthConfig, Box<dyn std::error::Error>> {
        if !self.config_path.exists() {
            return Ok(AuthConfig {
                default_profile: Some("default".to_string()),
                profiles: HashMap::new(),
            });
        }
        let data = fs::read_to_string(&self.config_path)?;
        let config: AuthConfig = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn save(&self, config: &AuthConfig) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(config)?;
        fs::write(&self.config_path, data)?;
        Ok(())
    }

    pub fn get_token(&self) -> Option<String> {
        if let Ok(env_token) = std::env::var("OPENAGENTS_TOKEN") {
            if !env_token.trim().is_empty() {
                return Some(env_token.trim().to_string());
            }
        }

        if let Ok(config) = self.load() {
            let profile_key = config.default_profile.unwrap_or_else(|| "default".to_string());
            if let Some(token) = config.profiles.get(&profile_key).and_then(|p| p.token.clone()) {
                if !token.trim().is_empty() {
                    return Some(token);
                }
            }
        }

        // Try OS Keychain on macOS with explicit origin account keys
        #[cfg(target_os = "macos")]
        {
            for origin in ["https://openagents.com", "http://localhost:4000", "https://staging.openagents.com"] {
                if let Ok(output) = Command::new("security")
                    .args(["find-generic-password", "-a", origin, "-s", "openagents-cli", "-w"])
                    .output()
                {
                    if output.status.success() {
                        let token_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if token_str.starts_with("oa_pat_") || token_str.starts_with("smct_") {
                            return Some(token_str);
                        }
                    }
                }
            }

            if let Ok(output) = Command::new("security")
                .args(["find-generic-password", "-s", "openagents-cli", "-w"])
                .output()
            {
                if output.status.success() {
                    let token_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if token_str.starts_with("oa_pat_") || token_str.starts_with("smct_") {
                        return Some(token_str);
                    }
                }
            }
        }

        // Try secret-tool on Linux
        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("secret-tool")
                .args(["lookup", "service", "openagents-cli"])
                .output()
            {
                if output.status.success() {
                    let token_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !token_str.is_empty() {
                        return Some(token_str);
                    }
                }
            }
        }

        None
    }

    pub fn set_token(&self, token: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load().unwrap_or_else(|_| AuthConfig {
            default_profile: Some("default".to_string()),
            profiles: HashMap::new(),
        });
        let profile_key = config.default_profile.clone().unwrap_or_else(|| "default".to_string());
        let profile = config.profiles.entry(profile_key).or_insert_with(Default::default);
        profile.token = Some(token.to_string());
        self.save(&config)?;

        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("security")
                .args(["add-generic-password", "-U", "-a", "https://openagents.com", "-s", "openagents-cli", "-w", token])
                .output();
        }

        Ok(())
    }

    pub fn clear_token(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load().unwrap_or_else(|_| AuthConfig {
            default_profile: Some("default".to_string()),
            profiles: HashMap::new(),
        });
        if let Some(profile_key) = &config.default_profile {
            if let Some(profile) = config.profiles.get_mut(profile_key) {
                profile.token = None;
            }
        }
        self.save(&config)?;

        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("security")
                .args(["delete-generic-password", "-a", "https://openagents.com", "-s", "openagents-cli"])
                .output();
        }

        Ok(())
    }
}
