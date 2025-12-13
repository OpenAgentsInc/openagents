use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::{Agent, AtifError, FinalMetrics, Step};

/// Root-level trajectory object storing global context and complete interaction history.
///
/// ## Example
///
/// ```
/// use atif::*;
///
/// let trajectory = Trajectory {
///     schema_version: "ATIF-v1.4".to_string(),
///     session_id: "ABC123".to_string(),
///     agent: Agent::new("my-agent", "1.0.0"),
///     steps: vec![
///         Step::user(1, "What is the price of GOOGL?"),
///         Step::agent(2, "Let me search for that information."),
///     ],
///     notes: None,
///     final_metrics: None,
///     extra: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trajectory {
    /// String defining ATIF compatibility (e.g., "ATIF-v1.4")
    pub schema_version: String,

    /// Unique identifier for the entire agent run
    pub session_id: String,

    /// Object specifying the agent configuration
    pub agent: Agent,

    /// Array of step objects representing the complete interaction history
    ///
    /// Includes user messages, agent responses, tool calls, and observations.
    pub steps: Vec<Step>,

    /// Optional notes field for developers to include custom information,
    /// design notes, or explanations for format discrepancies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Summary metrics for the entire trajectory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_metrics: Option<FinalMetrics>,

    /// Custom root-level metadata not covered by the core schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl Trajectory {
    /// Create a new trajectory
    pub fn new(
        schema_version: impl Into<String>,
        session_id: impl Into<String>,
        agent: Agent,
    ) -> Self {
        Self {
            schema_version: schema_version.into(),
            session_id: session_id.into(),
            agent,
            steps: Vec::new(),
            notes: None,
            final_metrics: None,
            extra: None,
        }
    }

    /// Create a new trajectory with ATIF v1.4 schema
    pub fn v1_4(session_id: impl Into<String>, agent: Agent) -> Self {
        Self::new("ATIF-v1.4", session_id, agent)
    }

    /// Add a step to the trajectory
    pub fn add_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    /// Set notes
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Set final metrics
    pub fn with_final_metrics(mut self, metrics: FinalMetrics) -> Self {
        self.final_metrics = Some(metrics);
        self
    }

    /// Set extra metadata
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }

    /// Validate the trajectory
    ///
    /// Checks:
    /// 1. Step IDs are sequential starting from 1
    /// 2. All tool call references exist
    /// 3. Individual step validation
    pub fn validate(&self) -> Result<(), AtifError> {
        // Validate step sequence
        for (i, step) in self.steps.iter().enumerate() {
            let expected_id = (i + 1) as i64;
            if step.step_id != expected_id {
                return Err(AtifError::InvalidStepSequence(format!(
                    "Step ID {} at index {} does not match expected sequential ID {}",
                    step.step_id, i, expected_id
                )));
            }

            // Validate individual step
            step.validate().map_err(|e| AtifError::Validation(e))?;
        }

        // Validate tool call references
        self.validate_tool_call_refs()?;

        Ok(())
    }

    /// Validate that all observation source_call_ids reference existing tool_call_ids
    fn validate_tool_call_refs(&self) -> Result<(), AtifError> {
        for step in &self.steps {
            // Collect all tool_call_ids in this step
            let tool_call_ids: HashSet<&str> = step
                .tool_calls
                .as_ref()
                .map(|calls| calls.iter().map(|c| c.tool_call_id.as_str()).collect())
                .unwrap_or_default();

            // Check observation results
            if let Some(observation) = &step.observation {
                for result in &observation.results {
                    if let Some(source_call_id) = &result.source_call_id {
                        if !tool_call_ids.contains(source_call_id.as_str()) {
                            return Err(AtifError::ToolCallReferenceError(format!(
                                "Observation source_call_id '{}' in step {} does not reference any tool_call_id",
                                source_call_id, step.step_id
                            )));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Save trajectory to JSON file
    pub fn to_json_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), AtifError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load trajectory from JSON file
    pub fn from_json_file(path: impl AsRef<std::path::Path>) -> Result<Self, AtifError> {
        let json = std::fs::read_to_string(path)?;
        let trajectory: Trajectory = serde_json::from_str(&json)?;
        trajectory.validate()?;
        Ok(trajectory)
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, AtifError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize from JSON string
    pub fn from_json(json: &str) -> Result<Self, AtifError> {
        let trajectory: Trajectory = serde_json::from_str(json)?;
        trajectory.validate()?;
        Ok(trajectory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observation, ObservationResult, ToolCall};
    use serde_json::json;

    #[test]
    fn test_trajectory_creation() {
        let agent = Agent::new("test-agent", "1.0.0");
        let mut trajectory = Trajectory::v1_4("session-123", agent);

        trajectory.add_step(Step::user(1, "Hello"));
        trajectory.add_step(Step::agent(2, "Hi there!"));

        assert_eq!(trajectory.steps.len(), 2);
        assert_eq!(trajectory.schema_version, "ATIF-v1.4");
    }

    #[test]
    fn test_trajectory_validation_step_sequence() {
        let agent = Agent::new("test-agent", "1.0.0");
        let trajectory = Trajectory {
            schema_version: "ATIF-v1.4".to_string(),
            session_id: "test".to_string(),
            agent,
            steps: vec![
                Step::user(1, "First"),
                Step::user(3, "Third"), // Wrong ID
            ],
            notes: None,
            final_metrics: None,
            extra: None,
        };

        assert!(trajectory.validate().is_err());
    }

    #[test]
    fn test_trajectory_validation_tool_call_refs() {
        let agent = Agent::new("test-agent", "1.0.0");
        let trajectory = Trajectory {
            schema_version: "ATIF-v1.4".to_string(),
            session_id: "test".to_string(),
            agent,
            steps: vec![
                Step::agent(1, "Searching")
                    .with_tool_calls(vec![ToolCall::new("call_1", "search", json!({}))])
                    .with_observation(Observation::single(ObservationResult::with_content(
                        Some("call_999".to_string()), // Non-existent call ID
                        "Result",
                    ))),
            ],
            notes: None,
            final_metrics: None,
            extra: None,
        };

        assert!(trajectory.validate().is_err());
    }

    #[test]
    fn test_trajectory_serialization() {
        let agent = Agent::new("test-agent", "1.0.0");
        let trajectory = Trajectory::v1_4("session-123", agent).with_notes("Test trajectory");

        let json = trajectory.to_json().unwrap();
        let deserialized = Trajectory::from_json(&json).unwrap();

        assert_eq!(trajectory, deserialized);
    }
}
