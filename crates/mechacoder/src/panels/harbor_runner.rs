//! Harbor Runner - Executes TB2 tasks using Harbor's tbench binary
//!
//! This module runs Terminal-Bench tasks using Harbor's official tbench harness.
//! Key benefits over docker_runner:
//! - Automatic ATIF v1.4 trajectory saving (required for leaderboard)
//! - Better isolation (no contamination from host computer)
//! - Official TB2 harness (matches leaderboard environment)
//! - Streaming events for real-time UI updates

use crate::panels::docker_runner::{DockerEvent, DockerRunResult};
use crate::panels::verifier::{TB2Verifier, VerificationResult};
use std::path::PathBuf;
use std::process::Stdio;
use terminalbench::TB2Task;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::{self, UnboundedSender};

/// Configuration for a Harbor TB2 run
#[derive(Debug, Clone)]
pub struct HarborRunConfig {
    /// TB2 task being run
    pub task: TB2Task,
    /// Working directory for task execution (workspace on host)
    pub workspace_dir: PathBuf,
    /// Output directory for logs, trajectory, metrics (on host)
    pub output_dir: PathBuf,
    /// Max turns for Claude
    pub max_turns: u32,
    /// Model to use (if overriding default)
    pub model: Option<String>,
}

impl HarborRunConfig {
    /// Create a new Harbor run config
    pub fn new(task: TB2Task, workspace_dir: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            task,
            workspace_dir,
            output_dir,
            max_turns: 300, // Default max turns (matches task_loader.rs)
            model: None,
        }
    }

    /// Set max turns
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.max_turns = turns;
        self
    }

    /// Set model
    pub fn model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }
}

/// Harbor StreamEvent from Harbor's lib.rs
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    RunStart {
        session_id: String,
        #[allow(dead_code)]
        instruction: String,
    },
    Assistant {
        turn: u32,
        text: String,
    },
    ToolUse {
        tool: String,
        id: String,
    },
    #[allow(dead_code)]
    ToolResult {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Complete {
        success: bool,
        turns: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        cost: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

/// Runner for executing TB2 tasks using Harbor's tbench
pub struct HarborRunner;

impl HarborRunner {
    pub fn new() -> Self {
        Self
    }

    /// Run a TB2 task using Harbor's tbench binary
    ///
    /// Spawns `cargo run --bin tbench -- --stream ...` and streams events to UI.
    /// Automatically saves ATIF trajectory to output_dir/trajectory.json.
    pub async fn run_tbench(
        &self,
        config: HarborRunConfig,
        event_tx: UnboundedSender<DockerEvent>,
        mut abort_rx: mpsc::Receiver<()>,
    ) -> Result<(), HarborRunError> {
        // Send initial RunStart event
        event_tx
            .send(DockerEvent::RunStart {
                task_id: config.task.id.clone(),
                task_name: config.task.name.clone(),
                image_name: "harbor-tbench".to_string(),
                working_dir: config.workspace_dir.display().to_string(),
            })
            .map_err(|_| HarborRunError::ChannelClosed)?;

        // Ensure output directory exists
        std::fs::create_dir_all(&config.output_dir)
            .map_err(|e| HarborRunError::IoError(e.to_string()))?;

        // Build tbench command
        let mut args = vec![
            "run".to_string(),
            "--bin".to_string(),
            "tbench".to_string(),
            "--".to_string(),
            "--instruction".to_string(),
            config.task.instruction.clone(),
            "--output-dir".to_string(),
            config.output_dir.display().to_string(),
            "--cwd".to_string(),
            config.workspace_dir.display().to_string(),
            "--max-turns".to_string(),
            config.max_turns.to_string(),
            "--stream".to_string(),
        ];

        // Add model if specified
        if let Some(ref model) = config.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        // Spawn tbench
        let mut child = Command::new("cargo")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| HarborRunError::SpawnError(e.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HarborRunError::IoError("Failed to capture stdout".to_string()))?;

        let mut stdout_reader = BufReader::new(stdout).lines();

        let mut run_result = DockerRunResult::default();

        // Process streaming events
        loop {
            tokio::select! {
                // Abort signal received
                _ = abort_rx.recv() => {
                    let _ = child.kill().await;
                    event_tx.send(DockerEvent::Error {
                        message: "Run aborted by user".to_string(),
                    }).ok();
                    return Err(HarborRunError::Aborted);
                }

                // Read next line from stdout
                line_result = stdout_reader.next_line() => {
                    match line_result {
                        Ok(Some(line)) => {
                            // Try to parse as StreamEvent
                            if let Ok(event) = serde_json::from_str::<StreamEvent>(&line) {
                                match event {
                                    StreamEvent::RunStart { session_id, .. } => {
                                        // Store session_id for later
                                        run_result.session_dir = Some(config.output_dir.join(&session_id));
                                    }
                                    StreamEvent::Assistant { turn, text } => {
                                        event_tx.send(DockerEvent::AssistantMessage { text, turn }).ok();
                                    }
                                    StreamEvent::ToolUse { tool, id } => {
                                        event_tx.send(DockerEvent::ToolUse {
                                            tool_name: tool,
                                            tool_id: id,
                                        }).ok();
                                    }
                                    StreamEvent::ToolResult { .. } => {
                                        // Don't need to forward tool results to UI
                                    }
                                    StreamEvent::Complete { success, turns, cost, error } => {
                                        run_result.success = success;
                                        run_result.turns = turns;
                                        run_result.cost_usd = cost.unwrap_or(0.0);
                                        run_result.error = error.clone();

                                        // Emit turn complete
                                        event_tx.send(DockerEvent::TurnComplete { turn: turns }).ok();

                                        // Run verification
                                        let verification = self.verify_task(&config).await;

                                        // Send final RunComplete event
                                        event_tx.send(DockerEvent::RunComplete {
                                            run_result: Some(run_result.clone()),
                                            run_error: error,
                                            verification: Some(verification),
                                        }).ok();

                                        // Wait for process to exit
                                        let _ = child.wait().await;
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        Ok(None) => {
                            // EOF - process exited
                            break;
                        }
                        Err(e) => {
                            event_tx.send(DockerEvent::Error {
                                message: format!("Stream read error: {}", e),
                            }).ok();
                            return Err(HarborRunError::IoError(e.to_string()));
                        }
                    }
                }
            }
        }

        // Process exited without Complete event - something went wrong
        let exit_status = child.wait().await.map_err(|e| HarborRunError::IoError(e.to_string()))?;
        let exit_code = exit_status.code().unwrap_or(-1);

        run_result.success = false;
        run_result.exit_code = exit_code;
        run_result.error = Some(format!("tbench exited with code {}", exit_code));

        event_tx
            .send(DockerEvent::RunComplete {
                run_result: Some(run_result.clone()),
                run_error: run_result.error.clone(),
                verification: None,
            })
            .ok();

        Err(HarborRunError::ProcessError(format!(
            "tbench exited with code {}",
            exit_code
        )))
    }

    /// Run verification using TB verifier
    async fn verify_task(&self, config: &HarborRunConfig) -> VerificationResult {
        let verifier = TB2Verifier::new();
        // Use output_dir's parent as logs_dir (output_dir is logs/agent, so parent is logs)
        let logs_dir = config.output_dir.parent().unwrap_or(&config.output_dir);
        verifier
            .run_tests(&config.task, &config.workspace_dir, logs_dir)
            .await
            .unwrap_or_else(|e| VerificationResult {
                passed: false,
                reward: 0.0,
                tests_passed: 0,
                tests_total: 0,
                output: String::new(),
                error: Some(e.to_string()),
            })
    }
}

impl Default for HarborRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum HarborRunError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Failed to spawn tbench: {0}")]
    SpawnError(String),
    #[error("tbench process error: {0}")]
    ProcessError(String),
    #[error("Event channel closed")]
    ChannelClosed,
    #[error("Run aborted")]
    Aborted,
}
