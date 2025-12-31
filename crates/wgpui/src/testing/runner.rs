//! Test runner with state machine and playback control.
//!
//! Manages test execution, step timing, and playback speed.

use crate::testing::assertion::AssertionResult;
use crate::testing::step::TestStep;
use std::time::{Duration, Instant};

/// State of the test runner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerState {
    /// Test has not started.
    Idle,
    /// Test is actively executing.
    Running,
    /// Test is paused by user.
    Paused,
    /// Single-step mode - advance one step at a time.
    Stepping,
    /// Test completed successfully.
    Passed,
    /// Test failed (assertion or action failed).
    Failed,
    /// Test was aborted by user.
    Aborted,
}

impl RunnerState {
    /// Returns true if the test is finished (passed, failed, or aborted).
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Aborted)
    }

    /// Returns true if the test is currently active (running or stepping).
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Stepping)
    }

    /// Get a display label for this state.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Running => "RUNNING",
            Self::Paused => "PAUSED",
            Self::Stepping => "STEPPING",
            Self::Passed => "PASSED",
            Self::Failed => "FAILED",
            Self::Aborted => "ABORTED",
        }
    }
}

/// Playback speed multiplier.
#[derive(Clone, Copy, Debug)]
pub struct PlaybackSpeed(pub f32);

impl PlaybackSpeed {
    /// Slow playback (0.5x).
    pub const SLOW: Self = Self(0.5);
    /// Normal playback (1.0x).
    pub const NORMAL: Self = Self(1.0);
    /// Fast playback (2.0x).
    pub const FAST: Self = Self(2.0);
    /// Instant playback (10.0x).
    pub const INSTANT: Self = Self(10.0);

    /// Create a custom speed.
    pub fn custom(speed: f32) -> Self {
        Self(speed.max(0.1).min(100.0))
    }

    /// Get the speed multiplier.
    pub fn multiplier(&self) -> f32 {
        self.0
    }

    /// Scale a duration by this speed.
    pub fn scale(&self, duration: Duration) -> Duration {
        Duration::from_secs_f64(duration.as_secs_f64() / self.0 as f64)
    }
}

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// Result of a single step execution.
#[derive(Clone, Debug)]
pub struct StepResult {
    /// Index of the step.
    pub step_index: usize,
    /// Duration the step took.
    pub duration: Duration,
    /// Assertion result if this was an assertion step.
    pub assertion: Option<AssertionResult>,
    /// Error message if the step failed.
    pub error: Option<String>,
}

impl StepResult {
    /// Returns true if the step succeeded.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
            && self
                .assertion
                .as_ref()
                .map(|a| a.is_passed())
                .unwrap_or(true)
    }
}

/// Test runner that executes test steps.
pub struct TestRunner {
    /// Name of the test.
    name: String,
    /// Steps to execute.
    steps: Vec<TestStep>,
    /// Current step index.
    current_step: usize,
    /// Current state.
    state: RunnerState,
    /// Playback speed.
    speed: PlaybackSpeed,
    /// When the current step started.
    step_started: Option<Instant>,
    /// Results from executed steps.
    results: Vec<StepResult>,
    /// Error message if the test failed.
    error: Option<String>,
}

impl TestRunner {
    /// Create a new test runner with a name and steps.
    pub fn new(name: impl Into<String>, steps: Vec<TestStep>) -> Self {
        Self {
            name: name.into(),
            steps,
            current_step: 0,
            state: RunnerState::Idle,
            speed: PlaybackSpeed::NORMAL,
            step_started: None,
            results: Vec::new(),
            error: None,
        }
    }

    /// Get the test name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the current state.
    pub fn state(&self) -> RunnerState {
        self.state
    }

    /// Get the current step index.
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Get the total number of steps.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Get the current step, if any.
    pub fn current_step_ref(&self) -> Option<&TestStep> {
        self.steps.get(self.current_step)
    }

    /// Get all steps.
    pub fn steps(&self) -> &[TestStep] {
        &self.steps
    }

    /// Get the playback speed.
    pub fn speed(&self) -> PlaybackSpeed {
        self.speed
    }

    /// Set the playback speed.
    pub fn set_speed(&mut self, speed: PlaybackSpeed) {
        self.speed = speed;
    }

    /// Get results from executed steps.
    pub fn results(&self) -> &[StepResult] {
        &self.results
    }

    /// Get the error message if the test failed.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Start the test.
    pub fn start(&mut self) {
        if self.state == RunnerState::Idle {
            self.state = RunnerState::Running;
            self.step_started = Some(Instant::now());
        }
    }

    /// Pause the test.
    pub fn pause(&mut self) {
        if self.state == RunnerState::Running {
            self.state = RunnerState::Paused;
        }
    }

    /// Resume the test from pause.
    pub fn resume(&mut self) {
        if self.state == RunnerState::Paused {
            self.state = RunnerState::Running;
        }
    }

    /// Toggle play/pause.
    pub fn toggle_pause(&mut self) {
        match self.state {
            RunnerState::Running => self.pause(),
            RunnerState::Paused => self.resume(),
            RunnerState::Idle => self.start(),
            _ => {}
        }
    }

    /// Enter single-step mode.
    pub fn step_mode(&mut self) {
        if !self.state.is_finished() {
            self.state = RunnerState::Stepping;
        }
    }

    /// Execute the next step (in stepping mode).
    pub fn step_forward(&mut self) {
        if self.state == RunnerState::Stepping && self.current_step < self.steps.len() {
            // Mark the step as starting
            self.step_started = Some(Instant::now());
            // The harness will call complete_step when done
        }
    }

    /// Abort the test.
    pub fn abort(&mut self) {
        if !self.state.is_finished() {
            self.state = RunnerState::Aborted;
        }
    }

    /// Reset the test to initial state.
    pub fn reset(&mut self) {
        self.current_step = 0;
        self.state = RunnerState::Idle;
        self.step_started = None;
        self.results.clear();
        self.error = None;
    }

    /// Complete the current step with a result.
    pub fn complete_step(&mut self, result: StepResult) {
        // If step failed, transition to failed state
        if !result.is_success() {
            self.error = result
                .error
                .clone()
                .or_else(|| result.assertion.as_ref().map(|a| format!("{}", a)));
            self.state = RunnerState::Failed;
        }

        self.results.push(result);
        self.current_step += 1;

        // Check if we're done
        if self.current_step >= self.steps.len() && self.state != RunnerState::Failed {
            self.state = RunnerState::Passed;
        }

        // Reset step timer for next step
        self.step_started = Some(Instant::now());
    }

    /// Get elapsed time since current step started.
    pub fn step_elapsed(&self) -> Duration {
        self.step_started
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Get progress as a float (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        if self.steps.is_empty() {
            1.0
        } else {
            self.current_step as f32 / self.steps.len() as f32
        }
    }

    /// Get a progress string like "Step 3/10".
    pub fn progress_string(&self) -> String {
        format!("Step {}/{}", self.current_step + 1, self.steps.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MouseButton;
    use crate::testing::step::{ClickTarget, ElementSelector};

    fn test_steps() -> Vec<TestStep> {
        vec![
            TestStep::Click {
                target: ClickTarget::from_selector("#button"),
                button: MouseButton::Left,
            },
            TestStep::Wait {
                duration: Duration::from_millis(100),
            },
            TestStep::Expect {
                selector: ElementSelector::Id(42),
            },
        ]
    }

    #[test]
    fn test_runner_initial_state() {
        let runner = TestRunner::new("Test", test_steps());
        assert_eq!(runner.state(), RunnerState::Idle);
        assert_eq!(runner.current_step(), 0);
        assert_eq!(runner.total_steps(), 3);
    }

    #[test]
    fn test_runner_start() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.start();
        assert_eq!(runner.state(), RunnerState::Running);
    }

    #[test]
    fn test_runner_pause_resume() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.start();
        runner.pause();
        assert_eq!(runner.state(), RunnerState::Paused);
        runner.resume();
        assert_eq!(runner.state(), RunnerState::Running);
    }

    #[test]
    fn test_runner_toggle_pause() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.toggle_pause(); // Idle -> Running
        assert_eq!(runner.state(), RunnerState::Running);
        runner.toggle_pause(); // Running -> Paused
        assert_eq!(runner.state(), RunnerState::Paused);
        runner.toggle_pause(); // Paused -> Running
        assert_eq!(runner.state(), RunnerState::Running);
    }

    #[test]
    fn test_runner_step_mode() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.step_mode();
        assert_eq!(runner.state(), RunnerState::Stepping);
    }

    #[test]
    fn test_runner_step_forward_marks_started() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.step_mode();
        runner.step_forward();

        assert_eq!(runner.state(), RunnerState::Stepping);
        assert!(runner.step_started.is_some());
        assert_eq!(runner.current_step(), 0);
    }

    #[test]
    fn test_runner_complete_step() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.start();

        let result = StepResult {
            step_index: 0,
            duration: Duration::from_millis(50),
            assertion: None,
            error: None,
        };
        runner.complete_step(result);

        assert_eq!(runner.current_step(), 1);
        assert_eq!(runner.results().len(), 1);
    }

    #[test]
    fn test_runner_passes_on_completion() {
        let mut runner = TestRunner::new(
            "Test",
            vec![TestStep::Wait {
                duration: Duration::from_millis(10),
            }],
        );
        runner.start();

        let result = StepResult {
            step_index: 0,
            duration: Duration::from_millis(10),
            assertion: None,
            error: None,
        };
        runner.complete_step(result);

        assert_eq!(runner.state(), RunnerState::Passed);
    }

    #[test]
    fn test_runner_fails_on_error() {
        let mut runner = TestRunner::new("Test", test_steps());
        runner.start();

        let result = StepResult {
            step_index: 0,
            duration: Duration::from_millis(10),
            assertion: None,
            error: Some("Element not found".to_string()),
        };
        runner.complete_step(result);

        assert_eq!(runner.state(), RunnerState::Failed);
        assert!(runner.error().is_some());
    }

    #[test]
    fn test_playback_speed_scale() {
        let duration = Duration::from_secs(2);
        assert_eq!(PlaybackSpeed::FAST.scale(duration), Duration::from_secs(1));
        assert_eq!(PlaybackSpeed::SLOW.scale(duration), Duration::from_secs(4));
    }

    #[test]
    fn test_playback_speed_custom_clamps() {
        let slow = PlaybackSpeed::custom(0.01);
        let fast = PlaybackSpeed::custom(200.0);
        assert!((slow.multiplier() - 0.1).abs() < 0.0001);
        assert!((fast.multiplier() - 100.0).abs() < 0.0001);
    }

    #[test]
    fn test_runner_state_labels() {
        assert_eq!(RunnerState::Idle.label(), "IDLE");
        assert_eq!(RunnerState::Running.label(), "RUNNING");
        assert_eq!(RunnerState::Passed.label(), "PASSED");
        assert_eq!(RunnerState::Failed.label(), "FAILED");
    }
}
