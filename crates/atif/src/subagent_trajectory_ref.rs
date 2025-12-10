use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Reference to a delegated subagent trajectory.
///
/// For multi-agent systems or hierarchical agent architectures, an observation
/// result may reference a complete subagent trajectory. This enables tracking of
/// recursive or delegated agent workflows where a parent agent spawns subagents
/// to handle specific subtasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentTrajectoryRef {
    /// The session ID of the delegated subagent trajectory
    pub session_id: String,

    /// Reference to the complete subagent trajectory file
    ///
    /// Examples: file path, S3 URL, database reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trajectory_path: Option<String>,

    /// Custom metadata about the subagent execution
    ///
    /// Examples: summary, exit status, performance metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl SubagentTrajectoryRef {
    /// Create a new subagent trajectory reference
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            trajectory_path: None,
            extra: None,
        }
    }

    /// Set the trajectory path
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.trajectory_path = Some(path.into());
        self
    }

    /// Set extra metadata
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = Some(extra);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_ref_creation() {
        let ref_obj = SubagentTrajectoryRef::new("subagent-ABC123")
            .with_path("s3://trajectories/subagent-ABC123.json");

        assert_eq!(ref_obj.session_id, "subagent-ABC123");
        assert_eq!(
            ref_obj.trajectory_path,
            Some("s3://trajectories/subagent-ABC123.json".to_string())
        );
    }
}
