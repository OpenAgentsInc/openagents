//! Configuration for Adjutant plan mode pipeline.

/// Plan Mode Pipeline Configuration.
#[derive(Debug, Clone)]
pub struct PlanModeConfig {
    /// Maximum number of exploration topics (2-4).
    pub max_topics: usize,
    /// Maximum tool calls per exploration agent.
    pub max_tool_calls_per_agent: usize,
    /// Enable deep planning for complex tasks.
    pub enable_deep_planning: bool,
    /// Complexity threshold for deep planning (0.0-1.0).
    pub deep_planning_threshold: f32,
    /// Enable result validation.
    pub enable_validation: bool,
}

impl Default for PlanModeConfig {
    fn default() -> Self {
        Self {
            max_topics: 4,
            max_tool_calls_per_agent: 8,
            enable_deep_planning: true,
            deep_planning_threshold: 0.7,
            enable_validation: true,
        }
    }
}
