//! Event type definitions for HillClimber and TestGen

use serde::{Deserialize, Serialize};

/// WebSocket event wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketEvent {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// HillClimber event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HillClimberEvent {
    #[serde(rename = "map_turn_start")]
    TurnStart {
        #[serde(rename = "sessionId")]
        session_id: String,
        turn: u32,
        #[serde(rename = "maxTurns")]
        max_turns: u32,
        subtask: String,
    },
    #[serde(rename = "map_fm_action")]
    FMAction {
        #[serde(rename = "sessionId")]
        session_id: String,
        action: String, // "thinking", "tool_call", "complete"
        #[serde(rename = "toolName", skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
    },
    #[serde(rename = "map_verify")]
    Verify {
        #[serde(rename = "sessionId")]
        session_id: String,
        status: String, // "start", "complete"
        #[serde(skip_serializing_if = "Option::is_none")]
        passed: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        progress: Option<f32>,
    },
    #[serde(rename = "map_heartbeat")]
    Heartbeat {
        #[serde(rename = "sessionId")]
        session_id: String,
        turn: u32,
        #[serde(rename = "maxTurns")]
        max_turns: u32,
        progress: f32,
        #[serde(rename = "bestProgress")]
        best_progress: f32,
        #[serde(rename = "elapsedMs")]
        elapsed_ms: u64,
    },
    #[serde(rename = "map_run_complete")]
    RunComplete {
        #[serde(rename = "sessionId")]
        session_id: String,
        success: bool,
        #[serde(rename = "finalProgress")]
        final_progress: f32,
    },
}

/// TestGen event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TestGenEvent {
    #[serde(rename = "testgen_start")]
    Start {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "taskId")]
        task_id: String,
        description: String,
    },
    #[serde(rename = "testgen_iteration")]
    Iteration {
        #[serde(rename = "sessionId")]
        session_id: String,
        iteration: u32,
        #[serde(rename = "maxIterations")]
        max_iterations: u32,
    },
    #[serde(rename = "testgen_category")]
    Category {
        #[serde(rename = "sessionId")]
        session_id: String,
        category: String,
        #[serde(rename = "testCount")]
        test_count: u32,
    },
    #[serde(rename = "testgen_test")]
    Test {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "testId")]
        test_id: String,
        category: String,
        description: String,
        code: String,
    },
    #[serde(rename = "testgen_complete")]
    Complete {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "totalTests")]
        total_tests: u32,
        score: f32,
    },
}
