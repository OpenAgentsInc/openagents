//! Docker Runner - Executes Claude Code in TB2 containers via SDK
//!
//! This module uses the claude-agent-sdk to run Claude Code on the host,
//! with the workspace mounted to TB2 Docker containers for verification.

use crate::panels::testgen_wrapper::TestGenWrapper;
use crate::panels::verifier::VerificationResult;
use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;
use sandbox::{ContainerBackend, ContainerConfig, DockerBackend};
use std::path::PathBuf;
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
    /// Run is starting
    RunStart {
        task_id: String,
        task_name: String,
        image_name: String,
        working_dir: String,
    },
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


    /// Run Claude Code in container, streaming events
    pub async fn run_claude(
        &self,
        config: &DockerRunConfig,
        event_tx: mpsc::UnboundedSender<DockerEvent>,
        _abort_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<DockerRunResult, DockerError> {
        // Check Docker availability
        if !self.is_available().await {
            return Err(DockerError::NotAvailable);
        }

        // Send run start event with metadata
        let _ = event_tx.send(DockerEvent::RunStart {
            task_id: config.task.id.clone(),
            task_name: config.task.name.clone(),
            image_name: config.task.docker_image().to_string(),
            working_dir: config.workspace_dir.display().to_string(),
        });

        // Setup directories
        self.setup_directories(config).await?;

        // Ensure image exists
        let image = config.task.docker_image();
        let _ = event_tx.send(DockerEvent::ContainerStarting {
            image: image.to_string(),
        });

        self.ensure_image(image).await?;

        // Run Claude via SDK on host
        // Note: Claude CLI will use credentials from ~/.claude/.credentials.json
        // or ANTHROPIC_API_KEY environment variable (inherited via build_env_vars)
        let result = self
            .run_claude_with_sdk(config, event_tx.clone())
            .await;

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

    /// Run Claude via SDK with streaming output
    async fn run_claude_with_sdk(
        &self,
        config: &DockerRunConfig,
        event_tx: mpsc::UnboundedSender<DockerEvent>,
    ) -> Result<DockerRunResult, DockerError> {
        let instruction = TestGenWrapper::wrap_instruction(&config.task.instruction);

        tracing::debug!(
            target: "mechacoder::docker",
            "Instruction wrapped with TestGen protocol"
        );

        // Build query options
        let mut options = QueryOptions::new()
            .max_turns(config.max_turns)
            .cwd(config.workspace_dir.clone())
            .dangerously_skip_permissions(true);

        // Set allowed tools
        options.allowed_tools = Some(ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect());

        if let Some(model) = &config.model {
            options = options.model(model.clone());
        }

        tracing::info!(
            target: "mechacoder::docker",
            max_turns = config.max_turns,
            workspace = %config.workspace_dir.display(),
            "Starting Claude via SDK"
        );

        // Send container started event
        let _ = event_tx.send(DockerEvent::ContainerStarted {
            container_id: "host-claude-sdk".to_string(),
        });

        // Run query with streaming
        let mut stream = query(instruction, options)
            .await
            .map_err(|e| DockerError::StartFailed(format!("SDK error: {}", e)))?;

        let mut turns = 0;
        let mut cost_usd = 0.0;
        let mut success = false;

        // Process stream
        while let Some(msg) = stream.next().await {
            match msg.map_err(|e| DockerError::ExecutionFailed(e.to_string()))? {
                SdkMessage::Assistant(msg) => {
                    // Check for authentication errors immediately
                    if let Some(error_type) = msg.error.as_ref() {
                        if matches!(error_type, claude_agent_sdk::AssistantMessageError::AuthenticationFailed) {
                            tracing::error!(
                                target: "mechacoder::docker",
                                "Claude CLI authentication failed - check credentials at ~/.claude/.credentials.json or ANTHROPIC_API_KEY"
                            );

                            let _ = event_tx.send(DockerEvent::Error {
                                message: "Authentication failed. Run 'claude' in terminal to authenticate first.".to_string(),
                            });

                            return Err(DockerError::ExecutionFailed(
                                "Claude CLI authentication failed. Credentials not found.".to_string()
                            ));
                        }
                    }

                    turns += 1;

                    tracing::debug!(
                        target: "mechacoder::docker",
                        turn = turns,
                        "Assistant turn"
                    );

                    let _ = event_tx.send(DockerEvent::TurnComplete { turn: turns });

                    // Extract and send all text content (including from tool_use blocks)
                    if let Some(content_arr) = msg.message.get("content").and_then(|v| v.as_array()) {
                        let mut full_text = String::new();

                        for block in content_arr {
                            // Handle text blocks
                            if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                                if !full_text.is_empty() {
                                    full_text.push('\n');
                                }
                                full_text.push_str(text);
                            }
                            // Handle tool_use blocks - show what tool is being called
                            else if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                                if let Some(tool_name) = block.get("name").and_then(|v| v.as_str()) {
                                    if !full_text.is_empty() {
                                        full_text.push('\n');
                                    }
                                    full_text.push_str(&format!("[Using tool: {}]", tool_name));
                                }
                            }
                        }

                        if !full_text.is_empty() {
                            tracing::info!(
                                target: "mechacoder::docker",
                                turn = turns,
                                text_len = full_text.len(),
                                "Sending assistant message to UI"
                            );

                            let _ = event_tx.send(DockerEvent::AssistantMessage {
                                text: full_text,
                                turn: turns,
                            });
                        }
                    }
                }
                SdkMessage::ToolProgress(p) => {
                    tracing::debug!(
                        target: "mechacoder::docker",
                        tool = %p.tool_name,
                        elapsed = p.elapsed_time_seconds,
                        "Tool executing"
                    );

                    let _ = event_tx.send(DockerEvent::ToolUse {
                        tool_name: p.tool_name.clone(),
                        tool_id: p.tool_use_id.clone(),
                    });
                }
                SdkMessage::Result(result) => {
                    // Extract fields based on result type
                    match result {
                        claude_agent_sdk::SdkResultMessage::Success(s) => {
                            cost_usd = s.total_cost_usd;
                            turns = s.num_turns;
                            success = !s.is_error;
                        }
                        claude_agent_sdk::SdkResultMessage::ErrorDuringExecution(e) |
                        claude_agent_sdk::SdkResultMessage::ErrorMaxTurns(e) |
                        claude_agent_sdk::SdkResultMessage::ErrorMaxBudget(e) |
                        claude_agent_sdk::SdkResultMessage::ErrorMaxStructuredOutputRetries(e) => {
                            cost_usd = e.total_cost_usd;
                            turns = e.num_turns;
                            success = false;
                        }
                    }

                    tracing::info!(
                        target: "mechacoder::docker",
                        turns = turns,
                        cost = cost_usd,
                        success = success,
                        "Claude completed via SDK"
                    );
                }
                SdkMessage::User(user_msg) => {
                    // User messages contain tool results - show them in UI
                    if let Some(content_arr) = user_msg.message.get("content").and_then(|v| v.as_array()) {
                        for block in content_arr {
                            if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                                if let Some(content) = block.get("content").and_then(|v| v.as_str()) {
                                    // Send tool output to UI (truncate if very long)
                                    let display_content = if content.len() > 500 {
                                        format!("{}... (truncated)", &content[..500])
                                    } else {
                                        content.to_string()
                                    };

                                    tracing::info!(
                                        target: "mechacoder::docker",
                                        content_len = content.len(),
                                        "Sending tool result to UI"
                                    );

                                    let _ = event_tx.send(DockerEvent::AssistantMessage {
                                        text: format!("[Tool output]\n{}", display_content),
                                        turn: turns,
                                    });
                                }
                            }
                        }
                    }
                }
                SdkMessage::StreamEvent(_event) => {
                    // Partial content streaming - can log if needed
                    tracing::trace!(
                        target: "mechacoder::docker",
                        "Stream event"
                    );
                }
                _ => {
                    // Other message types (System, etc.)
                    tracing::trace!(
                        target: "mechacoder::docker",
                        "Other SDK message"
                    );
                }
            }
        }

        Ok(DockerRunResult {
            success,
            turns,
            cost_usd,
            error: None,
            exit_code: if success { 0 } else { 1 },
            session_dir: Some(config.logs_dir.join("agent/sessions")),
        })
    }


    /// Run container with streaming output
    #[allow(dead_code)]
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


#[cfg(test)]
mod tests {
    use super::*;

    // Tests removed - old implementation tests for manual JSONL parsing
    // SDK-based implementation is tested via integration tests
}
