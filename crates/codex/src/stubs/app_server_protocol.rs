//! Stub app server protocol types
//!
//! These are simplified stubs for the app server protocol types.
//! The full implementation is in codex-app-server-protocol.

use serde::{Deserialize, Serialize};

/// Authentication mode for the app server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AuthMode {
    #[default]
    None,
    ApiKey,
    OAuth,
}

/// Configuration for the app server
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub model: Option<String>,
    pub working_directory: Option<String>,
}

/// Parameters for batch config writes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigBatchWriteParams {
    pub values: Vec<ConfigValueWriteParams>,
}

/// A configuration layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLayer {
    pub name: ConfigLayerName,
    pub values: serde_json::Value,
}

/// Metadata for a configuration layer
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigLayerMetadata {
    pub name: String,
    pub path: Option<String>,
}

/// Name of a configuration layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigLayerName {
    Default,
    User,
    Workspace,
    Override,
}

impl Default for ConfigLayerName {
    fn default() -> Self {
        Self::Default
    }
}

/// Parameters for reading config
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigReadParams {
    pub keys: Vec<String>,
}

/// Response for reading config
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigReadResponse {
    pub values: serde_json::Value,
}

/// Parameters for writing a config value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValueWriteParams {
    pub key: String,
    pub value: serde_json::Value,
}

/// Error code for config writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigWriteErrorCode {
    InvalidKey,
    InvalidValue,
    PermissionDenied,
    IoError,
}

/// Response for writing config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigWriteResponse {
    pub status: WriteStatus,
    pub error: Option<ConfigWriteErrorCode>,
}

/// Git SHA identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GitSha(pub String);

impl GitSha {
    pub fn new(sha: impl Into<String>) -> Self {
        Self(sha.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Merge strategy for config
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MergeStrategy {
    #[default]
    Override,
    Merge,
}

/// Metadata for overridden values
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OverriddenMetadata {
    pub original_layer: Option<ConfigLayerName>,
}

/// Available tools configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tools {
    pub shell: bool,
    pub read_file: bool,
    pub write_file: bool,
    pub apply_patch: bool,
}

/// User saved configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSavedConfig {
    pub model: Option<String>,
    pub api_key: Option<String>,
}

/// Write status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteStatus {
    Success,
    Failure,
}

/// User profile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

/// Sandbox settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxSettings {
    pub enabled: bool,
    pub policy: Option<String>,
}
