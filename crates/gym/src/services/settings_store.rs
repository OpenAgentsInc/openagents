//! Settings store - persists TBCC settings to disk

use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};

use crate::tbcc::types::{ExecutionSettings, ContainerSettings};

/// Combined settings for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBCCSettings {
    pub execution: ExecutionSettings,
    pub container: ContainerSettings,
}

impl Default for TBCCSettings {
    fn default() -> Self {
        Self {
            execution: ExecutionSettings::default(),
            container: ContainerSettings::default(),
        }
    }
}

/// Settings store service
pub struct SettingsStore {
    data_dir: PathBuf,
    settings_file: PathBuf,
}

impl SettingsStore {
    pub fn new(data_dir: PathBuf) -> Self {
        let settings_file = data_dir.join("tbcc_settings.json");
        Self { data_dir, settings_file }
    }

    /// Load settings from disk, or return defaults
    pub fn load(&self) -> TBCCSettings {
        if let Ok(content) = fs::read_to_string(&self.settings_file) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
        TBCCSettings::default()
    }

    /// Save settings to disk
    pub fn save(&self, settings: &TBCCSettings) -> Result<(), SettingsError> {
        // Ensure directory exists
        fs::create_dir_all(&self.data_dir)
            .map_err(|e| SettingsError::IoError(e.to_string()))?;

        let content = serde_json::to_string_pretty(settings)
            .map_err(|e| SettingsError::SerializeError(e.to_string()))?;

        fs::write(&self.settings_file, content)
            .map_err(|e| SettingsError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Update execution settings
    pub fn update_execution(&self, execution: ExecutionSettings) -> Result<(), SettingsError> {
        let mut settings = self.load();
        settings.execution = execution;
        self.save(&settings)
    }

    /// Update container settings
    pub fn update_container(&self, container: ContainerSettings) -> Result<(), SettingsError> {
        let mut settings = self.load();
        settings.container = container;
        self.save(&settings)
    }

    /// Reset to defaults
    pub fn reset(&self) -> Result<(), SettingsError> {
        self.save(&TBCCSettings::default())
    }
}

#[derive(Debug)]
pub enum SettingsError {
    IoError(String),
    SerializeError(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::SerializeError(e) => write!(f, "Serialize error: {}", e),
        }
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_settings_persistence() {
        let dir = temp_dir().join("test_settings");
        let _ = fs::create_dir_all(&dir);

        let store = SettingsStore::new(dir.clone());

        // Save custom settings
        let mut settings = TBCCSettings::default();
        settings.execution.max_attempts = 10;
        settings.container.memory_limit = "8G".to_string();

        store.save(&settings).unwrap();

        // Load them back
        let loaded = store.load();
        assert_eq!(loaded.execution.max_attempts, 10);
        assert_eq!(loaded.container.memory_limit, "8G");

        // Cleanup
        let _ = fs::remove_dir_all(dir);
    }
}
