//! Step representation in the timeline.

use coder_domain::ids::RunId;
use wgpui::Hsla;

/// Unique identifier for a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StepId(pub u64);

impl StepId {
    /// Create a new step ID.
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Status of a step in the workflow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepStatus {
    /// Step is waiting to start.
    #[default]
    Pending,
    /// Step is currently running.
    Running,
    /// Step completed successfully.
    Completed,
    /// Step failed with an error.
    Failed,
    /// Step was cancelled.
    Cancelled,
    /// Step was skipped.
    Skipped,
}

impl StepStatus {
    /// Get the color for this status.
    pub fn color(&self) -> Hsla {
        match self {
            StepStatus::Pending => Hsla::new(0.0, 0.0, 0.4, 1.0),
            StepStatus::Running => Hsla::new(200.0 / 360.0, 0.8, 0.5, 1.0),
            StepStatus::Completed => Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0),
            StepStatus::Failed => Hsla::new(0.0, 0.7, 0.5, 1.0),
            StepStatus::Cancelled => Hsla::new(30.0 / 360.0, 0.6, 0.5, 1.0),
            StepStatus::Skipped => Hsla::new(0.0, 0.0, 0.5, 1.0),
        }
    }

    /// Get icon/symbol for this status.
    pub fn icon(&self) -> &'static str {
        match self {
            StepStatus::Pending => "○",
            StepStatus::Running => "◉",
            StepStatus::Completed => "✓",
            StepStatus::Failed => "✗",
            StepStatus::Cancelled => "⊘",
            StepStatus::Skipped => "↷",
        }
    }

    /// Check if the step is terminal (won't change).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepStatus::Completed | StepStatus::Failed | StepStatus::Cancelled | StepStatus::Skipped
        )
    }

    /// Check if step is active.
    pub fn is_active(&self) -> bool {
        matches!(self, StepStatus::Running)
    }
}

/// A step in a workflow run.
#[derive(Clone, Debug)]
pub struct Step {
    /// Unique identifier.
    pub id: StepId,
    /// Parent run ID.
    pub run_id: RunId,
    /// Step name/label.
    pub name: String,
    /// Step description.
    pub description: Option<String>,
    /// Current status.
    pub status: StepStatus,
    /// Start time (ms since epoch).
    pub start_time: Option<u64>,
    /// End time (ms since epoch).
    pub end_time: Option<u64>,
    /// Duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Progress (0.0 to 1.0) for running steps.
    pub progress: Option<f32>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Output/result summary.
    pub output: Option<String>,
    /// Artifacts produced by this step.
    pub artifacts: Vec<String>,
    /// Whether this step is expanded in the UI.
    pub expanded: bool,
}

impl Step {
    /// Create a new pending step.
    pub fn new(id: StepId, run_id: RunId, name: impl Into<String>) -> Self {
        Self {
            id,
            run_id,
            name: name.into(),
            description: None,
            status: StepStatus::Pending,
            start_time: None,
            end_time: None,
            duration_ms: None,
            progress: None,
            error: None,
            output: None,
            artifacts: Vec::new(),
            expanded: false,
        }
    }

    /// Set the step description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark step as running.
    pub fn start(&mut self, time: u64) {
        self.status = StepStatus::Running;
        self.start_time = Some(time);
    }

    /// Mark step as completed.
    pub fn complete(&mut self, time: u64, output: Option<String>) {
        self.status = StepStatus::Completed;
        self.end_time = Some(time);
        self.output = output;
        self.calculate_duration();
    }

    /// Mark step as failed.
    pub fn fail(&mut self, time: u64, error: impl Into<String>) {
        self.status = StepStatus::Failed;
        self.end_time = Some(time);
        self.error = Some(error.into());
        self.calculate_duration();
    }

    /// Mark step as cancelled.
    pub fn cancel(&mut self, time: u64) {
        self.status = StepStatus::Cancelled;
        self.end_time = Some(time);
        self.calculate_duration();
    }

    /// Mark step as skipped.
    pub fn skip(&mut self) {
        self.status = StepStatus::Skipped;
    }

    /// Update progress for a running step.
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = Some(progress.clamp(0.0, 1.0));
    }

    /// Add an artifact.
    pub fn add_artifact(&mut self, artifact: impl Into<String>) {
        self.artifacts.push(artifact.into());
    }

    /// Toggle expanded state.
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Calculate duration from start/end times.
    fn calculate_duration(&mut self) {
        if let (Some(start), Some(end)) = (self.start_time, self.end_time) {
            self.duration_ms = Some(end.saturating_sub(start));
        }
    }

    /// Get formatted duration string.
    pub fn formatted_duration(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms < 1000 {
                format!("{}ms", ms)
            } else if ms < 60000 {
                format!("{:.1}s", ms as f64 / 1000.0)
            } else {
                let mins = ms / 60000;
                let secs = (ms % 60000) / 1000;
                format!("{}m {}s", mins, secs)
            }
        })
    }

    /// Get the visual width for this step (based on duration).
    pub fn visual_width(&self, scale: f32) -> f32 {
        // Minimum width for visibility
        let min_width = 60.0;
        // Maximum width to prevent overflow
        let max_width = 400.0;

        match self.duration_ms {
            Some(ms) => {
                let width = (ms as f32 * scale).max(min_width);
                width.min(max_width)
            }
            None => {
                // Running or pending - use minimum width
                min_width
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_creation() {
        let step = Step::new(StepId::new(1), RunId::new(), "Test Step");
        assert_eq!(step.status, StepStatus::Pending);
        assert_eq!(step.name, "Test Step");
    }

    #[test]
    fn test_step_lifecycle() {
        let mut step = Step::new(StepId::new(1), RunId::new(), "Test");

        // Start
        step.start(1000);
        assert_eq!(step.status, StepStatus::Running);
        assert_eq!(step.start_time, Some(1000));

        // Complete
        step.complete(2500, Some("Done".into()));
        assert_eq!(step.status, StepStatus::Completed);
        assert_eq!(step.duration_ms, Some(1500));
    }

    #[test]
    fn test_step_failure() {
        let mut step = Step::new(StepId::new(1), RunId::new(), "Test");
        step.start(1000);
        step.fail(1500, "Something went wrong");

        assert_eq!(step.status, StepStatus::Failed);
        assert!(step.error.is_some());
    }

    #[test]
    fn test_status_colors() {
        // Just verify each status has a color
        for status in [
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Cancelled,
            StepStatus::Skipped,
        ] {
            let color = status.color();
            assert!(color.a > 0.0);
        }
    }

    #[test]
    fn test_formatted_duration() {
        let mut step = Step::new(StepId::new(1), RunId::new(), "Test");
        step.duration_ms = Some(500);
        assert_eq!(step.formatted_duration(), Some("500ms".into()));

        step.duration_ms = Some(2500);
        assert_eq!(step.formatted_duration(), Some("2.5s".into()));

        step.duration_ms = Some(125000);
        assert_eq!(step.formatted_duration(), Some("2m 5s".into()));
    }

    #[test]
    fn test_progress() {
        let mut step = Step::new(StepId::new(1), RunId::new(), "Test");
        step.set_progress(0.5);
        assert_eq!(step.progress, Some(0.5));

        // Clamp to range
        step.set_progress(1.5);
        assert_eq!(step.progress, Some(1.0));
    }
}
