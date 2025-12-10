use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Metrics, Observation, ToolCall};

/// The originator of a step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StepSource {
    /// System prompts
    System,
    /// User messages
    User,
    /// Agent responses
    Agent,
}

/// Qualitative or quantitative measure of reasoning effort.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ReasoningEffort {
    /// Qualitative effort level (e.g., "low", "medium", "high")
    Qualitative(String),
    /// Quantitative effort score
    Quantitative(f64),
}

/// A single step in the trajectory representing either a system prompt, user message,
/// or complete agent turn (LLM inference, action execution, and observation receipt).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Step {
    /// Ordinal index of the turn (starting from 1)
    pub step_id: i64,

    /// ISO 8601 timestamp indicating when this step occurred
    ///
    /// Example: "2025-10-16T14:30:00Z"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,

    /// The originator of this step
    ///
    /// Must be one of: "system", "user", or "agent"
    pub source: StepSource,

    /// The specific LLM model used for this turn
    ///
    /// Example: "gemini-2.5-flash"
    /// Only applicable when source is "agent".
    /// If omitted, the model can be inferred from the top-level agent configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,

    /// Qualitative or quantitative measure of reasoning effort
    ///
    /// Examples: "low", "medium", "high", or a float score
    /// Only applicable when source is "agent".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// The dialogue message
    ///
    /// - For system steps: the system prompt
    /// - For user steps: the user's prompt or instruction
    /// - For agent steps: the assistant's response
    ///
    /// This field is required but can be an empty string.
    pub message: String,

    /// Explicit internal reasoning by the agent
    ///
    /// Only applicable when source is "agent".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,

    /// Array of structured tool/function calls
    ///
    /// A single LLM output may contain multiple tool calls.
    /// Only applicable when source is "agent".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Environment feedback/result after actions or system events
    ///
    /// For agent steps, contains results from tool calls, non-tool actions, or
    /// subagent delegation.
    ///
    /// For system steps, may contain results from system-initiated operations
    /// (e.g., subagent delegation, context management, environment reset, checkpoint creation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<Observation>,

    /// LLM operational and confidence data for this step
    ///
    /// Includes RL-specific fields (reward, log_probs) if applicable.
    /// Only applicable when source is "agent".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,

    /// Custom step-level metadata not covered by the core schema
    ///
    /// Applicable to all step types (system, user, and agent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl Step {
    /// Create a new user step with current timestamp
    pub fn user(step_id: i64, message: impl Into<String>) -> Self {
        Self {
            step_id,
            timestamp: Some(Utc::now()),
            source: StepSource::User,
            model_name: None,
            reasoning_effort: None,
            message: message.into(),
            reasoning_content: None,
            tool_calls: None,
            observation: None,
            metrics: None,
            extra: None,
        }
    }

    /// Create a new system step with current timestamp
    pub fn system(step_id: i64, message: impl Into<String>) -> Self {
        Self {
            step_id,
            timestamp: Some(Utc::now()),
            source: StepSource::System,
            model_name: None,
            reasoning_effort: None,
            message: message.into(),
            reasoning_content: None,
            tool_calls: None,
            observation: None,
            metrics: None,
            extra: None,
        }
    }

    /// Create a new agent step with current timestamp
    pub fn agent(step_id: i64, message: impl Into<String>) -> Self {
        Self {
            step_id,
            timestamp: Some(Utc::now()),
            source: StepSource::Agent,
            model_name: None,
            reasoning_effort: None,
            message: message.into(),
            reasoning_content: None,
            tool_calls: None,
            observation: None,
            metrics: None,
            extra: None,
        }
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set the model name (agent steps only)
    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = Some(model_name.into());
        self
    }

    /// Set reasoning effort (agent steps only)
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    /// Set reasoning content (agent steps only)
    pub fn with_reasoning_content(mut self, content: impl Into<String>) -> Self {
        self.reasoning_content = Some(content.into());
        self
    }

    /// Set tool calls (agent steps only)
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = Some(tool_calls);
        self
    }

    /// Set observation
    pub fn with_observation(mut self, observation: Observation) -> Self {
        self.observation = Some(observation);
        self
    }

    /// Set metrics (agent steps only)
    pub fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Set extra metadata
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }

    /// Validate that agent-only fields are not set on non-agent steps
    pub fn validate(&self) -> Result<(), String> {
        match self.source {
            StepSource::Agent => Ok(()),
            StepSource::User | StepSource::System => {
                if self.model_name.is_some() {
                    return Err("model_name is only applicable when source is 'agent'".to_string());
                }
                if self.reasoning_effort.is_some() {
                    return Err("reasoning_effort is only applicable when source is 'agent'".to_string());
                }
                if self.reasoning_content.is_some() {
                    return Err("reasoning_content is only applicable when source is 'agent'".to_string());
                }
                if self.tool_calls.is_some() {
                    return Err("tool_calls is only applicable when source is 'agent'".to_string());
                }
                if self.metrics.is_some() {
                    return Err("metrics is only applicable when source is 'agent'".to_string());
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_creation() {
        let step = Step::user(1, "Hello, agent!");
        assert_eq!(step.step_id, 1);
        assert_eq!(step.message, "Hello, agent!");
        assert_eq!(step.source, StepSource::User);
    }

    #[test]
    fn test_agent_step_validation() {
        let step = Step::agent(1, "Response")
            .with_model("claude-3-5-sonnet");
        assert!(step.validate().is_ok());
    }

    #[test]
    fn test_user_step_validation_fails_with_agent_fields() {
        let step = Step::user(1, "Hello")
            .with_model("claude-3-5-sonnet");
        assert!(step.validate().is_err());
    }
}
