//! Trajectory logging for experiment tracking.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use lm_router::LmUsage;

/// Type of step in a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepType {
    /// LLM completion call.
    LlmCall {
        model: String,
        prompt_tokens: usize,
        completion_tokens: usize,
    },
    /// Code execution.
    CodeExecution {
        language: String,
        exit_code: i32,
    },
    /// Sub-query to LLM.
    SubQuery {
        prompt: String,
    },
    /// Retrieval operation.
    Retrieval {
        query: String,
        num_results: usize,
    },
    /// Final answer.
    Final {
        answer: String,
    },
    /// Error encountered.
    Error {
        message: String,
    },
}

/// A single step in an execution trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Step index.
    pub step_id: usize,
    /// Timestamp in milliseconds since trajectory start.
    pub timestamp_ms: u64,
    /// Type of step.
    pub step_type: StepType,
    /// Content (e.g., LLM response, code output).
    pub content: String,
}

impl TrajectoryStep {
    /// Create a new trajectory step.
    pub fn new(step_id: usize, step_type: StepType, content: impl Into<String>) -> Self {
        Self {
            step_id,
            timestamp_ms: 0,
            step_type,
            content: content.into(),
        }
    }

    /// Set the timestamp.
    pub fn with_timestamp(mut self, timestamp_ms: u64) -> Self {
        self.timestamp_ms = timestamp_ms;
        self
    }
}

/// Full execution trajectory for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    /// Task ID.
    pub task_id: String,
    /// Method name.
    pub method: String,
    /// Execution steps.
    pub steps: Vec<TrajectoryStep>,
    /// Start time (Unix timestamp in ms).
    pub start_time: u64,
    /// End time (Unix timestamp in ms).
    pub end_time: u64,
}

impl Trajectory {
    /// Create a new empty trajectory.
    pub fn new(task_id: impl Into<String>, method: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            task_id: task_id.into(),
            method: method.into(),
            steps: Vec::new(),
            start_time: now,
            end_time: now,
        }
    }

    /// Add a step to the trajectory.
    pub fn add_step(&mut self, step_type: StepType, content: impl Into<String>) {
        let step_id = self.steps.len();
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let timestamp_ms = now.saturating_sub(self.start_time);

        self.steps.push(TrajectoryStep {
            step_id,
            timestamp_ms,
            step_type,
            content: content.into(),
        });
        self.end_time = now;
    }

    /// Add an LLM call step.
    pub fn add_llm_call(
        &mut self,
        model: impl Into<String>,
        usage: &LmUsage,
        response: impl Into<String>,
    ) {
        self.add_step(
            StepType::LlmCall {
                model: model.into(),
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
            },
            response,
        );
    }

    /// Add a code execution step.
    pub fn add_code_execution(
        &mut self,
        language: impl Into<String>,
        exit_code: i32,
        output: impl Into<String>,
    ) {
        self.add_step(
            StepType::CodeExecution {
                language: language.into(),
                exit_code,
            },
            output,
        );
    }

    /// Add a final answer step.
    pub fn add_final(&mut self, answer: impl Into<String>) {
        let answer = answer.into();
        self.add_step(StepType::Final { answer: answer.clone() }, answer);
    }

    /// Add an error step.
    pub fn add_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.add_step(StepType::Error { message: message.clone() }, message);
    }

    /// Create a single-step trajectory.
    pub fn single_step(
        task_id: impl Into<String>,
        method: impl Into<String>,
        step_type: StepType,
        content: impl Into<String>,
    ) -> Self {
        let mut trajectory = Self::new(task_id, method);
        trajectory.add_step(step_type, content);
        trajectory
    }

    /// Get total duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.end_time.saturating_sub(self.start_time)
    }

    /// Get total LLM tokens used.
    pub fn total_tokens(&self) -> usize {
        self.steps
            .iter()
            .filter_map(|s| {
                if let StepType::LlmCall {
                    prompt_tokens,
                    completion_tokens,
                    ..
                } = &s.step_type
                {
                    Some(prompt_tokens + completion_tokens)
                } else {
                    None
                }
            })
            .sum()
    }

    /// Get total usage.
    pub fn total_usage(&self) -> LmUsage {
        let mut usage = LmUsage::default();
        for step in &self.steps {
            if let StepType::LlmCall {
                prompt_tokens,
                completion_tokens,
                ..
            } = &step.step_type
            {
                usage.prompt_tokens += prompt_tokens;
                usage.completion_tokens += completion_tokens;
                usage.total_tokens += prompt_tokens + completion_tokens;
            }
        }
        usage
    }
}

/// JSONL writer for trajectories.
pub struct TrajectoryWriter {
    path: PathBuf,
    writer: BufWriter<File>,
}

impl TrajectoryWriter {
    /// Create a new trajectory writer.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let file = File::create(&path)?;
        Ok(Self {
            path,
            writer: BufWriter::new(file),
        })
    }

    /// Write a trajectory to the file.
    pub fn write(&mut self, trajectory: &Trajectory) -> Result<()> {
        let json = serde_json::to_string(trajectory)?;
        writeln!(self.writer, "{}", json)?;
        Ok(())
    }

    /// Flush the writer.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    /// Get the output path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_creation() {
        let mut trajectory = Trajectory::new("task-1", "base");

        let usage = LmUsage::new(100, 50);
        trajectory.add_llm_call("gpt-4", &usage, "The answer is 4");
        trajectory.add_final("4");

        assert_eq!(trajectory.steps.len(), 2);
        assert_eq!(trajectory.total_tokens(), 150);
    }

    #[test]
    fn test_step_types() {
        let llm_step = StepType::LlmCall {
            model: "gpt-4".to_string(),
            prompt_tokens: 100,
            completion_tokens: 50,
        };

        if let StepType::LlmCall {
            model,
            prompt_tokens,
            ..
        } = llm_step
        {
            assert_eq!(model, "gpt-4");
            assert_eq!(prompt_tokens, 100);
        }
    }

    #[test]
    fn test_single_step() {
        let trajectory = Trajectory::single_step(
            "task-1",
            "base",
            StepType::Final {
                answer: "42".to_string(),
            },
            "The answer is 42",
        );

        assert_eq!(trajectory.steps.len(), 1);
    }
}
