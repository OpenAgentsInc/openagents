//! Configuration loading and saving

use crate::ProjectConfig;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Configuration error types
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),

    #[error("Failed to read configuration: {0}")]
    ReadError(String),

    #[error("Failed to write configuration: {0}")]
    WriteError(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

pub type ConfigResult<T> = Result<T, ConfigError>;

/// Default configuration file path relative to project root
pub const CONFIG_PATH: &str = ".openagents/project.json";

/// Get the full path to the configuration file
pub fn config_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root.as_ref().join(CONFIG_PATH)
}

/// Load configuration from a project root directory
///
/// CONF-001: Load project configuration
/// CONF-002: Apply defaults for missing fields
///
/// # Arguments
/// * `project_root` - Path to the project root directory (NOT the .openagents directory)
///
/// # Example
/// ```no_run
/// use config::load_config;
///
/// let config = load_config("/path/to/project").unwrap();
/// println!("Project: {}", config.project_id);
/// ```
pub fn load_config(project_root: impl AsRef<Path>) -> ConfigResult<ProjectConfig> {
    let path = config_path(&project_root);

    if !path.exists() {
        return Err(ConfigError::NotFound(path));
    }

    let content = fs::read_to_string(&path).map_err(|e| ConfigError::ReadError(e.to_string()))?;

    let config: ProjectConfig =
        serde_json::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    validate_config(&config)?;

    Ok(config)
}

/// Load configuration, returning None if file doesn't exist
///
/// Useful for optional configuration scenarios.
pub fn load_config_optional(project_root: impl AsRef<Path>) -> ConfigResult<Option<ProjectConfig>> {
    match load_config(project_root) {
        Ok(config) => Ok(Some(config)),
        Err(ConfigError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Save configuration to a project root directory
///
/// CONF-003: Save project configuration
///
/// Creates the `.openagents` directory if it doesn't exist.
pub fn save_config(project_root: impl AsRef<Path>, config: &ProjectConfig) -> ConfigResult<()> {
    validate_config(config)?;

    let path = config_path(&project_root);

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ConfigError::WriteError(e.to_string()))?;
    }

    let content = serde_json::to_string_pretty(config)
        .map_err(|e| ConfigError::WriteError(format!("Serialization failed: {}", e)))?;

    fs::write(&path, content).map_err(|e| ConfigError::WriteError(e.to_string()))?;

    Ok(())
}

/// Initialize a new project configuration
///
/// CONF-004: Create new configuration with defaults
///
/// Creates a new project.json file with the given project ID and defaults.
pub fn init_config(
    project_root: impl AsRef<Path>,
    project_id: &str,
) -> ConfigResult<ProjectConfig> {
    let config = ProjectConfig::new(project_id);
    save_config(&project_root, &config)?;
    Ok(config)
}

/// Validate a configuration
///
/// CONF-005: Validate configuration values
fn validate_config(config: &ProjectConfig) -> ConfigResult<()> {
    // Project ID must not be empty
    if config.project_id.is_empty() {
        return Err(ConfigError::ValidationError(
            "Project ID cannot be empty".into(),
        ));
    }

    // Default branch must not be empty
    if config.default_branch.is_empty() {
        return Err(ConfigError::ValidationError(
            "Default branch cannot be empty".into(),
        ));
    }

    // ID prefix must not be empty
    if config.id_prefix.is_empty() {
        return Err(ConfigError::ValidationError(
            "ID prefix cannot be empty".into(),
        ));
    }

    // Max tasks must be positive
    if config.max_tasks_per_run == 0 {
        return Err(ConfigError::ValidationError(
            "Max tasks per run must be positive".into(),
        ));
    }

    // Max runtime must be positive
    if config.max_runtime_minutes == 0 {
        return Err(ConfigError::ValidationError(
            "Max runtime minutes must be positive".into(),
        ));
    }

    // Codex max turns must be positive
    if config.codex_code.max_turns_per_subtask == 0 {
        return Err(ConfigError::ValidationError(
            "Codex max turns per subtask must be positive".into(),
        ));
    }

    // Sandbox timeout must be positive
    if config.sandbox.timeout_ms == 0 {
        return Err(ConfigError::ValidationError(
            "Sandbox timeout must be positive".into(),
        ));
    }

    // Parallel execution validation
    if config.parallel_execution.enabled && config.parallel_execution.max_agents == 0 {
        return Err(ConfigError::ValidationError(
            "Max agents must be positive when parallel execution is enabled".into(),
        ));
    }

    Ok(())
}

/// Check if a project has a configuration file
pub fn has_config(project_root: impl AsRef<Path>) -> bool {
    config_path(project_root).exists()
}

/// Merge a partial configuration with defaults
///
/// Useful for CLI overrides.
pub fn merge_with_defaults(partial_json: &str, project_id: &str) -> ConfigResult<ProjectConfig> {
    // Start with defaults
    let mut config = ProjectConfig::new(project_id);

    // Parse partial JSON and merge
    let partial: serde_json::Value =
        serde_json::from_str(partial_json).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    // If partial has project_id, use it
    if let Some(id) = partial.get("projectId").and_then(|v| v.as_str()) {
        config.project_id = id.to_string();
    }

    // Re-serialize defaults, merge with partial, deserialize
    let defaults_value = serde_json::to_value(&config)
        .map_err(|e| ConfigError::ParseError(format!("Failed to serialize defaults: {}", e)))?;

    let merged = merge_json_values(defaults_value, partial);

    let config: ProjectConfig =
        serde_json::from_value(merged).map_err(|e| ConfigError::ParseError(e.to_string()))?;

    // Validate the merged config
    validate_config(&config)?;

    Ok(config)
}

/// Deep merge two JSON values (partial overrides defaults)
fn merge_json_values(defaults: serde_json::Value, partial: serde_json::Value) -> serde_json::Value {
    match (defaults, partial) {
        (serde_json::Value::Object(mut d), serde_json::Value::Object(p)) => {
            for (key, value) in p {
                let merged = if let Some(default_value) = d.remove(&key) {
                    merge_json_values(default_value, value)
                } else {
                    value
                };
                d.insert(key, merged);
            }
            serde_json::Value::Object(d)
        }
        (_, partial) => partial,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = ProjectConfig::new("test-project");

        save_config(temp_dir.path(), &config).unwrap();
        let loaded = load_config(temp_dir.path()).unwrap();

        assert_eq!(loaded.project_id, "test-project");
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn test_load_missing_config() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_config(temp_dir.path());
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }

    #[test]
    fn test_load_optional_missing() {
        let temp_dir = TempDir::new().unwrap();
        let result = load_config_optional(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_optional_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config = ProjectConfig::new("test");
        save_config(temp_dir.path(), &config).unwrap();

        let result = load_config_optional(temp_dir.path()).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_init_config() {
        let temp_dir = TempDir::new().unwrap();
        let config = init_config(temp_dir.path(), "new-project").unwrap();

        assert_eq!(config.project_id, "new-project");
        assert!(has_config(temp_dir.path()));
    }

    #[test]
    fn test_validation_empty_project_id() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ProjectConfig::new("test");
        config.project_id = "".into();

        let result = save_config(temp_dir.path(), &config);
        assert!(matches!(result, Err(ConfigError::ValidationError(_))));
    }

    #[test]
    fn test_validation_zero_max_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = ProjectConfig::new("test");
        config.max_tasks_per_run = 0;

        let result = save_config(temp_dir.path(), &config);
        assert!(matches!(result, Err(ConfigError::ValidationError(_))));
    }

    #[test]
    fn test_has_config() {
        let temp_dir = TempDir::new().unwrap();
        assert!(!has_config(temp_dir.path()));

        save_config(temp_dir.path(), &ProjectConfig::new("test")).unwrap();
        assert!(has_config(temp_dir.path()));
    }

    #[test]
    fn test_merge_with_defaults() {
        let partial = r#"{"defaultBranch": "develop", "allowPush": false}"#;
        let config = merge_with_defaults(partial, "merged-project").unwrap();

        assert_eq!(config.project_id, "merged-project");
        assert_eq!(config.default_branch, "develop");
        assert!(!config.allow_push);
        // Defaults should still be applied
        assert_eq!(config.version, 1);
        assert!(config.codex_code.enabled);
    }

    #[test]
    fn test_merge_nested_config() {
        let partial = r#"{"codexCode": {"permissionMode": "plan", "maxTurnsPerSubtask": 100}}"#;
        let config = merge_with_defaults(partial, "test").unwrap();

        assert_eq!(
            config.codex_code.permission_mode,
            crate::PermissionMode::Plan
        );
        assert_eq!(config.codex_code.max_turns_per_subtask, 100);
        // Default should still apply
        assert!(config.codex_code.enabled);
    }

    #[test]
    fn test_config_path() {
        let path = config_path("/project");
        assert_eq!(path.to_str().unwrap(), "/project/.openagents/project.json");
    }

    #[test]
    fn test_merge_with_defaults_validates_invalid_max_tasks() {
        // Attempt to override with invalid max_tasks_per_run = 0
        let partial = r#"{"maxTasksPerRun": 0}"#;
        let result = merge_with_defaults(partial, "test");

        assert!(result.is_err());
        match result {
            Err(ConfigError::ValidationError(msg)) => {
                assert!(msg.contains("Max tasks per run must be positive"));
            }
            _ => panic!("Expected ValidationError for max_tasks_per_run = 0"),
        }
    }

    #[test]
    fn test_merge_with_defaults_validates_empty_project_id() {
        // Attempt to override with empty project_id
        let partial = r#"{"projectId": ""}"#;
        let result = merge_with_defaults(partial, "test");

        assert!(result.is_err());
        match result {
            Err(ConfigError::ValidationError(msg)) => {
                assert!(msg.contains("Project ID cannot be empty"));
            }
            _ => panic!("Expected ValidationError for empty project_id"),
        }
    }

    #[test]
    fn test_merge_with_defaults_validates_empty_branch() {
        // Attempt to override with empty default branch
        let partial = r#"{"defaultBranch": ""}"#;
        let result = merge_with_defaults(partial, "test");

        assert!(result.is_err());
        match result {
            Err(ConfigError::ValidationError(msg)) => {
                assert!(msg.contains("Default branch cannot be empty"));
            }
            _ => panic!("Expected ValidationError for empty default_branch"),
        }
    }
}
