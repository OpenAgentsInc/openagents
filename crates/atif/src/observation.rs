use crate::ObservationResult;
use serde::{Deserialize, Serialize};

/// Environment feedback or results from tool executions and actions.
///
/// For agent steps, results may stem from structured tool_calls, agent actions
/// that don't use standard tool calling mechanisms, or subagent delegation.
///
/// For system steps, observations may contain results from system-initiated
/// operations such as subagent delegation, context management, environment resets,
/// checkpoint creation, or other infrastructure-level events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    /// Array of result objects, each containing feedback from a single tool call or action
    pub results: Vec<ObservationResult>,
}

impl Observation {
    /// Create a new observation with results
    pub fn new(results: Vec<ObservationResult>) -> Self {
        Self { results }
    }

    /// Create an observation with a single result
    pub fn single(result: ObservationResult) -> Self {
        Self {
            results: vec![result],
        }
    }

    /// Create an empty observation
    pub fn empty() -> Self {
        Self {
            results: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_creation() {
        let result = ObservationResult::with_content(Some("call_1".to_string()), "Test result");
        let observation = Observation::single(result);

        assert_eq!(observation.results.len(), 1);
    }
}
