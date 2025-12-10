use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Agent configuration identifying the agent system used for the trajectory.
///
/// ## Example
///
/// ```
/// use atif::Agent;
///
/// let agent = Agent {
///     name: "harbor-agent".to_string(),
///     version: "1.0.0".to_string(),
///     model_name: Some("gemini-2.5-flash".to_string()),
///     extra: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Agent {
    /// The name of the agent system (e.g., "openhands", "claude-code", "mini-swe-agent")
    pub name: String,

    /// The version identifier of the agent system (e.g., "1.0.0", "v2.3.1")
    pub version: String,

    /// Default LLM model used for this trajectory
    ///
    /// Examples: "gemini-2.5-flash", "claude-3-5-sonnet"
    /// Step-level model_name overrides this if specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,

    /// Custom agent configuration details not covered by the core schema
    ///
    /// Use for prompting strategy, custom parameters, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

impl Agent {
    /// Create a new agent configuration with minimal required fields
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            model_name: None,
            extra: None,
        }
    }

    /// Set the default model name
    pub fn with_model(mut self, model_name: impl Into<String>) -> Self {
        self.model_name = Some(model_name.into());
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
    fn test_agent_creation() {
        let agent = Agent::new("test-agent", "1.0.0")
            .with_model("claude-3-5-sonnet");

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.version, "1.0.0");
        assert_eq!(agent.model_name, Some("claude-3-5-sonnet".to_string()));
    }

    #[test]
    fn test_agent_serialization() {
        let agent = Agent::new("test", "1.0");
        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: Agent = serde_json::from_str(&json).unwrap();
        assert_eq!(agent, deserialized);
    }
}
