//! Lanes for parallel execution in the timeline.

use crate::step::{Step, StepId};

/// Unique identifier for a lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaneId(pub u64);

impl LaneId {
    /// Create a new lane ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// A lane represents a parallel execution track in the timeline.
#[derive(Clone, Debug)]
pub struct Lane {
    /// Unique identifier.
    pub id: LaneId,
    /// Lane label (e.g., agent name).
    pub label: String,
    /// Steps in this lane (in order).
    pub steps: Vec<Step>,
    /// Whether this lane is collapsed.
    pub collapsed: bool,
    /// Lane height in pixels.
    pub height: f32,
}

impl Lane {
    /// Create a new lane.
    pub fn new(id: LaneId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            steps: Vec::new(),
            collapsed: false,
            height: 60.0,
        }
    }

    /// Add a step to this lane.
    pub fn add_step(&mut self, step: Step) {
        self.steps.push(step);
    }

    /// Get a step by ID.
    pub fn get_step(&self, id: StepId) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == id)
    }

    /// Get a mutable step by ID.
    pub fn get_step_mut(&mut self, id: StepId) -> Option<&mut Step> {
        self.steps.iter_mut().find(|s| s.id == id)
    }

    /// Remove a step by ID.
    pub fn remove_step(&mut self, id: StepId) -> Option<Step> {
        if let Some(pos) = self.steps.iter().position(|s| s.id == id) {
            Some(self.steps.remove(pos))
        } else {
            None
        }
    }

    /// Toggle collapsed state.
    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Get total duration of all steps in this lane.
    pub fn total_duration_ms(&self) -> u64 {
        self.steps.iter().filter_map(|s| s.duration_ms).sum()
    }

    /// Get the earliest start time in this lane.
    pub fn earliest_start(&self) -> Option<u64> {
        self.steps.iter().filter_map(|s| s.start_time).min()
    }

    /// Get the latest end time in this lane.
    pub fn latest_end(&self) -> Option<u64> {
        self.steps.iter().filter_map(|s| s.end_time).max()
    }

    /// Check if any step is currently running.
    pub fn has_running_step(&self) -> bool {
        self.steps.iter().any(|s| s.status.is_active())
    }

    /// Get the number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Calculate visual height based on content.
    pub fn calculate_height(&self, base_height: f32) -> f32 {
        if self.collapsed {
            base_height * 0.5
        } else {
            let expanded_steps = self.steps.iter().filter(|s| s.expanded).count();
            base_height + (expanded_steps as f32 * 80.0)
        }
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::new(LaneId::new(0), "Default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_domain::ids::RunId;

    #[test]
    fn test_lane_creation() {
        let lane = Lane::new(LaneId::new(1), "Agent 1");
        assert_eq!(lane.label, "Agent 1");
        assert!(lane.steps.is_empty());
    }

    #[test]
    fn test_lane_add_step() {
        let mut lane = Lane::new(LaneId::new(1), "Agent");
        let step = Step::new(StepId::new(1), RunId::new(), "Step 1");
        lane.add_step(step);

        assert_eq!(lane.step_count(), 1);
    }

    #[test]
    fn test_lane_get_step() {
        let mut lane = Lane::new(LaneId::new(1), "Agent");
        let step_id = StepId::new(42);
        let step = Step::new(step_id, RunId::new(), "Step");
        lane.add_step(step);

        assert!(lane.get_step(step_id).is_some());
        assert!(lane.get_step(StepId::new(999)).is_none());
    }

    #[test]
    fn test_lane_remove_step() {
        let mut lane = Lane::new(LaneId::new(1), "Agent");
        let step_id = StepId::new(1);
        lane.add_step(Step::new(step_id, RunId::new(), "Step"));

        let removed = lane.remove_step(step_id);
        assert!(removed.is_some());
        assert!(lane.steps.is_empty());
    }

    #[test]
    fn test_lane_collapse() {
        let mut lane = Lane::new(LaneId::new(1), "Agent");
        assert!(!lane.collapsed);

        lane.toggle_collapsed();
        assert!(lane.collapsed);
    }

    #[test]
    fn test_lane_duration() {
        let mut lane = Lane::new(LaneId::new(1), "Agent");

        let mut step1 = Step::new(StepId::new(1), RunId::new(), "Step 1");
        step1.start(0);
        step1.complete(1000, None);
        lane.add_step(step1);

        let mut step2 = Step::new(StepId::new(2), RunId::new(), "Step 2");
        step2.start(1000);
        step2.complete(3000, None);
        lane.add_step(step2);

        assert_eq!(lane.total_duration_ms(), 3000);
        assert_eq!(lane.earliest_start(), Some(0));
        assert_eq!(lane.latest_end(), Some(3000));
    }
}
