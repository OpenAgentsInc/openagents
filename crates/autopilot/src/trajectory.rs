//! Trajectory data structures for recording agent runs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Complete trajectory of an agent run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub session_id: String,
    pub prompt: String,
    pub model: String,
    pub cwd: String,
    pub repo_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    pub steps: Vec<Step>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TrajectoryResult>,
    pub usage: TokenUsage,
}

/// A single step in the trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub step_id: u32,
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub step_type: StepType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_cached: Option<u64>,
}

/// Type of step in the trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepType {
    /// User message
    User { content: String },
    /// Assistant text response
    Assistant { content: String },
    /// Thinking block
    Thinking {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// Tool call (start)
    ToolCall {
        tool: String,
        tool_id: String,
        input: Value,
    },
    /// Tool result
    ToolResult {
        tool_id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    /// System init message
    SystemInit { model: String },
    /// System status
    SystemStatus { status: String },
    /// Subagent spawn/completion (for x: lines in rlog)
    Subagent {
        /// Unique ID for the subagent
        agent_id: String,
        /// Type of subagent (explore, plan, etc.)
        agent_type: String,
        /// Status: started, done, error
        status: SubagentStatus,
        /// Summary of subagent work (only on completion)
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
}

/// Status of a subagent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Started,
    Done,
    Error,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

/// Result of the trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryResult {
    pub success: bool,
    pub duration_ms: u64,
    pub num_turns: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    #[serde(default)]
    pub issues_completed: u32,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(
        prompt: String,
        model: String,
        cwd: String,
        repo_sha: String,
        branch: Option<String>,
    ) -> Self {
        Self {
            session_id: String::new(),
            prompt,
            model,
            cwd,
            repo_sha,
            branch,
            started_at: Utc::now(),
            ended_at: None,
            steps: Vec::new(),
            result: None,
            usage: TokenUsage::default(),
        }
    }

    /// Add a step to the trajectory
    pub fn add_step(&mut self, step_type: StepType) -> &mut Step {
        let step_id = self.steps.len() as u32 + 1;
        self.steps.push(Step {
            step_id,
            timestamp: Utc::now(),
            step_type,
            tokens_in: None,
            tokens_out: None,
            tokens_cached: None,
        });
        self.steps.last_mut().unwrap()
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

impl Step {
    /// Get the content for display
    pub fn content(&self) -> Option<&str> {
        match &self.step_type {
            StepType::User { content } => Some(content),
            StepType::Assistant { content } => Some(content),
            StepType::Thinking { content, .. } => Some(content),
            StepType::ToolResult { output, .. } => output.as_deref(),
            StepType::SystemInit { model } => Some(model),
            StepType::SystemStatus { status } => Some(status),
            StepType::ToolCall { .. } => None,
            StepType::Subagent { summary, .. } => summary.as_deref(),
        }
    }
}

/// Writer for JSONL files (full untruncated data)
///
/// Writes SDK messages in Claude Code compatible JSONL format.
/// Each message is written as a single line of JSON.
pub struct JsonlWriter {
    writer: Option<BufWriter<File>>,
    path: Option<String>,
    session_id: String,
    line_count: u64,
}

impl JsonlWriter {
    /// Create a new JSONL writer
    pub fn new() -> Self {
        Self {
            writer: None,
            path: None,
            session_id: String::new(),
            line_count: 0,
        }
    }

    /// Initialize the writer with a file path
    pub fn init<P: AsRef<Path>>(&mut self, path: P, session_id: &str) -> std::io::Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.as_ref())?;

        self.writer = Some(BufWriter::new(file));
        self.path = Some(path.as_ref().to_string_lossy().to_string());
        self.session_id = session_id.to_string();
        self.line_count = 0;
        Ok(())
    }

    /// Write a raw SDK message as JSONL
    ///
    /// This preserves the full message without any truncation.
    pub fn write_message<T: Serialize>(&mut self, message: &T) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.writer {
            let json = serde_json::to_string(message)?;
            writeln!(writer, "{}", json)?;
            self.line_count += 1;
        }
        Ok(())
    }

    /// Write a raw JSON value as JSONL
    pub fn write_value(&mut self, value: &Value) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.writer {
            let json = serde_json::to_string(value)?;
            writeln!(writer, "{}", json)?;
            self.line_count += 1;
        }
        Ok(())
    }

    /// Flush the writer
    pub fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush()?;
        }
        Ok(())
    }

    /// Get the number of lines written
    pub fn line_count(&self) -> u64 {
        self.line_count
    }

    /// Get the file path
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

impl Default for JsonlWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for JsonlWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
