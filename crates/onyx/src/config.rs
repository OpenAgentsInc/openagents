//! User configuration and preferences

use serde::{Deserialize, Serialize};

/// User preferences for Onyx
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Path to the vault directory
    pub vault_path: Option<String>,
    /// Enable autosave
    pub autosave: bool,
    /// Autosave interval in seconds
    pub autosave_interval: u64,
}
