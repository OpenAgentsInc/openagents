//! Harbor - Terminal-Bench CLI for agent evaluation
//!
//! This crate provides a CLI tool (`tbench`) for running agents in Harbor's
//! Terminal-Bench evaluation framework. It executes tasks using Claude Code CLI
//! in headless mode and outputs structured results:
//!
//! - `events.jsonl` - Streaming events during execution
//! - `trajectory.json` - ATIF v1.4 format trajectory
//! - `metrics.json` - Token usage, cost, timing, tool stats
//!
//! # Usage
//!
//! ```bash
//! tbench --instruction "Task description" --output-dir /logs/agent
//! ```

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

// ============================================================================
// ATIF Types (Agent Trajectory Interchange Format v1.4)
// ============================================================================

/// ATIF schema version
pub const ATIF_SCHEMA_VERSION: &str = "1.4";

/// Agent identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

impl Agent {
    /// Create a Claude Code agent
    pub fn claude_code(version: &str) -> Self {
        Self {
            name: "claude-code".to_string(),
            version: version.to_string(),
            model: Some("claude-sonnet-4-20250514".to_string()),
            provider: Some("anthropic".to_string()),
        }
    }

    /// Create a MechaCoder agent
    pub fn mechacoder(version: &str) -> Self {
        Self {
            name: "mechacoder".to_string(),
            version: version.to_string(),
            model: None,
            provider: Some("openagents".to_string()),
        }
    }
}

/// Step source in trajectory
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StepSource {
    User,
    Agent,
    System,
}

/// A single step in the trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub step_id: u32,
    pub timestamp: String,
    pub source: StepSource,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<Observation>,
}

/// Tool call record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// Observation from environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    #[serde(rename = "type")]
    pub observation_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ObservationResult>,
}

/// Result of an observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Final metrics for the trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalMetrics {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_cost_usd: Option<f64>,
    pub total_steps: u32,
}

/// Complete ATIF trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub schema_version: String,
    pub session_id: String,
    pub agent: Agent,
    pub steps: Vec<Step>,
    pub final_metrics: FinalMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<TrajectoryExtra>,
}

/// Extra trajectory metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryExtra {
    pub instruction: String,
    pub start_time: String,
    pub end_time: String,
    pub success: bool,
}

impl Trajectory {
    /// Create an empty trajectory
    pub fn new(session_id: &str, agent: Agent) -> Self {
        Self {
            schema_version: ATIF_SCHEMA_VERSION.to_string(),
            session_id: session_id.to_string(),
            agent,
            steps: Vec::new(),
            final_metrics: FinalMetrics {
                total_prompt_tokens: 0,
                total_completion_tokens: 0,
                total_cost_usd: None,
                total_steps: 0,
            },
            extra: None,
        }
    }
}

// ============================================================================
// Event Types
// ============================================================================

/// Event recorded during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBenchEvent {
    pub timestamp: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

impl TBenchEvent {
    /// Create a new event
    pub fn new(event_type: &str, data: serde_json::Value) -> Self {
        Self {
            timestamp: timestamp(),
            event_type: event_type.to_string(),
            data,
        }
    }
}

/// Event recorder that writes to events.jsonl
pub struct EventRecorder {
    writer: BufWriter<File>,
}

impl EventRecorder {
    /// Create a new event recorder
    pub fn new(output_dir: &Path) -> std::io::Result<Self> {
        let path = output_dir.join("events.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    /// Record an event
    pub fn record(&mut self, event_type: &str, data: serde_json::Value) -> std::io::Result<()> {
        let event = TBenchEvent::new(event_type, data);
        let json = serde_json::to_string(&event)?;
        writeln!(self.writer, "{}", json)?;
        self.writer.flush()
    }
}

// ============================================================================
// Metrics Types
// ============================================================================

/// Token usage breakdown
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
    pub total: u64,
}

/// Metrics collected during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TBenchMetrics {
    pub instruction: String,
    pub success: bool,
    pub start_time: String,
    pub end_time: String,
    pub duration_ms: u64,
    pub turns: u32,
    pub tokens: TokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TBenchMetrics {
    /// Write metrics to file
    pub fn write_to_file(&self, output_dir: &Path) -> std::io::Result<()> {
        let path = output_dir.join("metrics.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)
    }
}

// ============================================================================
// Claude CLI Result
// ============================================================================

/// Result from running Claude CLI
#[derive(Debug, Clone, Default)]
pub struct ClaudeResult {
    pub success: bool,
    pub output: String,
    pub session_id: Option<String>,
    pub turns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: Option<f64>,
    pub error: Option<String>,
}

/// Parse Claude CLI JSON output
impl ClaudeResult {
    /// Parse from JSON output
    pub fn parse_json(stdout: &str, exit_code: i32, stderr: &str) -> Self {
        let mut result = Self {
            success: exit_code == 0,
            output: stdout.to_string(),
            error: if exit_code != 0 {
                Some(format!("Claude CLI exited with code {}: {}", exit_code, stderr))
            } else {
                None
            },
            ..Default::default()
        };

        // Try to parse JSON output
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(stdout) {
            result.session_id = json.get("session_id").and_then(|v| v.as_str()).map(String::from);
            result.turns = json.get("num_turns").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            result.cost_usd = json.get("total_cost_usd").and_then(|v| v.as_f64());

            if let Some(usage) = json.get("usage") {
                result.input_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                result.output_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                result.cache_read_tokens = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                result.cache_creation_tokens = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            }

            // Check result type
            if let Some(result_type) = json.get("type").and_then(|v| v.as_str()) {
                if result_type == "result" {
                    let subtype = json.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
                    result.success = subtype == "success";
                    if !result.success && !subtype.is_empty() {
                        result.error = Some(format!("Claude finished with: {}", subtype));
                    }
                }
            }
        }

        result
    }
}

// ============================================================================
// Trajectory Builder
// ============================================================================

/// Builder for constructing trajectories
pub struct TrajectoryBuilder {
    session_id: String,
    agent: Agent,
    instruction: String,
    steps: Vec<Step>,
    step_id: u32,
    start_time: String,
}

impl TrajectoryBuilder {
    /// Create a new trajectory builder
    pub fn new(session_id: &str, agent: Agent, instruction: &str) -> Self {
        let mut builder = Self {
            session_id: session_id.to_string(),
            agent,
            instruction: instruction.to_string(),
            steps: Vec::new(),
            step_id: 0,
            start_time: timestamp(),
        };

        // Add initial user step with instruction
        builder.add_step(StepSource::User, instruction);
        builder
    }

    /// Add a step to the trajectory
    pub fn add_step(&mut self, source: StepSource, message: &str) {
        self.step_id += 1;
        self.steps.push(Step {
            step_id: self.step_id,
            timestamp: timestamp(),
            source,
            message: message.to_string(),
            tool_calls: None,
            observation: None,
        });
    }

    /// Build the final trajectory
    pub fn build(self, success: bool, input_tokens: u64, output_tokens: u64, cost_usd: Option<f64>) -> Trajectory {
        let end_time = timestamp();

        Trajectory {
            schema_version: ATIF_SCHEMA_VERSION.to_string(),
            session_id: self.session_id,
            agent: self.agent,
            steps: self.steps.clone(),
            final_metrics: FinalMetrics {
                total_prompt_tokens: input_tokens,
                total_completion_tokens: output_tokens,
                total_cost_usd: cost_usd,
                total_steps: self.steps.len() as u32,
            },
            extra: Some(TrajectoryExtra {
                instruction: self.instruction,
                start_time: self.start_time,
                end_time,
                success,
            }),
        }
    }

    /// Write trajectory to file
    pub fn write_to_file(&self, output_dir: &Path, success: bool, input_tokens: u64, output_tokens: u64, cost_usd: Option<f64>) -> std::io::Result<()> {
        let trajectory = TrajectoryBuilder {
            session_id: self.session_id.clone(),
            agent: self.agent.clone(),
            instruction: self.instruction.clone(),
            steps: self.steps.clone(),
            step_id: self.step_id,
            start_time: self.start_time.clone(),
        }.build(success, input_tokens, output_tokens, cost_usd);

        let path = output_dir.join("trajectory.json");
        let json = serde_json::to_string_pretty(&trajectory)?;
        std::fs::write(path, json)
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Generate an ISO 8601 timestamp
pub fn timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Generate a unique session ID
pub fn generate_session_id() -> String {
    format!("tbench-{}-{}",
        Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().to_string()[..8]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_agent_claude_code() {
        let agent = Agent::claude_code("2.0.58");
        assert_eq!(agent.name, "claude-code");
        assert_eq!(agent.version, "2.0.58");
        assert!(agent.model.is_some());
    }

    #[test]
    fn test_agent_mechacoder() {
        let agent = Agent::mechacoder("0.1.0");
        assert_eq!(agent.name, "mechacoder");
        assert_eq!(agent.provider, Some("openagents".to_string()));
    }

    #[test]
    fn test_trajectory_builder() {
        let agent = Agent::claude_code("2.0.58");
        let mut builder = TrajectoryBuilder::new("test-session", agent, "Test instruction");

        builder.add_step(StepSource::Agent, "Working on it...");
        builder.add_step(StepSource::System, "Task completed");

        let trajectory = builder.build(true, 1000, 500, Some(0.05));

        assert_eq!(trajectory.session_id, "test-session");
        assert_eq!(trajectory.steps.len(), 3); // user + agent + system
        assert!(trajectory.extra.unwrap().success);
    }

    #[test]
    fn test_event_recorder() {
        let temp = TempDir::new().unwrap();
        let mut recorder = EventRecorder::new(temp.path()).unwrap();

        recorder.record("run_start", serde_json::json!({
            "instruction": "Test task"
        })).unwrap();

        let events_path = temp.path().join("events.jsonl");
        let content = std::fs::read_to_string(events_path).unwrap();
        assert!(content.contains("run_start"));
    }

    #[test]
    fn test_claude_result_parse() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "session_id": "abc123",
            "num_turns": 5,
            "total_cost_usd": 0.123,
            "usage": {
                "input_tokens": 1000,
                "output_tokens": 500,
                "cache_read_input_tokens": 200,
                "cache_creation_input_tokens": 100
            }
        }"#;

        let result = ClaudeResult::parse_json(json, 0, "");

        assert!(result.success);
        assert_eq!(result.session_id, Some("abc123".to_string()));
        assert_eq!(result.turns, 5);
        assert_eq!(result.input_tokens, 1000);
        assert_eq!(result.output_tokens, 500);
        assert_eq!(result.cost_usd, Some(0.123));
    }

    #[test]
    fn test_timestamp_format() {
        let ts = timestamp();
        // Should be ISO 8601 format
        assert!(ts.contains("T"));
        assert!(ts.ends_with("Z"));
    }

    #[test]
    fn test_session_id_format() {
        let id = generate_session_id();
        assert!(id.starts_with("tbench-"));
    }

    #[test]
    fn test_metrics_serialization() {
        let metrics = TBenchMetrics {
            instruction: "Test".to_string(),
            success: true,
            start_time: timestamp(),
            end_time: timestamp(),
            duration_ms: 1000,
            turns: 3,
            tokens: TokenUsage {
                input: 500,
                output: 200,
                cache_read: 0,
                cache_creation: 0,
                total: 700,
            },
            cost: Some(0.01),
            error: None,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"turns\":3"));
    }
}
