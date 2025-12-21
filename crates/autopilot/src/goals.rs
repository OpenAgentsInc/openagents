//! Goal Tracking
//!
//! This module provides helpers for tracking agent goals and their progress.
//! Goals are persisted in agent state (kind:38001) and updated across execution cycles.

use anyhow::{Context, Result};
use nostr::{AgentStateContent, Goal, GoalStatus};

/// Goal manager for tracking agent goals
pub struct GoalManager {
    /// Agent state content
    state: AgentStateContent,
}

impl GoalManager {
    /// Create a new goal manager with existing state
    pub fn new(state: AgentStateContent) -> Self {
        Self { state }
    }

    /// Create a new goal manager with empty state
    pub fn new_empty() -> Self {
        Self {
            state: AgentStateContent::new(),
        }
    }

    /// Add a new goal
    pub fn add_goal(&mut self, id: impl Into<String>, description: impl Into<String>, priority: u32) -> &Goal {
        let goal = Goal::new(id, description, priority);
        self.state.add_goal(goal);
        self.state.goals.last().unwrap()
    }

    /// Update goal progress
    pub fn update_progress(&mut self, goal_id: &str, progress: f64) -> Result<()> {
        let goal = self.find_goal_mut(goal_id)
            .context(format!("Goal not found: {}", goal_id))?;
        goal.update_progress(progress);
        Ok(())
    }

    /// Pause a goal
    pub fn pause_goal(&mut self, goal_id: &str) -> Result<()> {
        let goal = self.find_goal_mut(goal_id)
            .context(format!("Goal not found: {}", goal_id))?;
        goal.pause();
        Ok(())
    }

    /// Resume a goal
    pub fn resume_goal(&mut self, goal_id: &str) -> Result<()> {
        let goal = self.find_goal_mut(goal_id)
            .context(format!("Goal not found: {}", goal_id))?;
        goal.resume();
        Ok(())
    }

    /// Cancel a goal
    pub fn cancel_goal(&mut self, goal_id: &str) -> Result<()> {
        let goal = self.find_goal_mut(goal_id)
            .context(format!("Goal not found: {}", goal_id))?;
        goal.cancel();
        Ok(())
    }

    /// Remove a goal by ID
    pub fn remove_goal(&mut self, goal_id: &str) -> Result<()> {
        let index = self.state.goals.iter().position(|g| g.id == goal_id)
            .context(format!("Goal not found: {}", goal_id))?;
        self.state.goals.remove(index);
        Ok(())
    }

    /// Get all goals
    pub fn get_goals(&self) -> &[Goal] {
        &self.state.goals
    }

    /// Get active goals only
    pub fn get_active_goals(&self) -> Vec<&Goal> {
        self.state.goals.iter()
            .filter(|g| g.status == GoalStatus::Active)
            .collect()
    }

    /// Get completed goals
    pub fn get_completed_goals(&self) -> Vec<&Goal> {
        self.state.goals.iter()
            .filter(|g| g.status == GoalStatus::Completed)
            .collect()
    }

    /// Get goal by ID
    pub fn get_goal(&self, goal_id: &str) -> Option<&Goal> {
        self.state.goals.iter().find(|g| g.id == goal_id)
    }

    /// Get overall progress (average of all active goals)
    pub fn get_overall_progress(&self) -> f64 {
        let active_goals: Vec<_> = self.get_active_goals();
        if active_goals.is_empty() {
            return 0.0;
        }

        let sum: f64 = active_goals.iter().map(|g| g.progress).sum();
        sum / active_goals.len() as f64
    }

    /// Associate a goal with an issue number
    pub fn link_goal_to_issue(&mut self, goal_id: &str, issue_number: u32) -> Result<()> {
        let goal = self.find_goal_mut(goal_id)
            .context(format!("Goal not found: {}", goal_id))?;

        // Store issue number in description or add metadata
        // For now, we'll update the description to include the issue reference
        if !goal.description.contains(&format!("#issue-{}", issue_number)) {
            goal.description = format!("{} #issue-{}", goal.description, issue_number);
        }

        Ok(())
    }

    /// Get the underlying state
    pub fn state(&self) -> &AgentStateContent {
        &self.state
    }

    /// Get mutable state
    pub fn state_mut(&mut self) -> &mut AgentStateContent {
        &mut self.state
    }

    /// Consume and return the state
    pub fn into_state(self) -> AgentStateContent {
        self.state
    }

    /// Find a goal by ID (mutable)
    fn find_goal_mut(&mut self, goal_id: &str) -> Option<&mut Goal> {
        self.state.goals.iter_mut().find(|g| g.id == goal_id)
    }

    /// Generate a progress report
    pub fn progress_report(&self) -> String {
        let total = self.state.goals.len();
        let active = self.get_active_goals().len();
        let completed = self.get_completed_goals().len();
        let overall_progress = self.get_overall_progress();

        format!(
            "Goals: {} total, {} active, {} completed\nOverall progress: {:.1}%",
            total,
            active,
            completed,
            overall_progress * 100.0
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_manager_creation() {
        let manager = GoalManager::new_empty();
        assert_eq!(manager.get_goals().len(), 0);
    }

    #[test]
    fn test_add_goal() {
        let mut manager = GoalManager::new_empty();
        let goal = manager.add_goal("goal-1", "Test goal", 1);
        assert_eq!(goal.id, "goal-1");
        assert_eq!(goal.description, "Test goal");
        assert_eq!(goal.priority, 1);
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(manager.get_goals().len(), 1);
    }

    #[test]
    fn test_update_progress() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal", 1);

        manager.update_progress("goal-1", 0.5).unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert_eq!(goal.progress, 0.5);

        manager.update_progress("goal-1", 1.0).unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert_eq!(goal.progress, 1.0);
        assert_eq!(goal.status, GoalStatus::Completed);
    }

    #[test]
    fn test_pause_resume_goal() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal", 1);

        manager.pause_goal("goal-1").unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert_eq!(goal.status, GoalStatus::Paused);

        manager.resume_goal("goal-1").unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
    }

    #[test]
    fn test_cancel_goal() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal", 1);

        manager.cancel_goal("goal-1").unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert_eq!(goal.status, GoalStatus::Cancelled);
    }

    #[test]
    fn test_remove_goal() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal", 1);
        assert_eq!(manager.get_goals().len(), 1);

        manager.remove_goal("goal-1").unwrap();
        assert_eq!(manager.get_goals().len(), 0);
    }

    #[test]
    fn test_get_active_goals() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal 1", 1);
        manager.add_goal("goal-2", "Test goal 2", 2);
        manager.add_goal("goal-3", "Test goal 3", 3);

        manager.pause_goal("goal-2").unwrap();
        manager.update_progress("goal-3", 1.0).unwrap();

        let active = manager.get_active_goals();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "goal-1");
    }

    #[test]
    fn test_get_completed_goals() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal 1", 1);
        manager.add_goal("goal-2", "Test goal 2", 2);

        manager.update_progress("goal-2", 1.0).unwrap();

        let completed = manager.get_completed_goals();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "goal-2");
    }

    #[test]
    fn test_overall_progress() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal 1", 1);
        manager.add_goal("goal-2", "Test goal 2", 2);
        manager.add_goal("goal-3", "Test goal 3", 3);

        manager.update_progress("goal-1", 0.2).unwrap();
        manager.update_progress("goal-2", 0.6).unwrap();
        manager.update_progress("goal-3", 0.4).unwrap();

        let overall = manager.get_overall_progress();
        assert!((overall - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_link_goal_to_issue() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal", 1);

        manager.link_goal_to_issue("goal-1", 123).unwrap();
        let goal = manager.get_goal("goal-1").unwrap();
        assert!(goal.description.contains("#issue-123"));
    }

    #[test]
    fn test_progress_report() {
        let mut manager = GoalManager::new_empty();
        manager.add_goal("goal-1", "Test goal 1", 1);
        manager.add_goal("goal-2", "Test goal 2", 2);
        manager.update_progress("goal-2", 1.0).unwrap();

        let report = manager.progress_report();
        assert!(report.contains("2 total"));
        assert!(report.contains("1 active"));
        assert!(report.contains("1 completed"));
    }

    #[test]
    fn test_goal_not_found() {
        let mut manager = GoalManager::new_empty();
        let result = manager.update_progress("nonexistent", 0.5);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Goal not found"));
    }
}
