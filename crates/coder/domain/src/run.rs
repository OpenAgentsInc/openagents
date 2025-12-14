//! Run entity for workflow executions.
//!
//! A Run represents a single execution of a workflow, tracking
//! the status of each step and associated costs.

use crate::ids::{ArtifactId, RunId, StepId, WorkflowId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// The status of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Run is queued but not yet started.
    Queued,
    /// Run is currently executing.
    Running,
    /// Run completed successfully.
    Success,
    /// Run failed.
    Failed,
    /// Run was cancelled.
    Cancelled,
    /// Run is waiting for user approval.
    WaitingForApproval,
}

impl RunStatus {
    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            RunStatus::Success | RunStatus::Failed | RunStatus::Cancelled
        )
    }

    /// Returns true if the run is currently active.
    pub fn is_active(&self) -> bool {
        matches!(self, RunStatus::Running | RunStatus::WaitingForApproval)
    }
}

/// The status of a step within a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step is pending execution.
    Pending,
    /// Step is currently executing.
    Running,
    /// Step completed successfully.
    Success,
    /// Step failed.
    Failed,
    /// Step was skipped.
    Skipped,
    /// Step is waiting for approval.
    WaitingForApproval,
}

impl StepStatus {
    /// Returns true if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepStatus::Success | StepStatus::Failed | StepStatus::Skipped
        )
    }
}

/// Cost summary for a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostSummary {
    /// Total input tokens used.
    pub input_tokens: u64,
    /// Total output tokens used.
    pub output_tokens: u64,
    /// Estimated cost in USD cents.
    pub cost_cents: u64,
}

impl CostSummary {
    /// Create a new cost summary.
    pub fn new(input_tokens: u64, output_tokens: u64, cost_cents: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cost_cents,
        }
    }

    /// Add another cost summary to this one.
    pub fn add(&mut self, other: &CostSummary) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cost_cents += other.cost_cents;
    }

    /// Total tokens (input + output).
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// A step execution within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRun {
    /// Unique identifier for this step execution.
    pub id: StepId,

    /// Name of the step.
    pub name: String,

    /// Current status.
    pub status: StepStatus,

    /// Log output from the step.
    pub log: String,

    /// Artifacts produced by this step.
    pub artifacts: SmallVec<[ArtifactId; 4]>,

    /// When the step started.
    pub started_at: Option<DateTime<Utc>>,

    /// When the step finished.
    pub finished_at: Option<DateTime<Utc>>,

    /// Cost for this step.
    pub cost: CostSummary,
}

impl StepRun {
    /// Create a new pending step.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: StepId::new(),
            name: name.into(),
            status: StepStatus::Pending,
            log: String::new(),
            artifacts: SmallVec::new(),
            started_at: None,
            finished_at: None,
            cost: CostSummary::default(),
        }
    }

    /// Start the step.
    pub fn start(&mut self) {
        self.status = StepStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Append to the step log.
    pub fn append_log(&mut self, text: &str) {
        self.log.push_str(text);
    }

    /// Mark the step as successful.
    pub fn succeed(&mut self) {
        self.status = StepStatus::Success;
        self.finished_at = Some(Utc::now());
    }

    /// Mark the step as failed.
    pub fn fail(&mut self) {
        self.status = StepStatus::Failed;
        self.finished_at = Some(Utc::now());
    }

    /// Add an artifact to this step.
    pub fn add_artifact(&mut self, artifact_id: ArtifactId) {
        self.artifacts.push(artifact_id);
    }

    /// Get the duration of this step.
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}

/// A workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Unique identifier for this run.
    pub id: RunId,

    /// The workflow this run is executing.
    pub workflow_id: WorkflowId,

    /// Current status.
    pub status: RunStatus,

    /// Step executions within this run.
    pub steps: Vec<StepRun>,

    /// When the run started.
    pub started_at: DateTime<Utc>,

    /// When the run finished.
    pub finished_at: Option<DateTime<Utc>>,

    /// Aggregate cost for the run.
    pub cost: CostSummary,
}

impl Run {
    /// Create a new run.
    pub fn new(workflow_id: WorkflowId) -> Self {
        Self {
            id: RunId::new(),
            workflow_id,
            status: RunStatus::Queued,
            steps: Vec::new(),
            started_at: Utc::now(),
            finished_at: None,
            cost: CostSummary::default(),
        }
    }

    /// Start the run.
    pub fn start(&mut self) {
        self.status = RunStatus::Running;
    }

    /// Add a step to the run.
    pub fn add_step(&mut self, step: StepRun) {
        self.steps.push(step);
    }

    /// Get a step by ID.
    pub fn get_step(&self, step_id: StepId) -> Option<&StepRun> {
        self.steps.iter().find(|s| s.id == step_id)
    }

    /// Get a mutable step by ID.
    pub fn get_step_mut(&mut self, step_id: StepId) -> Option<&mut StepRun> {
        self.steps.iter_mut().find(|s| s.id == step_id)
    }

    /// Mark the run as successful.
    pub fn succeed(&mut self) {
        self.status = RunStatus::Success;
        self.finished_at = Some(Utc::now());
        self.recalculate_cost();
    }

    /// Mark the run as failed.
    pub fn fail(&mut self) {
        self.status = RunStatus::Failed;
        self.finished_at = Some(Utc::now());
        self.recalculate_cost();
    }

    /// Cancel the run.
    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
        self.finished_at = Some(Utc::now());
        self.recalculate_cost();
    }

    /// Recalculate the aggregate cost from all steps.
    pub fn recalculate_cost(&mut self) {
        self.cost = CostSummary::default();
        for step in &self.steps {
            self.cost.add(&step.cost);
        }
    }

    /// Get the duration of this run.
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.finished_at.map(|end| end - self.started_at)
    }

    /// Get the current step (first non-terminal step).
    pub fn current_step(&self) -> Option<&StepRun> {
        self.steps.iter().find(|s| !s.status.is_terminal())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_lifecycle() {
        let workflow_id = WorkflowId::new();
        let mut run = Run::new(workflow_id);
        assert_eq!(run.status, RunStatus::Queued);

        run.start();
        assert_eq!(run.status, RunStatus::Running);

        let mut step = StepRun::new("Build");
        step.start();
        step.succeed();
        run.add_step(step);

        run.succeed();
        assert_eq!(run.status, RunStatus::Success);
        assert!(run.finished_at.is_some());
    }

    #[test]
    fn test_cost_summary() {
        let mut cost1 = CostSummary::new(100, 50, 10);
        let cost2 = CostSummary::new(200, 100, 20);

        cost1.add(&cost2);
        assert_eq!(cost1.input_tokens, 300);
        assert_eq!(cost1.output_tokens, 150);
        assert_eq!(cost1.cost_cents, 30);
        assert_eq!(cost1.total_tokens(), 450);
    }
}
