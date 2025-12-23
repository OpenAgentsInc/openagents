//! Daemon configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How the worker should be invoked
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerCommand {
    /// Use cargo to run: `cargo autopilot run ...`
    Cargo {
        /// Optional path to Cargo.toml
        manifest_path: Option<PathBuf>,
    },
    /// Use compiled binary: `/path/to/autopilot run ...`
    Binary {
        /// Path to autopilot binary
        path: PathBuf,
    },
}

impl Default for WorkerCommand {
    fn default() -> Self {
        WorkerCommand::Cargo { manifest_path: None }
    }
}

/// Memory monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Minimum available memory before intervention (bytes)
    /// Default: 2GB
    pub min_available_bytes: u64,

    /// Memory threshold to force worker restart (bytes)
    /// Default: 1GB
    pub critical_threshold_bytes: u64,

    /// Polling interval (milliseconds)
    /// Default: 5000
    pub poll_interval_ms: u64,

    /// Kill node processes using more than this (bytes)
    /// Default: 500MB
    pub node_kill_threshold_bytes: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            min_available_bytes: 2 * 1024 * 1024 * 1024,        // 2 GB
            critical_threshold_bytes: 1024 * 1024 * 1024,   // 1 GB
            poll_interval_ms: 5000,                              // 5 seconds
            node_kill_threshold_bytes: 500 * 1024 * 1024,        // 500 MB
        }
    }
}

/// Restart policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartConfig {
    /// Initial backoff delay (milliseconds)
    /// Default: 1000
    pub initial_backoff_ms: u64,

    /// Maximum backoff delay (milliseconds)
    /// Default: 300000 (5 min)
    pub max_backoff_ms: u64,

    /// Backoff multiplier
    /// Default: 2.0
    pub backoff_multiplier: f64,

    /// Reset backoff after successful run of this duration (ms)
    /// Default: 60000 (1 min)
    pub success_threshold_ms: u64,

    /// Maximum consecutive restarts before giving up
    /// Default: 10
    pub max_consecutive_restarts: u32,

    /// Stall timeout - restart worker if no log activity for this long (ms)
    /// Default: 300000 (5 min)
    pub stall_timeout_ms: u64,

    /// Recovery cooldown - after hitting max consecutive restarts, wait this long
    /// before resetting failure counter and trying again (ms)
    /// Default: 600000 (10 min)
    pub recovery_cooldown_ms: u64,
}

impl Default for RestartConfig {
    fn default() -> Self {
        // Allow environment variable overrides for key timeout values
        let stall_timeout_ms = std::env::var("AUTOPILOT_STALL_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300_000); // 5 minutes

        let recovery_cooldown_ms = std::env::var("AUTOPILOT_RECOVERY_COOLDOWN_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(600_000); // 10 minutes

        Self {
            initial_backoff_ms: 1000,
            max_backoff_ms: 300_000,
            backoff_multiplier: 2.0,
            success_threshold_ms: 60_000,
            max_consecutive_restarts: 10,
            stall_timeout_ms,
            recovery_cooldown_ms,
        }
    }
}

/// Main daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// How to invoke the worker
    pub worker_command: WorkerCommand,

    /// Working directory for the worker
    pub working_dir: PathBuf,

    /// Project name to run (--project flag)
    pub project: Option<String>,

    /// Memory monitoring configuration
    pub memory: MemoryConfig,

    /// Restart policy
    pub restart: RestartConfig,

    /// Control socket path
    pub socket_path: PathBuf,

    /// PID file path
    pub pid_file: PathBuf,

    /// Model to use (sonnet, opus, haiku)
    pub model: String,

    /// Maximum budget in USD
    pub max_budget: f64,

    /// Maximum turns
    pub max_turns: u32,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let autopilot_dir = PathBuf::from(&home).join(".autopilot");

        Self {
            worker_command: WorkerCommand::default(),
            working_dir: PathBuf::from("."),
            project: None,
            memory: MemoryConfig::default(),
            restart: RestartConfig::default(),
            socket_path: autopilot_dir.join("autopilotd.sock"),
            pid_file: autopilot_dir.join("autopilotd.pid"),
            model: "sonnet".to_string(),
            max_budget: 0.0, // 0 = no constraint
            max_turns: 99999,
        }
    }
}

impl DaemonConfig {
    /// Load configuration from a TOML file
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: DaemonConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration with defaults, overriding from file if it exists
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let config_path = PathBuf::from(&home).join(".autopilot").join("daemon.toml");

        if config_path.exists() {
            match Self::load_from_file(&config_path) {
                Ok(config) => return config,
                Err(e) => {
                    eprintln!("Warning: Failed to load config from {:?}: {}", config_path, e);
                }
            }
        }

        Self::default()
    }

    /// Ensure the autopilot directory exists
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.pid_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stall_timeout_env_var() {
        unsafe {
            std::env::set_var("AUTOPILOT_STALL_TIMEOUT_MS", "120000");
        }
        let config = RestartConfig::default();
        assert_eq!(config.stall_timeout_ms, 120000);
        unsafe {
            std::env::remove_var("AUTOPILOT_STALL_TIMEOUT_MS");
        }
    }

    #[test]
    fn test_recovery_cooldown_env_var() {
        unsafe {
            std::env::set_var("AUTOPILOT_RECOVERY_COOLDOWN_MS", "900000");
        }
        let config = RestartConfig::default();
        assert_eq!(config.recovery_cooldown_ms, 900000);
        unsafe {
            std::env::remove_var("AUTOPILOT_RECOVERY_COOLDOWN_MS");
        }
    }

    #[test]
    fn test_default_timeouts_without_env() {
        unsafe {
            std::env::remove_var("AUTOPILOT_STALL_TIMEOUT_MS");
            std::env::remove_var("AUTOPILOT_RECOVERY_COOLDOWN_MS");
        }
        let config = RestartConfig::default();
        assert_eq!(config.stall_timeout_ms, 300_000);
        assert_eq!(config.recovery_cooldown_ms, 600_000);
    }

    #[test]
    fn test_invalid_env_var_uses_default() {
        unsafe {
            std::env::set_var("AUTOPILOT_STALL_TIMEOUT_MS", "not_a_number");
        }
        let config = RestartConfig::default();
        assert_eq!(config.stall_timeout_ms, 300_000);
        unsafe {
            std::env::remove_var("AUTOPILOT_STALL_TIMEOUT_MS");
        }
    }
}
