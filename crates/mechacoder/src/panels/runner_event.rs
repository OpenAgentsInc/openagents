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
            // ClaudeOutput is raw output, not needed for UI events
            DockerEvent::ClaudeOutput { .. } => vec![],
        }
    }
}
