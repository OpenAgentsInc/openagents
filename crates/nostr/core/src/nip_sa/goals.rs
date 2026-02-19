//! Agent Goals Event (kind:39203)
//!
//! For agents that want to expose their goals publicly (for transparency or
//! coordination), a separate goals event can be published.
//!
//! This is optional - goals can also be kept private in the encrypted state
//! event (kind:39201). Public goals enable coordination with other agents and
//! build trust with humans.
//!
//! ## Tags
//!
//! - `["d", "goals"]` - Addressable event marker
//!
//! ## Content
//!
//! Array of public goals (same structure as goals in state event):
//!
//! ```json
//! [
//!   {
//!     "id": "goal-1",
//!     "description": "Post interesting content about Bitcoin daily",
//!     "priority": 1,
//!     "created_at": 1703000000,
//!     "status": "active",
//!     "progress": 0.3
//!   }
//! ]
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-export Goal type from state module (GoalStatus also available via state module)
pub use super::state::{Goal, GoalStatus};

/// Kind for public goals event
pub const KIND_PUBLIC_GOALS: u16 = 39203;

/// Errors that can occur during NIP-SA public goals operations
#[derive(Debug, Error)]
pub enum PublicGoalsError {
    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("invalid goal: {0}")]
    InvalidGoal(String),
}

/// Public goals content (stored in content field)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicGoalsContent {
    /// List of public goals
    pub goals: Vec<Goal>,
}

impl PublicGoalsContent {
    /// Create new public goals content
    pub fn new() -> Self {
        Self { goals: Vec::new() }
    }

    /// Create public goals content with goals
    pub fn with_goals(goals: Vec<Goal>) -> Self {
        Self { goals }
    }

    /// Add a goal
    pub fn add_goal(mut self, goal: Goal) -> Self {
        self.goals.push(goal);
        self
    }

    /// Filter active goals
    pub fn active_goals(&self) -> Vec<&Goal> {
        self.goals
            .iter()
            .filter(|g| g.status == GoalStatus::Active)
            .collect()
    }

    /// Filter by priority (lower number = higher priority)
    pub fn goals_by_priority(&self) -> Vec<&Goal> {
        let mut goals: Vec<&Goal> = self.goals.iter().collect();
        goals.sort_by_key(|g| g.priority);
        goals
    }

    /// Serialize to JSON string
    pub fn to_json(&self) -> Result<String, PublicGoalsError> {
        serde_json::to_string(self).map_err(|e| PublicGoalsError::Serialization(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, PublicGoalsError> {
        serde_json::from_str(json).map_err(|e| PublicGoalsError::Deserialization(e.to_string()))
    }
}

impl Default for PublicGoalsContent {
    fn default() -> Self {
        Self::new()
    }
}

/// Public goals event wrapper
#[derive(Debug, Clone)]
pub struct PublicGoals {
    /// Goals content
    pub content: PublicGoalsContent,
}

impl PublicGoals {
    /// Create new public goals event
    pub fn new(content: PublicGoalsContent) -> Self {
        Self { content }
    }

    /// Build tags for the event
    pub fn build_tags(&self) -> Vec<Vec<String>> {
        vec![vec!["d".to_string(), "goals".to_string()]]
    }

    /// Validate the public goals
    pub fn validate(&self) -> Result<(), PublicGoalsError> {
        // Goals can be empty (agent hasn't set any public goals yet)
        // Individual goals are already validated via their own methods
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_goals_content_creation() {
        let content = PublicGoalsContent::new();
        assert_eq!(content.goals.len(), 0);
    }

    #[test]
    fn test_public_goals_content_with_goals() {
        let goal1 = Goal::new("goal-1", "First goal", 1);
        let goal2 = Goal::new("goal-2", "Second goal", 2);

        let content = PublicGoalsContent::with_goals(vec![goal1, goal2]);
        assert_eq!(content.goals.len(), 2);
    }

    #[test]
    fn test_public_goals_content_add_goal() {
        let content = PublicGoalsContent::new()
            .add_goal(Goal::new("goal-1", "First goal", 1))
            .add_goal(Goal::new("goal-2", "Second goal", 2));

        assert_eq!(content.goals.len(), 2);
    }

    #[test]
    fn test_public_goals_content_active_goals() {
        let goal1 = Goal::new("goal-1", "Active goal", 1);
        let mut goal2 = Goal::new("goal-2", "Paused goal", 2);
        goal2.pause();
        let goal3 = Goal::new("goal-3", "Another active", 3);

        let content = PublicGoalsContent::with_goals(vec![goal1, goal2, goal3]);
        let active = content.active_goals();

        assert_eq!(active.len(), 2);
        assert_eq!(active[0].id, "goal-1");
        assert_eq!(active[1].id, "goal-3");
    }

    #[test]
    fn test_public_goals_content_goals_by_priority() {
        let goal1 = Goal::new("goal-1", "Low priority", 3);
        let goal2 = Goal::new("goal-2", "High priority", 1);
        let goal3 = Goal::new("goal-3", "Medium priority", 2);

        let content = PublicGoalsContent::with_goals(vec![goal1, goal2, goal3]);
        let sorted = content.goals_by_priority();

        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].priority, 1);
        assert_eq!(sorted[1].priority, 2);
        assert_eq!(sorted[2].priority, 3);
    }

    #[test]
    fn test_public_goals_content_serialization() {
        let content = PublicGoalsContent::new().add_goal(Goal::new("goal-1", "Test goal", 1));

        let json = content.to_json().unwrap();
        let parsed = PublicGoalsContent::from_json(&json).unwrap();

        assert_eq!(parsed.goals.len(), 1);
        assert_eq!(parsed.goals[0].id, "goal-1");
    }

    #[test]
    fn test_public_goals_creation() {
        let content = PublicGoalsContent::new();
        let goals = PublicGoals::new(content);

        assert_eq!(goals.content.goals.len(), 0);
    }

    #[test]
    fn test_public_goals_tags() {
        let content = PublicGoalsContent::new();
        let goals = PublicGoals::new(content);
        let tags = goals.build_tags();

        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["d", "goals"]);
    }

    #[test]
    fn test_public_goals_validation() {
        let content = PublicGoalsContent::new();
        let goals = PublicGoals::new(content);

        assert!(goals.validate().is_ok());
    }

    #[test]
    fn test_public_goals_validation_with_goals() {
        let content = PublicGoalsContent::new()
            .add_goal(Goal::new("goal-1", "Test goal", 1))
            .add_goal(Goal::new("goal-2", "Another goal", 2));
        let goals = PublicGoals::new(content);

        assert!(goals.validate().is_ok());
    }
}
