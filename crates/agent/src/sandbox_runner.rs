//! Sandbox Runner
//!
//! Provides sandboxed execution for orchestrator operations.
//! When sandbox is enabled, commands run inside a container with the workspace mounted.
//! Falls back to host execution when sandbox is disabled or unavailable.

use crate::error::{AgentError, AgentResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Sandbox configuration from project.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxConfig {
    /// Whether sandbox is enabled
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Backend to use (docker, podman, none, auto)
    #[serde(default)]
    pub backend: Option<String>,
    /// Container image to use
    #[serde(default)]
    pub image: Option<String>,
    /// Memory limit (e.g., "2g")
    #[serde(default)]
    pub memory_limit: Option<String>,
    /// CPU limit (e.g., "2")
    #[serde(default)]
    pub cpu_limit: Option<String>,
    /// Timeout in milliseconds
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Sandbox runner configuration
#[derive(Debug, Clone)]
pub struct SandboxRunnerConfig {
    /// Sandbox configuration from project.json
    pub sandbox_config: SandboxConfig,
    /// Working directory (repo root) to mount as /workspace
    pub cwd: String,
    /// Execution context for UI grouping (default: "verification")
    pub context: Option<String>,
}

impl Default for SandboxRunnerConfig {
    fn default() -> Self {
        Self {
            sandbox_config: SandboxConfig::default(),
            cwd: ".".to_string(),
            context: Some("verification".to_string()),
        }
    }
}

/// Events emitted during sandbox operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SandboxRunnerEvent {
    /// Sandbox availability check started
    SandboxCheckStart,
    /// Sandbox is available
    SandboxAvailable { backend: String },
    /// Sandbox is not available
    SandboxUnavailable { reason: String },
    /// Command execution started
    SandboxCommandStart {
        command: Vec<String>,
        in_container: bool,
    },
    /// Command execution completed
    SandboxCommandComplete {
        command: Vec<String>,
        exit_code: i32,
        duration_ms: u64,
    },
    /// Falling back to host execution
    SandboxFallback { reason: String },
}

/// Result of running a command
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Whether the command ran in a container or on host
    pub sandboxed: bool,
}

/// Container configuration
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Container image
    pub image: String,
    /// Host directory to mount as workspace
    pub workspace_dir: String,
    /// Working directory inside container
    pub workdir: String,
    /// Memory limit
    pub memory_limit: Option<String>,
    /// CPU limit
    pub cpu_limit: Option<String>,
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
    /// Volume mounts
    pub volume_mounts: Option<Vec<String>>,
    /// Auto-remove container after exit
    pub auto_remove: bool,
}

/// Default sandbox image
pub const DEFAULT_SANDBOX_IMAGE: &str = "oven/bun:latest";

// ─────────────────────────────────────────────────────────────────────────────
// Sandbox Availability Check
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a container backend (docker/podman) is available
pub fn is_container_available() -> bool {
    // Try docker first
    if Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return true;
    }

    // Try podman
    Command::new("podman")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get the available container backend
pub fn get_container_backend() -> Option<String> {
    if Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some("docker".to_string());
    }

    if Command::new("podman")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some("podman".to_string());
    }

    None
}

/// Check if sandbox execution is available
pub fn check_sandbox_available(config: &SandboxConfig) -> (bool, Option<SandboxRunnerEvent>) {
    // Check if explicitly disabled
    if config.enabled == Some(false) {
        return (
            false,
            Some(SandboxRunnerEvent::SandboxUnavailable {
                reason: "sandbox.enabled is false".to_string(),
            }),
        );
    }

    // Check if backend is explicitly "none"
    if config.backend.as_deref() == Some("none") {
        return (
            false,
            Some(SandboxRunnerEvent::SandboxUnavailable {
                reason: "sandbox.backend is 'none'".to_string(),
            }),
        );
    }

    // Check if container backend is available
    if is_container_available() {
        let backend = config
            .backend
            .clone()
            .unwrap_or_else(|| "auto".to_string());
        (
            true,
            Some(SandboxRunnerEvent::SandboxAvailable { backend }),
        )
    } else {
        (
            false,
            Some(SandboxRunnerEvent::SandboxUnavailable {
                reason: "no container backend available".to_string(),
            }),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Container Config Builder
// ─────────────────────────────────────────────────────────────────────────────

/// Build ContainerConfig from SandboxConfig and working directory
pub fn build_container_config(
    sandbox_config: &SandboxConfig,
    cwd: &str,
    env: Option<HashMap<String, String>>,
    volume_mounts: Option<Vec<String>>,
) -> ContainerConfig {
    ContainerConfig {
        image: sandbox_config
            .image
            .clone()
            .unwrap_or_else(|| DEFAULT_SANDBOX_IMAGE.to_string()),
        workspace_dir: cwd.to_string(),
        workdir: "/workspace".to_string(),
        memory_limit: sandbox_config.memory_limit.clone(),
        cpu_limit: sandbox_config.cpu_limit.clone(),
        timeout_ms: sandbox_config.timeout_ms,
        env,
        volume_mounts,
        auto_remove: true,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Host Execution
// ─────────────────────────────────────────────────────────────────────────────

/// Run a command on the host (no sandbox)
pub fn run_on_host(
    command: &[String],
    cwd: &str,
    timeout_ms: Option<u64>,
    env: Option<&HashMap<String, String>>,
) -> AgentResult<CommandResult> {
    // Validate working directory exists
    if !Path::new(cwd).exists() {
        return Err(AgentError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Working directory does not exist: {}", cwd),
        )));
    }

    let cmd_string = command.join(" ");
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(120_000));
    let start = Instant::now();

    let mut cmd = Command::new("sh");
    cmd.args(["-c", &cmd_string])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Add environment variables
    if let Some(env_vars) = env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    let output = cmd.output().map_err(AgentError::Io)?;

    let elapsed = start.elapsed();
    if elapsed > timeout {
        return Err(AgentError::Timeout(format!(
            "Host command timed out after {}ms",
            timeout.as_millis()
        )));
    }

    Ok(CommandResult {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        sandboxed: false,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Sandboxed Execution
// ─────────────────────────────────────────────────────────────────────────────

/// Run a command in a sandbox container
pub fn run_in_container(
    command: &[String],
    config: &ContainerConfig,
) -> AgentResult<CommandResult> {
    let backend = get_container_backend().ok_or_else(|| {
        AgentError::ContainerError("No container backend (docker/podman) available".to_string())
    })?;

    let mut args = vec!["run".to_string(), "--rm".to_string()];

    // Add workdir
    args.push("-w".to_string());
    args.push(config.workdir.clone());

    // Mount workspace
    args.push("-v".to_string());
    args.push(format!("{}:{}", config.workspace_dir, config.workdir));

    // Add memory limit
    if let Some(ref limit) = config.memory_limit {
        args.push("-m".to_string());
        args.push(limit.clone());
    }

    // Add CPU limit
    if let Some(ref limit) = config.cpu_limit {
        args.push("--cpus".to_string());
        args.push(limit.clone());
    }

    // Add environment variables
    if let Some(ref env) = config.env {
        for (key, value) in env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }
    }

    // Add volume mounts
    if let Some(ref mounts) = config.volume_mounts {
        for mount in mounts {
            args.push("-v".to_string());
            args.push(mount.clone());
        }
    }

    // Add image
    args.push(config.image.clone());

    // Add command
    args.push("sh".to_string());
    args.push("-c".to_string());
    args.push(command.join(" "));

    let timeout = Duration::from_millis(config.timeout_ms.unwrap_or(120_000));
    let start = Instant::now();

    let output = Command::new(&backend)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(AgentError::Io)?;

    let elapsed = start.elapsed();
    if elapsed > timeout {
        return Err(AgentError::Timeout(format!(
            "Container command timed out after {}ms",
            timeout.as_millis()
        )));
    }

    Ok(CommandResult {
        exit_code: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        sandboxed: true,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Runner API
// ─────────────────────────────────────────────────────────────────────────────

/// Run a shell command, using sandbox if available and enabled
pub fn run_command(
    command: &[String],
    config: &SandboxRunnerConfig,
    env: Option<&HashMap<String, String>>,
) -> AgentResult<CommandResult> {
    // Validate working directory exists
    if !Path::new(&config.cwd).exists() {
        return Err(AgentError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "Working directory does not exist: {}. The worktree may have been corrupted or removed during execution.",
                config.cwd
            ),
        )));
    }

    let (sandbox_available, _event) = check_sandbox_available(&config.sandbox_config);

    if sandbox_available {
        // Try container execution with fallback to host
        let container_config = build_container_config(
            &config.sandbox_config,
            &config.cwd,
            env.cloned(),
            None,
        );

        match run_in_container(command, &container_config) {
            Ok(result) => Ok(result),
            Err(_) => {
                // Fallback to host
                run_on_host(
                    command,
                    &config.cwd,
                    config.sandbox_config.timeout_ms,
                    env,
                )
            }
        }
    } else {
        run_on_host(
            command,
            &config.cwd,
            config.sandbox_config.timeout_ms,
            env,
        )
    }
}

/// Run a shell command string, using sandbox if available
pub fn run_command_string(
    command_string: &str,
    config: &SandboxRunnerConfig,
    env: Option<&HashMap<String, String>>,
) -> AgentResult<CommandResult> {
    let command: Vec<String> = command_string
        .trim()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    run_command(&command, config, env)
}

// ─────────────────────────────────────────────────────────────────────────────
// Verification Command Runner
// ─────────────────────────────────────────────────────────────────────────────

/// Result of running verification commands
#[derive(Debug, Clone)]
pub struct SandboxVerificationResult {
    /// Whether all verification commands passed
    pub passed: bool,
    /// Output from each verification command
    pub outputs: Vec<String>,
    /// Whether any command ran in a container
    pub sandboxed: bool,
}

/// Run verification commands (typecheck, tests) with sandbox support
pub fn run_verification_with_sandbox(
    commands: &[String],
    config: &SandboxRunnerConfig,
) -> AgentResult<SandboxVerificationResult> {
    let mut outputs = Vec::new();
    let mut all_passed = true;
    let mut any_sandboxed = false;

    for cmd in commands {
        let result = run_command_string(cmd, config, None)?;
        let output = if result.stderr.is_empty() {
            result.stdout.clone()
        } else {
            format!("{}\n{}", result.stdout, result.stderr)
        };
        outputs.push(output);

        if result.sandboxed {
            any_sandboxed = true;
        }

        if result.exit_code != 0 {
            all_passed = false;
        }
    }

    Ok(SandboxVerificationResult {
        passed: all_passed,
        outputs,
        sandboxed: any_sandboxed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enabled.is_none());
        assert!(config.backend.is_none());
        assert!(config.image.is_none());
    }

    #[test]
    fn test_check_sandbox_available_disabled() {
        let config = SandboxConfig {
            enabled: Some(false),
            ..Default::default()
        };
        let (available, event) = check_sandbox_available(&config);
        assert!(!available);
        assert!(matches!(
            event,
            Some(SandboxRunnerEvent::SandboxUnavailable { .. })
        ));
    }

    #[test]
    fn test_check_sandbox_available_backend_none() {
        let config = SandboxConfig {
            backend: Some("none".to_string()),
            ..Default::default()
        };
        let (available, event) = check_sandbox_available(&config);
        assert!(!available);
        assert!(matches!(
            event,
            Some(SandboxRunnerEvent::SandboxUnavailable { reason })
            if reason.contains("none")
        ));
    }

    #[test]
    fn test_build_container_config() {
        let sandbox = SandboxConfig {
            image: Some("test:latest".to_string()),
            memory_limit: Some("1g".to_string()),
            ..Default::default()
        };
        let config = build_container_config(&sandbox, "/workspace", None, None);

        assert_eq!(config.image, "test:latest");
        assert_eq!(config.workspace_dir, "/workspace");
        assert_eq!(config.workdir, "/workspace");
        assert_eq!(config.memory_limit, Some("1g".to_string()));
        assert!(config.auto_remove);
    }

    #[test]
    fn test_build_container_config_defaults() {
        let sandbox = SandboxConfig::default();
        let config = build_container_config(&sandbox, "/work", None, None);

        assert_eq!(config.image, DEFAULT_SANDBOX_IMAGE);
    }

    #[test]
    fn test_command_result_fields() {
        let result = CommandResult {
            exit_code: 0,
            stdout: "output".to_string(),
            stderr: "".to_string(),
            sandboxed: true,
        };

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "output");
        assert!(result.sandboxed);
    }

    #[test]
    fn test_sandbox_runner_event_serialization() {
        let event = SandboxRunnerEvent::SandboxAvailable {
            backend: "docker".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("sandbox_available"));
        assert!(json.contains("docker"));
    }

    #[test]
    fn test_run_on_host_missing_dir() {
        let result = run_on_host(
            &["echo".to_string(), "hello".to_string()],
            "/nonexistent/path",
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_run_on_host_simple() {
        let result = run_on_host(
            &["echo".to_string(), "hello".to_string()],
            ".",
            None,
            None,
        );
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout.contains("hello"));
        assert!(!res.sandboxed);
    }

    #[test]
    fn test_run_command_string() {
        let config = SandboxRunnerConfig {
            sandbox_config: SandboxConfig {
                enabled: Some(false),
                ..Default::default()
            },
            cwd: ".".to_string(),
            context: None,
        };

        let result = run_command_string("echo test", &config, None);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout.contains("test"));
    }

    #[test]
    fn test_sandbox_verification_result() {
        let result = SandboxVerificationResult {
            passed: true,
            outputs: vec!["ok".to_string()],
            sandboxed: false,
        };
        assert!(result.passed);
        assert_eq!(result.outputs.len(), 1);
    }

    #[test]
    fn test_run_verification_with_sandbox() {
        let config = SandboxRunnerConfig {
            sandbox_config: SandboxConfig {
                enabled: Some(false),
                ..Default::default()
            },
            cwd: ".".to_string(),
            context: None,
        };

        let commands = vec!["echo ok".to_string()];
        let result = run_verification_with_sandbox(&commands, &config);
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(res.passed);
        assert!(!res.sandboxed);
    }
}
