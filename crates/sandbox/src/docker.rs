//! Docker container backend

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
use uuid::Uuid;

const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Docker container backend
pub struct DockerBackend;

impl DockerBackend {
    /// Create a new Docker backend
    pub fn new() -> Self {
        Self
    }

    /// Check if docker command is available
    async fn has_docker_command() -> bool {
        // Check environment override
        if std::env::var("OPENAGENTS_DOCKER_AVAILABLE").as_deref() == Ok("0") {
            return false;
        }
        if std::env::var("OPENAGENTS_DOCKER_AVAILABLE").as_deref() == Ok("1") {
            return true;
        }

        // Try to run docker --version
        match Command::new("docker")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
        {
            Ok(status) => status.success(),
            Err(_) => false,
        }
    }

    /// Build docker run arguments
    fn build_docker_args(
        command: &[String],
        config: &ContainerConfig,
        container_name: &str,
    ) -> Vec<String> {
        let mut args = vec![
            "run".to_string(),
            "-i".to_string(),
            "--name".to_string(),
            container_name.to_string(),
        ];

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

        // Memory limit
        if let Some(ref limit) = config.memory_limit {
            args.push("--memory".to_string());
            args.push(limit.clone());
        }

        // CPU limit
        if let Some(cpus) = config.cpu_limit {
            args.push("--cpus".to_string());
            args.push(cpus.to_string());
        }

        // Environment variables
        for (key, value) in &config.env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Image
        args.push(config.image.clone());

        // Command
        args.extend(command.iter().cloned());

        args
    }
}

impl Default for DockerBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ContainerBackend for DockerBackend {
    fn name(&self) -> &'static str {
        "docker"
    }

    async fn is_available(&self) -> bool {
        Self::has_docker_command().await
    }

    async fn run(
        &self,
        command: &[String],
        config: &ContainerConfig,
    ) -> ContainerResult<ContainerRunResult> {
        // Verify workspace exists
        if !config.workspace_dir.exists() {
            return Err(ContainerError::start_failed(format!(
                "workspaceDir does not exist: {}",
                config.workspace_dir.display()
            )));
        }

        let container_name = format!("oa-sbx-{}", Uuid::new_v4());
        let args = Self::build_docker_args(command, config, &container_name);

        // Spawn docker process
        let mut child = Command::new("docker")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ContainerError::start_failed(format!("Failed to start docker: {}", e)))?;

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Read stdout
        let stdout_task = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(stdout) = stdout_handle {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    output.push_str(&line);
                    line.clear();
                }
            }
            output
        });

        // Read stderr
        let stderr_task = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(stderr) = stderr_handle {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    output.push_str(&line);
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
                    container_id: Some(container_name),
                })
            }
            Ok(Err(e)) => Err(ContainerError::execution_failed(
                format!("Docker process error: {}", e),
                None,
            )),
            Err(_) => {
                // Timeout - kill the container
                let _ = Command::new("docker")
                    .args(["kill", &container_name])
                    .output()
                    .await;

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

        let output = Command::new("docker")
            .args(&args)
            .output()
            .await
            .map_err(|e| ContainerError::start_failed(format!("Failed to start docker build: {}", e)))?;

        if !output.status.success() {
            return Err(ContainerError::execution_failed(
                format!(
                    "docker build exited with code {}: {}",
                    output.status.code().unwrap_or(1),
                    String::from_utf8_lossy(&output.stderr)
                ),
                output.status.code(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_docker_args() {
        let config = ContainerConfig::new("ubuntu:latest", "/tmp/workspace")
            .workdir("/app")
            .memory_limit("4G")
            .cpu_limit(2.0)
            .env("FOO", "bar")
            .volume_mount("/tmp/creds:/root/.claude:ro");

        let args = DockerBackend::build_docker_args(
            &["bash".to_string(), "-c".to_string(), "echo hello".to_string()],
            &config,
            "test-container",
        );

        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--rm".to_string()));
        assert!(args.contains(&"-v".to_string()));
        assert!(args.contains(&"--memory".to_string()));
        assert!(args.contains(&"4G".to_string()));
        assert!(args.contains(&"--cpus".to_string()));
        assert!(args.contains(&"2".to_string()));
        assert!(args.contains(&"FOO=bar".to_string()));
        assert!(args.contains(&"ubuntu:latest".to_string()));
    }
}
