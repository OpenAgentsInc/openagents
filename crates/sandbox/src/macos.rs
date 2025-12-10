//! macOS Container backend (for macOS 26+)

use crate::{
    backend::{BuildOptions, ContainerBackend},
    config::{ContainerConfig, ContainerRunResult},
    error::{ContainerError, ContainerResult},
};
use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const CONTAINER_CLI: &str = "container";
const DEFAULT_TIMEOUT_MS: u64 = 300_000; // 5 minutes
const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// macOS Container backend using the `container` CLI
pub struct MacOSContainerBackend;

impl MacOSContainerBackend {
    /// Create a new macOS container backend
    pub fn new() -> Self {
        Self
    }

    /// Check if macOS and container CLI are available
    async fn check_available() -> bool {
        // Check if we're on macOS
        if std::env::consts::OS != "macos" {
            return false;
        }

        // Check if `container` CLI exists
        let which_result = Command::new("which")
            .arg(CONTAINER_CLI)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        if !matches!(which_result, Ok(status) if status.success()) {
            return false;
        }

        // Check if container system is running
        let status_result = Command::new(CONTAINER_CLI)
            .args(["system", "status"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        matches!(status_result, Ok(status) if status.success())
    }
}

impl Default for MacOSContainerBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContainerBackend for MacOSContainerBackend {
    fn name(&self) -> &'static str {
        "macos-container"
    }

    async fn is_available(&self) -> bool {
        Self::check_available().await
    }

    async fn run(
        &self,
        command: &[String],
        config: &ContainerConfig,
    ) -> ContainerResult<ContainerRunResult> {
        let mut args = vec!["run".to_string()];

        // Auto-remove
        if config.auto_remove {
            args.push("--rm".to_string());
        }

        // Workspace volume
        args.push("-v".to_string());
        args.push(format!("{}:/workspace", config.workspace_dir.display()));

        // Additional volume mounts
        for mount in &config.volume_mounts {
            args.push("-v".to_string());
            args.push(mount.clone());
        }

        // Working directory
        args.push("-w".to_string());
        args.push(config.workdir.clone().unwrap_or_else(|| "/workspace".to_string()));

        // Resource limits
        if let Some(ref limit) = config.memory_limit {
            args.push("--memory".to_string());
            args.push(limit.clone());
        }
        if let Some(cpus) = config.cpu_limit {
            args.push("--cpus".to_string());
            args.push(cpus.to_string());
        }

        // Environment variables
        for (key, value) in &config.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Image and command
        args.push(config.image.clone());
        args.extend(command.iter().cloned());

        // Spawn container process
        let mut child = Command::new(CONTAINER_CLI)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                ContainerError::start_failed(format!("Failed to start container: {}", e))
            })?;

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Read stdout with size limit
        let stdout_task = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(stdout) = stdout_handle {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    if output.len() < MAX_OUTPUT_SIZE {
                        let remaining = MAX_OUTPUT_SIZE - output.len();
                        if line.len() <= remaining {
                            output.push_str(&line);
                        } else {
                            output.push_str(&line[..remaining]);
                        }
                    }
                    line.clear();
                }
            }
            output
        });

        // Read stderr with size limit
        let stderr_task = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(stderr) = stderr_handle {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    if output.len() < MAX_OUTPUT_SIZE {
                        let remaining = MAX_OUTPUT_SIZE - output.len();
                        if line.len() <= remaining {
                            output.push_str(&line);
                        } else {
                            output.push_str(&line[..remaining]);
                        }
                    }
                    line.clear();
                }
            }
            output
        });

        // Wait with timeout
        let timeout_ms = config
            .timeout
            .map(|d| d.as_millis() as u64)
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        let timeout = Duration::from_millis(timeout_ms);

        let result = tokio::time::timeout(timeout, child.wait()).await;

        match result {
            Ok(Ok(status)) => {
                let stdout = stdout_task.await.unwrap_or_default();
                let stderr = stderr_task.await.unwrap_or_default();

                Ok(ContainerRunResult {
                    exit_code: status.code().unwrap_or(1),
                    stdout,
                    stderr,
                    container_id: None,
                })
            }
            Ok(Err(e)) => Err(ContainerError::execution_failed(
                format!("Container process error: {}", e),
                None,
            )),
            Err(_) => {
                // Timeout - the child will be killed when dropped
                Err(ContainerError::timeout(format!(
                    "Container execution timed out after {}ms",
                    timeout_ms
                )))
            }
        }
    }

    async fn build(
        &self,
        context_dir: &Path,
        tag: &str,
        options: Option<BuildOptions>,
    ) -> ContainerResult<()> {
        let mut args = vec!["build".to_string(), "-t".to_string(), tag.to_string()];

        if let Some(ref opts) = options {
            if let Some(ref file) = opts.file {
                args.push("-f".to_string());
                args.push(file.clone());
            }
            if let Some(ref limit) = opts.memory_limit {
                args.push("--memory".to_string());
                args.push(limit.clone());
            }
            if let Some(cpus) = opts.cpu_limit {
                args.push("--cpus".to_string());
                args.push(cpus.to_string());
            }
        }

        args.push(context_dir.display().to_string());

        let output = Command::new(CONTAINER_CLI)
            .args(&args)
            .output()
            .await
            .map_err(|e| {
                ContainerError::start_failed(format!("Failed to start container build: {}", e))
            })?;

        if !output.status.success() {
            return Err(ContainerError::execution_failed(
                format!("Build failed: {}", String::from_utf8_lossy(&output.stderr)),
                output.status.code(),
            ));
        }

        Ok(())
    }
}
