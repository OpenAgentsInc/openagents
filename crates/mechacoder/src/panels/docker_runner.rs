//! Docker Runner - Executes Claude Code in TB2 containers via SDK
//!
//! This module uses the claude-agent-sdk to run Claude Code on the host,
//! with the workspace mounted to TB2 Docker containers for verification.

use crate::panels::testgen_wrapper::TestGenWrapper;
use crate::panels::verifier::VerificationResult;
use claude_agent_sdk::{
    query, AllowAllPermissions, ExecutableConfig, QueryOptions, SdkMessage,
};
use futures::StreamExt;
use sandbox::{cleanup_credential_mount, create_credential_mount, ContainerBackend, ContainerConfig, DockerBackend};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use terminalbench::TB2Task;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Allowed tools for Claude Code in TB2 environment (matching Harbor)
pub const ALLOWED_TOOLS: &[&str] = &[
    "Bash",
    "Edit",
    "Write",
    "Read",
    "Glob",
    "Grep",
    "LS",
    "WebFetch",
    "NotebookEdit",
    "NotebookRead",
    "TodoRead",
    "TodoWrite",
    "Agent",
];

/// Configuration for a TB2 Docker run
#[derive(Debug, Clone)]
pub struct DockerRunConfig {
    /// TB2 task being run
    pub task: TB2Task,
    /// Working directory on host (temp dir that maps to /app)
    pub workspace_dir: PathBuf,
    /// Logs directory on host (maps to /logs)
    pub logs_dir: PathBuf,
    /// Max turns for Claude
    pub max_turns: u32,
    /// Model to use (if overriding default)
    pub model: Option<String>,
}

impl DockerRunConfig {
    /// Create a new run config
    pub fn new(task: TB2Task, workspace_dir: PathBuf, logs_dir: PathBuf) -> Self {
        Self {
            task,
            workspace_dir,
            logs_dir,
            max_turns: 300, // Default max turns
            model: None,
        }
    }

    /// Set max turns
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set model override
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// Events streamed from Docker container execution
#[derive(Debug, Clone)]
pub enum DockerEvent {
    /// Container is starting
    ContainerStarting { image: String },
    /// Container started successfully
    ContainerStarted { container_id: String },
    /// Raw output line from Claude
    ClaudeOutput { line: String },
    /// Parsed tool use from Claude
    ToolUse {
        tool_name: String,
        tool_id: String,
    },
    /// Assistant message content
    AssistantMessage { text: String, turn: u32 },
    /// Turn completed
    TurnComplete { turn: u32 },
    /// Container stopped
    ContainerStopped { exit_code: i32 },
    /// Error occurred
    Error { message: String },
    /// Run completed with verification results
    RunComplete {
        run_result: Option<DockerRunResult>,
        run_error: Option<String>,
        verification: Option<VerificationResult>,
    },
}

/// Result from a Docker run
#[derive(Debug, Clone)]
pub struct DockerRunResult {
    pub success: bool,
    pub exit_code: i32,
    pub turns: u32,
    pub cost_usd: f64,
    pub session_dir: Option<PathBuf>,
    pub error: Option<String>,
}

impl Default for DockerRunResult {
    fn default() -> Self {
        Self {
            success: false,
            exit_code: -1,
            turns: 0,
            cost_usd: 0.0,
            session_dir: None,
            error: None,
        }
    }
}

/// Errors from Docker runner
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker not available")]
    NotAvailable,

    #[error("Image not found: {0}")]
    ImageNotFound(String),

    #[error("Failed to start container: {0}")]
    StartFailed(String),

    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Credential error: {0}")]
    CredentialError(String),
}

/// Docker runner for TB2 tasks
pub struct DockerRunner {
    backend: DockerBackend,
}

impl DockerRunner {
    /// Create a new Docker runner
    pub fn new() -> Self {
        Self {
            backend: DockerBackend::new(),
        }
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        self.backend.is_available().await
    }

    /// Pull or verify the Docker image exists
    pub async fn ensure_image(&self, image: &str) -> Result<(), DockerError> {
        tracing::info!(
            target: "mechacoder::docker",
            image,
            "Checking Docker image"
        );

        // Check if image exists locally
        let check = Command::new("docker")
            .args(["image", "inspect", image])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        if check.success() {
            tracing::debug!(
                target: "mechacoder::docker",
                image,
                "Image exists locally"
            );
            return Ok(());
        }

        // Try to pull the image
        tracing::info!(
            target: "mechacoder::docker",
            image,
            "Pulling Docker image"
        );

        let pull = Command::new("docker")
            .args(["pull", image])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .await?;

        if !pull.success() {
            return Err(DockerError::ImageNotFound(image.to_string()));
        }

        Ok(())
    }

    /// Setup directories for the container
    pub async fn setup_directories(&self, config: &DockerRunConfig) -> Result<(), DockerError> {
        // Create workspace dir (maps to /app)
        std::fs::create_dir_all(&config.workspace_dir)?;

        // Create logs structure
        let agent_logs = config.logs_dir.join("agent");
        let agent_sessions = agent_logs.join("sessions");
        let verifier_logs = config.logs_dir.join("verifier");

        std::fs::create_dir_all(&agent_sessions)?;
        std::fs::create_dir_all(&verifier_logs)?;

        tracing::debug!(
            target: "mechacoder::docker",
            workspace = %config.workspace_dir.display(),
            logs = %config.logs_dir.display(),
            "Created run directories"
        );

        Ok(())
    }

    /// Build environment variables for the container
    fn build_env(&self, config: &DockerRunConfig) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Pass through ANTHROPIC_API_KEY if set
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            env.insert("ANTHROPIC_API_KEY".to_string(), api_key);
        }

        // Model override
        if let Some(ref model) = config.model {
            env.insert("ANTHROPIC_MODEL".to_string(), model.clone());
        }

        // Claude config dir for session logs
        env.insert(
            "CLAUDE_CONFIG_DIR".to_string(),
            "/logs/agent/sessions".to_string(),
        );

        // Enable background tasks
        env.insert("FORCE_AUTO_BACKGROUND_TASKS".to_string(), "1".to_string());
        env.insert("ENABLE_BACKGROUND_TASKS".to_string(), "1".to_string());

        env
    }

    /// Build arguments for running Claude CLI on host
    ///
    /// Wraps the instruction with TestGen protocol, requiring Claude to
    /// DESCRIBE → WRITE TESTS → ITERATE before submitting a solution.
    fn build_claude_args(&self, instruction: &str, max_turns: u32) -> Vec<String> {
        let allowed_tools = ALLOWED_TOOLS.join(",");

        // Wrap instruction with TestGen protocol
        let wrapped_instruction = TestGenWrapper::wrap_instruction(instruction);

        tracing::debug!(
            target: "mechacoder::docker",
            "Instruction wrapped with TestGen protocol"
        );

        vec![
            "--verbose".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--dangerously-skip-permissions".to_string(),
            "-p".to_string(),
            wrapped_instruction,
            "--allowedTools".to_string(),
            allowed_tools,
            "--max-turns".to_string(),
            max_turns.to_string(),
        ]
    }

    /// Run Claude Code in container, streaming events
    pub async fn run_claude(
        &self,
        config: &DockerRunConfig,
        event_tx: mpsc::UnboundedSender<DockerEvent>,
        mut abort_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<DockerRunResult, DockerError> {
        // Check Docker availability
        if !self.is_available().await {
            return Err(DockerError::NotAvailable);
        }

        // Setup directories
        self.setup_directories(config).await?;

        // Ensure image exists
        let image = config.task.docker_image();
        let _ = event_tx.send(DockerEvent::ContainerStarting {
            image: image.to_string(),
        });

        self.ensure_image(image).await?;

        // Try to create credential mount for Claude CLI OAuth credentials (optional)
        tracing::debug!(
            target: "mechacoder::docker",
            "Attempting to create Claude CLI credential mount"
        );
        let credential_mount = match create_credential_mount().await {
            Ok(mount) => {
                tracing::info!(
                    target: "mechacoder::docker",
                    mount = %mount.volume_mount,
                    "Created Claude CLI credential mount"
                );
                Some(mount)
            }
            Err(e) => {
                tracing::warn!(
                    target: "mechacoder::docker",
                    error = %e,
                    "Failed to create credential mount - will use ANTHROPIC_API_KEY instead"
                );
                None
            }
        };

        // Build Claude arguments for running on host
        let claude_args = self.build_claude_args(&config.task.instruction, config.max_turns);

        // Build environment for Claude CLI
        let mut claude_env = self.build_env(config);

        // Set config dir for session logs
        let claude_config_dir = config.logs_dir.join("agent").join("sessions");
        claude_env.insert("CLAUDE_CONFIG_DIR".to_string(), claude_config_dir.display().to_string());

        tracing::info!(
            target: "mechacoder::docker",
            max_turns = config.max_turns,
            workspace = %config.workspace_dir.display(),
            "Starting Claude on host"
        );

        // Run Claude on host with streaming output
        let result = self
            .run_claude_on_host(
                claude_args,
                claude_env,
                &config.workspace_dir,
                &config.logs_dir,
                event_tx.clone(),
                &mut abort_rx
            )
            .await;

        // Clean up credential mount if it was created
        if let Some(ref mount) = credential_mount {
            if let Err(e) = cleanup_credential_mount(mount).await {
                tracing::warn!(
                    target: "mechacoder::docker",
                    error = %e,
                    "Failed to cleanup credential mount"
                );
            }
        }

        match result {
            Ok(run_result) => {
                let _ = event_tx.send(DockerEvent::ContainerStopped {
                    exit_code: run_result.exit_code,
                });
                Ok(run_result)
            }
            Err(e) => {
                let _ = event_tx.send(DockerEvent::Error {
                    message: e.to_string(),
                });
                Err(e)
            }
        }
    }

    /// Run Claude CLI on host with streaming output
    async fn run_claude_on_host(
        &self,
        args: Vec<String>,
        env: HashMap<String, String>,
        workspace_dir: &Path,
        logs_dir: &Path,
        event_tx: mpsc::UnboundedSender<DockerEvent>,
        abort_rx: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<DockerRunResult, DockerError> {
        // Create log file path
        let log_file = logs_dir.join("agent").join("claude-code.txt");

        tracing::debug!(
            target: "mechacoder::docker",
            claude_path = "claude",
            workspace = %workspace_dir.display(),
            "Spawning Claude CLI on host"
        );

        // Spawn Claude CLI process
        let mut child = Command::new("claude")
            .args(&args)
            .current_dir(workspace_dir)
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DockerError::StartFailed(format!("Failed to spawn Claude CLI: {}", e)))?;

        let _ = event_tx.send(DockerEvent::ContainerStarted {
            container_id: "host-claude".to_string(),
        });

        // Stream stdout
        let stdout = child.stdout.take().expect("stdout captured");
        let mut reader = BufReader::new(stdout).lines();

        let mut result = DockerRunResult::default();
        let mut current_turn = 0u32;
        let mut output_lines = Vec::new();

        // Process output with abort handling
        let timeout = Duration::from_secs(3600); // 1 hour default

        let process_result = tokio::time::timeout(timeout, async {
            loop {
                tokio::select! {
                    _ = &mut *abort_rx => {
                        tracing::info!(
                            target: "mechacoder::docker",
                            "Abort signal received, killing Claude"
                        );
                        let _ = child.kill().await;
                        return Err(DockerError::ExecutionFailed("Aborted".to_string()));
                    }
                    line_result = reader.next_line() => {
                        match line_result {
                            Ok(Some(line)) => {
                                output_lines.push(line.clone());

                                // Send raw output
                                let _ = event_tx.send(DockerEvent::ClaudeOutput {
                                    line: line.clone(),
                                });

                                // Try to parse JSON events
                                if let Some(event) = parse_claude_output(&line, &mut current_turn) {
                                    let _ = event_tx.send(event);
                                }

                                // Track metrics from result events
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if json.get("type").and_then(|v| v.as_str()) == Some("result") {
                                        result.turns = json.get("num_turns")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0) as u32;
                                        result.cost_usd = json.get("total_cost_usd")
                                            .and_then(|v| v.as_f64())
                                            .unwrap_or(0.0);
                                        let subtype = json.get("subtype")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        result.success = subtype == "success";
                                    }
                                }
                            }
                            Ok(None) => break, // EOF
                            Err(e) => {
                                tracing::warn!(
                                    target: "mechacoder::docker",
                                    error = %e,
                                    "Error reading stdout"
                                );
                                break;
                            }
                        }
                    }
                }
            }
            Ok(())
        }).await;

        // Handle timeout
        if process_result.is_err() {
            tracing::warn!(
                target: "mechacoder::docker",
                "Claude timed out, killing"
            );
            let _ = child.kill().await;
            return Err(DockerError::Timeout(timeout.as_secs()));
        }

        // Wait for process to complete
        let status = child.wait().await?;
        result.exit_code = status.code().unwrap_or(-1);

        // Write output to log file
        if let Err(e) = fs::write(&log_file, output_lines.join("\n")).await {
            tracing::warn!(
                target: "mechacoder::docker",
                error = %e,
                "Failed to write Claude output log"
            );
        }

        tracing::info!(
            target: "mechacoder::docker",
            exit_code = result.exit_code,
            turns = result.turns,
            cost = result.cost_usd,
            success = result.success,
            "Claude finished"
        );

        Ok(result)
    }

    /// Run container with streaming output
    async fn run_with_streaming(
        &self,
        config: ContainerConfig,
        command: Vec<String>,
        event_tx: mpsc::UnboundedSender<DockerEvent>,
        abort_rx: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<DockerRunResult, DockerError> {
        let container_name = format!("mechacoder-{}", uuid::Uuid::new_v4());

        // Build docker args
        let mut args = vec![
            "run".to_string(),
            "-i".to_string(),
            "--name".to_string(),
            container_name.clone(),
            "--rm".to_string(),
        ];

        // Workspace volume
        args.push("-v".to_string());
        args.push(format!("{}:/app", config.workspace_dir.display()));

        // Additional volumes
        for mount in &config.volume_mounts {
            args.push("-v".to_string());
            args.push(mount.clone());
        }

        // Working directory
        args.push("-w".to_string());
        args.push("/app".to_string());

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

        // Image and command
        args.push(config.image.clone());
        args.extend(command);

        tracing::debug!(
            target: "mechacoder::docker",
            container = %container_name,
            "Spawning docker container"
        );

        // Spawn docker process
        let mut child = Command::new("docker")
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| DockerError::StartFailed(e.to_string()))?;

        let _ = event_tx.send(DockerEvent::ContainerStarted {
            container_id: container_name.clone(),
        });

        // Stream stdout
        let stdout = child.stdout.take().expect("stdout captured");
        let mut reader = BufReader::new(stdout).lines();

        let mut result = DockerRunResult::default();
        let mut current_turn = 0u32;

        // Process output with abort handling
        let timeout = config
            .timeout
            .unwrap_or(Duration::from_secs(3600));

        let process_result = tokio::time::timeout(timeout, async {
            loop {
                tokio::select! {
                    _ = &mut *abort_rx => {
                        tracing::info!(
                            target: "mechacoder::docker",
                            "Abort signal received, killing container"
                        );
                        let _ = Command::new("docker")
                            .args(["kill", &container_name])
                            .output()
                            .await;
                        return Err(DockerError::ExecutionFailed("Aborted".to_string()));
                    }
                    line_result = reader.next_line() => {
                        match line_result {
                            Ok(Some(line)) => {
                                // Send raw output
                                let _ = event_tx.send(DockerEvent::ClaudeOutput {
                                    line: line.clone(),
                                });

                                // Try to parse JSON events
                                if let Some(event) = parse_claude_output(&line, &mut current_turn) {
                                    let _ = event_tx.send(event);
                                }

                                // Track metrics from result events
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                                    if json.get("type").and_then(|v| v.as_str()) == Some("result") {
                                        result.turns = json.get("num_turns")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0) as u32;
                                        result.cost_usd = json.get("total_cost_usd")
                                            .and_then(|v| v.as_f64())
                                            .unwrap_or(0.0);
                                        let subtype = json.get("subtype")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        result.success = subtype == "success";
                                    }
                                }
                            }
                            Ok(None) => break, // EOF
                            Err(e) => {
                                tracing::warn!(
                                    target: "mechacoder::docker",
                                    error = %e,
                                    "Error reading stdout"
                                );
                                break;
                            }
                        }
                    }
                }
            }
            Ok(())
        }).await;

        // Handle timeout
        if process_result.is_err() {
            tracing::warn!(
                target: "mechacoder::docker",
                "Container timed out, killing"
            );
            let _ = Command::new("docker")
                .args(["kill", &container_name])
                .output()
                .await;
            return Err(DockerError::Timeout(timeout.as_secs()));
        }

        // Wait for process to complete
        let status = child.wait().await?;
        result.exit_code = status.code().unwrap_or(-1);

        // Set session dir
        result.session_dir = Some(
            config
                .volume_mounts
                .iter()
                .find(|m| m.contains("/logs"))
                .map(|m| {
                    let host_path = m.split(':').next().unwrap_or("");
                    PathBuf::from(host_path).join("agent").join("sessions")
                })
                .unwrap_or_default(),
        );

        tracing::info!(
            target: "mechacoder::docker",
            exit_code = result.exit_code,
            turns = result.turns,
            cost = result.cost_usd,
            success = result.success,
            "Container finished"
        );

        Ok(result)
    }
}

impl Default for DockerRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse Claude stream-json output line into an event
fn parse_claude_output(line: &str, current_turn: &mut u32) -> Option<DockerEvent> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;
    let event_type = json.get("type")?.as_str()?;

    match event_type {
        "assistant" => {
            *current_turn += 1;
            let message = json.get("message")?;
            let content = message.get("content")?;

            // Extract text from content blocks
            let text = if let Some(text) = content.as_str() {
                text.to_string()
            } else if let Some(arr) = content.as_array() {
                arr.iter()
                    .filter_map(|block| {
                        if block.get("type")?.as_str()? == "text" {
                            block.get("text")?.as_str().map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                return None;
            };

            Some(DockerEvent::AssistantMessage {
                text,
                turn: *current_turn,
            })
        }
        "tool_use" | "tool_result" => {
            let tool_name = json
                .get("tool")
                .or_else(|| json.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let tool_id = json
                .get("id")
                .or_else(|| json.get("tool_use_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            Some(DockerEvent::ToolUse { tool_name, tool_id })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_claude_command() {
        let runner = DockerRunner::new();
        let cmd = runner.build_claude_command("Test instruction", 30);

        assert_eq!(cmd.len(), 3);
        assert_eq!(cmd[0], "bash");
        assert_eq!(cmd[1], "-c");
        assert!(cmd[2].contains("--output-format stream-json"));
        assert!(cmd[2].contains("--max-turns 30"));
        assert!(cmd[2].contains("Test instruction"));
    }

    #[test]
    fn test_parse_assistant_event() {
        let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello"}]}}"#;
        let mut turn = 0;
        let event = parse_claude_output(line, &mut turn);

        assert!(matches!(event, Some(DockerEvent::AssistantMessage { .. })));
        assert_eq!(turn, 1);
    }
}
