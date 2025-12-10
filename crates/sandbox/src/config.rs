//! Container configuration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Configuration for running a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Image to run (e.g., "mechacoder:latest" or "oven/bun:latest")
    pub image: String,

    /// Host directory to mount as /workspace inside container
    pub workspace_dir: PathBuf,

    /// Working directory inside container (default: /workspace)
    #[serde(default)]
    pub workdir: Option<String>,

    /// Memory limit with optional suffix K/M/G (e.g., "4G")
    #[serde(default)]
    pub memory_limit: Option<String>,

    /// Number of CPUs to allocate
    #[serde(default)]
    pub cpu_limit: Option<f32>,

    /// Environment variables to set
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Timeout for the entire operation
    #[serde(default, with = "optional_duration_serde")]
    pub timeout: Option<Duration>,

    /// Remove container after it exits (default: true)
    #[serde(default = "default_true")]
    pub auto_remove: bool,

    /// Additional volume mounts (e.g., ["/tmp/creds:/root/.claude:ro"])
    #[serde(default)]
    pub volume_mounts: Vec<String>,
}

fn default_true() -> bool {
    true
}

mod optional_duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(duration) => duration.as_millis().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let ms: Option<u64> = Option::deserialize(deserializer)?;
        Ok(ms.map(Duration::from_millis))
    }
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            image: String::new(),
            workspace_dir: PathBuf::new(),
            workdir: None,
            memory_limit: None,
            cpu_limit: None,
            env: HashMap::new(),
            timeout: None,
            auto_remove: true,
            volume_mounts: Vec::new(),
        }
    }
}

impl ContainerConfig {
    /// Create a new config with image and workspace
    pub fn new(image: impl Into<String>, workspace_dir: impl Into<PathBuf>) -> Self {
        Self {
            image: image.into(),
            workspace_dir: workspace_dir.into(),
            ..Default::default()
        }
    }

    /// Set the working directory inside the container
    pub fn workdir(mut self, dir: impl Into<String>) -> Self {
        self.workdir = Some(dir.into());
        self
    }

    /// Set memory limit (e.g., "4G", "512M")
    pub fn memory_limit(mut self, limit: impl Into<String>) -> Self {
        self.memory_limit = Some(limit.into());
        self
    }

    /// Set CPU limit
    pub fn cpu_limit(mut self, cpus: f32) -> Self {
        self.cpu_limit = Some(cpus);
        self
    }

    /// Add an environment variable
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set environment variables from a HashMap
    pub fn envs(mut self, envs: HashMap<String, String>) -> Self {
        self.env.extend(envs);
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set timeout in milliseconds
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout = Some(Duration::from_millis(ms));
        self
    }

    /// Set auto-remove flag
    pub fn auto_remove(mut self, remove: bool) -> Self {
        self.auto_remove = remove;
        self
    }

    /// Add a volume mount
    pub fn volume_mount(mut self, mount: impl Into<String>) -> Self {
        self.volume_mounts.push(mount.into());
        self
    }

    /// Add multiple volume mounts
    pub fn volume_mounts(mut self, mounts: Vec<String>) -> Self {
        self.volume_mounts.extend(mounts);
        self
    }
}

/// Result of running a command in a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRunResult {
    /// Exit code from the command
    pub exit_code: i32,

    /// Standard output
    pub stdout: String,

    /// Standard error
    pub stderr: String,

    /// Container ID (useful for debugging)
    #[serde(default)]
    pub container_id: Option<String>,
}

impl ContainerRunResult {
    /// Create a new run result
    pub fn new(exit_code: i32, stdout: String, stderr: String) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
            container_id: None,
        }
    }

    /// Set the container ID
    pub fn with_container_id(mut self, id: impl Into<String>) -> Self {
        self.container_id = Some(id.into());
        self
    }

    /// Check if the command succeeded (exit code 0)
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    /// Get combined output (stdout + stderr)
    pub fn combined_output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Credential mount information for container use
#[derive(Debug, Clone)]
pub struct CredentialMount {
    /// Host directory (e.g., /tmp/mechacoder-creds-abc123)
    pub host_dir: PathBuf,

    /// Host file path (e.g., /tmp/mechacoder-creds-abc123/.credentials.json)
    pub host_file_path: PathBuf,

    /// Container directory (/root/.claude)
    pub container_dir: String,

    /// Volume mount string (host_dir:container_dir:ro)
    pub volume_mount: String,
}

impl CredentialMount {
    /// Create a new credential mount
    pub fn new(host_dir: PathBuf, host_file_path: PathBuf, container_dir: String) -> Self {
        let volume_mount = format!("{}:{}:ro", host_dir.display(), container_dir);
        Self {
            host_dir,
            host_file_path,
            container_dir,
            volume_mount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_config_builder() {
        let config = ContainerConfig::new("ubuntu:latest", "/home/user/project")
            .workdir("/app")
            .memory_limit("4G")
            .cpu_limit(2.0)
            .env("FOO", "bar")
            .timeout_ms(60000)
            .volume_mount("/tmp/creds:/root/.claude:ro");

        assert_eq!(config.image, "ubuntu:latest");
        assert_eq!(config.workdir, Some("/app".to_string()));
        assert_eq!(config.memory_limit, Some("4G".to_string()));
        assert_eq!(config.cpu_limit, Some(2.0));
        assert_eq!(config.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.timeout, Some(Duration::from_millis(60000)));
        assert_eq!(config.volume_mounts.len(), 1);
    }

    #[test]
    fn test_container_run_result() {
        let result = ContainerRunResult::new(0, "hello".to_string(), "".to_string())
            .with_container_id("abc123");

        assert!(result.success());
        assert_eq!(result.container_id, Some("abc123".to_string()));
        assert_eq!(result.combined_output(), "hello");
    }
}
