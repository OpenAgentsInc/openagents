use crate::SubagentTrajectoryRef;
use serde::{Deserialize, Serialize};

/// Individual result from a tool execution or action.
///
/// Each element in the observation results array follows this schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationResult {
    /// The `tool_call_id` from the tool_calls array that this result corresponds to
    ///
    /// If null or omitted, the result comes from an action that doesn't use the
    /// standard tool calling format (e.g., agent actions without tool calls or
    /// system-initiated operations).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_call_id: Option<String>,

    /// The textual output or result from the tool execution or action
    ///
    /// May be omitted when `subagent_trajectory_ref` is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// References to delegated subagent trajectories
    ///
    /// Use a singleton array for a single subagent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_trajectory_ref: Option<Vec<SubagentTrajectoryRef>>,
}

impl ObservationResult {
    /// Create a new observation result with content
    pub fn with_content(source_call_id: Option<String>, content: impl Into<String>) -> Self {
        Self {
            source_call_id,
            content: Some(content.into()),
            subagent_trajectory_ref: None,
        }
    }

    /// Create a new observation result referencing a subagent
    pub fn with_subagent(
        source_call_id: Option<String>,
        subagent_refs: Vec<SubagentTrajectoryRef>,
    ) -> Self {
        Self {
            source_call_id,
            content: None,
            subagent_trajectory_ref: Some(subagent_refs),
        }
    }

    /// Create a new observation result with both content and subagent reference
    ///
    /// The content may serve as a summary without loading the full subagent trajectory.
    pub fn with_both(
        source_call_id: Option<String>,
        content: impl Into<String>,
        subagent_refs: Vec<SubagentTrajectoryRef>,
    ) -> Self {
        Self {
            source_call_id,
            content: Some(content.into()),
            subagent_trajectory_ref: Some(subagent_refs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_result_with_content() {
        let result = ObservationResult::with_content(
            Some("call_1".to_string()),
            "GOOGL is currently trading at $185.35",
        );

        assert_eq!(result.source_call_id, Some("call_1".to_string()));
        assert!(result.content.is_some());
        assert!(result.subagent_trajectory_ref.is_none());
    }

    #[test]
    fn test_observation_result_with_subagent() {
        let subagent = SubagentTrajectoryRef::new("subagent-123");
        let result =
            ObservationResult::with_subagent(Some("call_delegate_1".to_string()), vec![subagent]);

        assert_eq!(result.source_call_id, Some("call_delegate_1".to_string()));
        assert!(result.content.is_none());
        assert!(result.subagent_trajectory_ref.is_some());
    }
}
