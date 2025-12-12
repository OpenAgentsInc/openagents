//! Unified event types for TB2 Docker runs
//!
//! Bridges DockerEvent to UI-compatible TB2RunnerEvent with container metadata

use crate::panels::docker_runner::DockerEvent;

/// Unified event type for TB2 runs with container metadata
#[derive(Debug, Clone)]
pub enum TB2RunnerEvent {
    /// Run is starting
    RunStart {
        run_id: String,
        task_id: String,
        task_name: String,
        container_id: Option<String>,
        image_name: String,
    },
    /// Container is starting
    ContainerStarting {
        run_id: String,
        image: String,
    },
    /// Container started successfully
    ContainerStarted {
        run_id: String,
        container_id: String,
    },
    /// Assistant message content
    AssistantMessage {
        run_id: String,
        turn: u32,
        text: String,
    },
    /// Tool use
    ToolUse {
        run_id: String,
        tool_name: String,
        tool_id: String,
    },
    /// Turn completed
    TurnComplete {
        run_id: String,
        turn: u32,
    },
    /// Container stopped
    ContainerStopped {
        run_id: String,
        exit_code: i32,
    },
    /// Run completed
    RunComplete {
        run_id: String,
        success: bool,
        turns: u32,
        cost_usd: f64,
        verification_passed: bool,
        verification_reward: f64,
        error: Option<String>,
    },
    /// Error occurred
    Error {
        run_id: String,
        message: String,
    },
}

impl TB2RunnerEvent {
    /// Convert a DockerEvent to TB2RunnerEvent
    ///
    /// Returns a vector because some events might not have a corresponding
    /// TB2RunnerEvent or might generate multiple events.
    pub fn from_docker_event(run_id: String, event: DockerEvent) -> Vec<Self> {
        match event {
            DockerEvent::ContainerStarting { image } => {
                vec![Self::ContainerStarting {
                    run_id,
                    image,
                }]
            }
            DockerEvent::ContainerStarted { container_id } => {
                vec![Self::ContainerStarted {
                    run_id,
                    container_id,
                }]
            }
            DockerEvent::AssistantMessage { text, turn } => {
                vec![Self::AssistantMessage {
                    run_id,
                    turn,
                    text,
                }]
            }
            DockerEvent::ToolUse { tool_name, tool_id } => {
                vec![Self::ToolUse {
                    run_id,
                    tool_name,
                    tool_id,
                }]
            }
            DockerEvent::TurnComplete { turn } => {
                vec![Self::TurnComplete {
                    run_id,
                    turn,
                }]
            }
            DockerEvent::ContainerStopped { exit_code } => {
                vec![Self::ContainerStopped {
                    run_id,
                    exit_code,
                }]
            }
            DockerEvent::Error { message } => {
                vec![Self::Error {
                    run_id,
                    message,
                }]
            }
            DockerEvent::RunComplete { run_result, run_error, verification } => {
                vec![Self::RunComplete {
                    run_id,
                    success: run_result.as_ref().map(|r| r.success).unwrap_or(false),
                    turns: run_result.as_ref().map(|r| r.turns).unwrap_or(0),
                    cost_usd: run_result.as_ref().map(|r| r.cost_usd).unwrap_or(0.0),
                    verification_passed: verification.as_ref().map(|v| v.passed).unwrap_or(false),
                    verification_reward: verification.as_ref().map(|v| v.reward).unwrap_or(0.0),
                    error: run_error.clone(),
                }]
            }
            // ClaudeOutput is raw output, not needed for UI events
            DockerEvent::ClaudeOutput { .. } => vec![],
        }
    }
}
