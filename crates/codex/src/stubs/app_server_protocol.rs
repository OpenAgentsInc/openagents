//! Stub app server protocol types
//!
//! These are simplified stubs for the app server protocol types.
//! The full implementation is in codex-app-server-protocol.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Authentication mode for the app server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AuthMode {
    #[default]
    None,
    ApiKey,
    OAuth,
    /// ChatGPT authentication mode (SSO)
    ChatGPT,
}

/// Configuration for the app server
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub model: Option<String>,
    pub working_directory: Option<String>,
}

/// A single edit in a batch write
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEdit {
    pub key_path: String,
    pub value: serde_json::Value,
    pub merge_strategy: Option<MergeStrategy>,
}

/// Parameters for batch config writes
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigBatchWriteParams {
    pub values: Vec<ConfigValueWriteParams>,
    pub file_path: Option<PathBuf>,
    pub expected_version: Option<String>,
    pub edits: Vec<ConfigEdit>,
}

/// A configuration layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigLayer {
    pub name: ConfigLayerName,
    pub values: serde_json::Value,
    pub source: String,
    pub version: Option<String>,
    pub config: serde_json::Value,
}

/// Metadata for a configuration layer
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigLayerMetadata {
    pub name: String,
    pub path: Option<String>,
    pub source: String,
    pub version: Option<String>,
}

/// Name of a configuration layer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigLayerName {
    Default,
    User,
    Workspace,
    Override,
    System,
    SessionFlags,
    Mdm,
}

impl std::fmt::Display for ConfigLayerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigLayerName::Default => write!(f, "default"),
            ConfigLayerName::User => write!(f, "user"),
            ConfigLayerName::Workspace => write!(f, "workspace"),
            ConfigLayerName::Override => write!(f, "override"),
            ConfigLayerName::System => write!(f, "system"),
            ConfigLayerName::SessionFlags => write!(f, "session_flags"),
            ConfigLayerName::Mdm => write!(f, "mdm"),
        }
    }
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
    /// Whether to include configuration layers in the response
    pub include_layers: bool,
}

/// Response for reading config
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigReadResponse {
    pub values: serde_json::Value,
    /// The merged configuration
    pub config: Config,
    /// Origins for each configuration value
    pub origins: Vec<ConfigLayerMetadata>,
    /// Optional list of layers (if requested)
    pub layers: Option<Vec<ConfigLayerMetadata>>,
}

/// Parameters for writing a config value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValueWriteParams {
    pub key: String,
    pub value: serde_json::Value,
    pub file_path: Option<PathBuf>,
    pub key_path: String,
    pub expected_version: Option<String>,
    pub merge_strategy: Option<MergeStrategy>,
}

/// Error code for config writes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigWriteErrorCode {
    InvalidKey,
    InvalidValue,
    PermissionDenied,
    IoError,
    ConfigValidationError,
    ConfigVersionConflict,
    ConfigPathNotFound,
    ConfigLayerReadonly,
}

/// Response for writing config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigWriteResponse {
    pub status: WriteStatus,
    pub error: Option<ConfigWriteErrorCode>,
    pub version: Option<String>,
    pub file_path: Option<std::path::PathBuf>,
    pub overridden_metadata: Option<OverriddenMetadata>,
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
    Upsert,
}

/// Metadata for overridden values
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OverriddenMetadata {
    pub original_layer: Option<ConfigLayerName>,
    pub message: Option<String>,
    pub overriding_layer: Option<ConfigLayerName>,
    pub effective_value: Option<serde_json::Value>,
}

/// Available tools configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tools {
    pub shell: bool,
    pub read_file: bool,
    pub write_file: bool,
    pub apply_patch: bool,
    pub web_search: bool,
    pub view_image: bool,
}

/// Forced login method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ForcedLoginMethod {
    #[default]
    None,
    Chatgpt,
    Api,
}

impl From<crate::protocol::config_types::ForcedLoginMethod> for ForcedLoginMethod {
    fn from(method: crate::protocol::config_types::ForcedLoginMethod) -> Self {
        match method {
            crate::protocol::config_types::ForcedLoginMethod::Chatgpt => ForcedLoginMethod::Chatgpt,
            crate::protocol::config_types::ForcedLoginMethod::Api => ForcedLoginMethod::Api,
        }
    }
}

/// User saved configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSavedConfig {
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub tools: Option<Tools>,
    pub sandbox_settings: Option<SandboxSettings>,
    pub sandbox_mode: Option<String>,
    pub profiles: Option<Vec<Profile>>,
    pub profile: Option<String>,
    pub model_verbosity: Option<String>,
    pub model_reasoning_summary: Option<bool>,
    pub model_reasoning_effort: Option<String>,
    pub forced_login_method: Option<ForcedLoginMethod>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub approval_policy: Option<String>,
}

/// Write status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteStatus {
    Success,
    Failure,
    Ok,
    OkOverridden,
}

/// User profile
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub approval_policy: Option<String>,
    pub model_reasoning_effort: Option<String>,
    pub model_reasoning_summary: Option<bool>,
    pub model_verbosity: Option<String>,
    pub chatgpt_base_url: Option<String>,
}

/// Sandbox settings
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxSettings {
    pub enabled: bool,
    pub policy: Option<String>,
    pub writable_roots: Vec<String>,
    pub network_access: bool,
    pub exclude_tmpdir_env_var: Option<String>,
    pub exclude_slash_tmp: Option<bool>,
}
