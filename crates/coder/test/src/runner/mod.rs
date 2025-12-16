//! Test runner for executing stories in parallel.
//!
//! Provides parallel execution with configurable thread counts,
//! filtering by tags or name patterns, and multiple reporter support.

use crate::report::Reporter;
use crate::story::{Story, StoryInventory, StoryOutcome};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Parallelism configuration for test execution.
#[derive(Debug, Clone, Copy)]
pub enum Parallelism {
    /// Run tests serially.
    Serial,
    /// Run tests in parallel with specific thread count.
    Parallel { threads: usize },
    /// Use rayon's default thread count (number of CPUs).
    Auto,
}

impl Default for Parallelism {
    fn default() -> Self {
        Parallelism::Auto
    }
}

/// Configuration for the test runner.
#[derive(Default)]
pub struct RunnerConfig {
    /// Parallelism mode.
    pub parallelism: Parallelism,
    /// Filter by story name pattern.
    pub filter: Option<String>,
    /// Filter by tags.
    pub tags: Vec<String>,
    /// Reporters to use.
    pub reporters: Vec<Box<dyn Reporter + Send + Sync>>,
    /// Stop on first failure.
    pub fail_fast: bool,
}

impl RunnerConfig {
    /// Create a new runner config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set parallelism mode.
    pub fn parallelism(mut self, p: Parallelism) -> Self {
        self.parallelism = p;
        self
    }

    /// Set name filter.
    pub fn filter(mut self, pattern: impl Into<String>) -> Self {
        self.filter = Some(pattern.into());
        self
    }

    /// Add tags to filter by.
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Add a reporter.
    pub fn reporter<R: Reporter + Send + Sync + 'static>(mut self, reporter: R) -> Self {
        self.reporters.push(Box::new(reporter));
        self
    }

    /// Enable fail-fast mode.
    pub fn fail_fast(mut self) -> Self {
        self.fail_fast = true;
        self
    }
}

/// Results of a test run.
#[derive(Debug, Default)]
pub struct TestResults {
    /// Total number of stories.
    pub total: usize,
    /// Number of passed stories.
    pub passed: usize,
    /// Number of failed stories.
    pub failed: usize,
    /// Number of skipped stories.
    pub skipped: usize,
    /// Total duration.
    pub duration: Duration,
    /// Individual results.
    pub results: Vec<StoryResult>,
}

impl TestResults {
    /// Check if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Check if any tests failed.
    pub fn has_failures(&self) -> bool {
        self.failed > 0
    }

    /// Get failure messages.
    pub fn failures(&self) -> Vec<&StoryResult> {
        self.results
            .iter()
            .filter(|r| r.outcome.is_failed())
            .collect()
    }

    /// Get pass rate as percentage.
    pub fn pass_rate(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.passed as f64 / self.total as f64) * 100.0
        }
    }
}

/// Result of a single story execution.
#[derive(Debug)]
pub struct StoryResult {
    /// Story name.
    pub name: String,
    /// Story tags.
    pub tags: Vec<String>,
    /// Execution outcome.
    pub outcome: StoryOutcome,
    /// Duration of execution.
    pub duration: Duration,
}

/// Test runner for executing stories.
pub struct TestRunner {
    /// Story inventory.
    inventory: StoryInventory,
    /// Configuration.
    config: RunnerConfig,
}

// Global runner for registration
lazy_static::lazy_static! {
    static ref GLOBAL_RUNNER: Mutex<Option<StoryInventory>> = Mutex::new(None);
}

impl TestRunner {
    /// Create a new test runner.
    pub fn new(config: RunnerConfig) -> Self {
        Self {
            inventory: StoryInventory::new(),
            config,
        }
    }

    /// Create a runner with default config.
    pub fn default_runner() -> Self {
        Self::new(RunnerConfig::default())
    }

    /// Register a story with the global runner.
    pub fn register(story: Story) {
        let mut guard = GLOBAL_RUNNER.lock().unwrap();
        if guard.is_none() {
            *guard = Some(StoryInventory::new());
        }
        guard.as_mut().unwrap().register(story);
    }

    /// Add a story to this runner.
    pub fn add_story(&mut self, story: Story) {
        self.inventory.register(story);
    }

    /// Run all stories and return results.
    pub fn run(&self) -> TestResults {
        let start = Instant::now();

        // Get stories to run (apply filters)
        let stories: Vec<&Story> = self
            .inventory
            .stories()
            .iter()
            .filter(|s| self.should_run(s))
            .collect();

        let total = stories.len();

        // Notify reporters
        for reporter in &self.config.reporters {
            reporter.on_run_start(total);
        }

        // Execute stories
        let results: Vec<StoryResult> = match self.config.parallelism {
            Parallelism::Serial => self.run_serial(&stories),
            Parallelism::Parallel { threads } => self.run_parallel(&stories, threads),
            Parallelism::Auto => self.run_parallel(&stories, num_cpus::get()),
        };

        // Calculate summary
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for result in &results {
            match &result.outcome {
                StoryOutcome::Passed => passed += 1,
                StoryOutcome::Failed { .. } => failed += 1,
                StoryOutcome::Skipped { .. } => skipped += 1,
            }
        }

        let duration = start.elapsed();

        let test_results = TestResults {
            total,
            passed,
            failed,
            skipped,
            duration,
            results,
        };

        // Notify reporters
        for reporter in &self.config.reporters {
            reporter.on_run_complete(&test_results);
        }

        test_results
    }

    /// Check if a story should be run based on filters.
    fn should_run(&self, story: &Story) -> bool {
        // Check name filter
        if let Some(filter) = &self.config.filter {
            if !story.matches_filter(filter) {
                return false;
            }
        }

        // Check tag filter
        if !self.config.tags.is_empty() && !story.matches_any_tag(&self.config.tags) {
            return false;
        }

        true
    }

    /// Run stories serially.
    fn run_serial(&self, stories: &[&Story]) -> Vec<StoryResult> {
        let mut results = Vec::with_capacity(stories.len());

        for story in stories {
            let result = self.run_story(story);

            // Notify reporters
            for reporter in &self.config.reporters {
                reporter.on_story_complete(&result);
            }

            let is_failure = result.outcome.is_failed();
            results.push(result);

            if self.config.fail_fast && is_failure {
                break;
            }
        }

        results
    }

    /// Run stories in parallel.
    fn run_parallel(&self, stories: &[&Story], threads: usize) -> Vec<StoryResult> {
        use rayon::prelude::*;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("Failed to create thread pool");

        let failed = Arc::new(Mutex::new(false));

        pool.install(|| {
            stories
                .par_iter()
                .filter(|_| {
                    if self.config.fail_fast {
                        !*failed.lock().unwrap()
                    } else {
                        true
                    }
                })
                .map(|story| {
                    let result = self.run_story(story);

                    if self.config.fail_fast && result.outcome.is_failed() {
                        *failed.lock().unwrap() = true;
                    }

                    // Notify reporters (thread-safe)
                    for reporter in &self.config.reporters {
                        reporter.on_story_complete(&result);
                    }

                    result
                })
                .collect()
        })
    }

    /// Run a single story.
    fn run_story(&self, story: &Story) -> StoryResult {
        // Notify reporters
        for reporter in &self.config.reporters {
            reporter.on_story_start(&story.name);
        }

        let start = Instant::now();
        let outcome = story.execute();
        let duration = start.elapsed();

        StoryResult {
            name: story.name.clone(),
            tags: story.tags.clone(),
            outcome,
            duration,
        }
    }
}

// num_cpus crate substitute - use rayon's default
mod num_cpus {
    pub fn get() -> usize {
        rayon::current_num_threads()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story::StoryBuilder;

    #[test]
    fn test_runner_config_builder() {
        let config = RunnerConfig::new()
            .parallelism(Parallelism::Serial)
            .filter("chat")
            .tags(["smoke", "e2e"])
            .fail_fast();

        assert!(matches!(config.parallelism, Parallelism::Serial));
        assert_eq!(config.filter, Some("chat".to_string()));
        assert_eq!(config.tags, vec!["smoke", "e2e"]);
        assert!(config.fail_fast);
    }

    #[test]
    fn test_runner_serial() {
        let mut runner = TestRunner::new(RunnerConfig::new().parallelism(Parallelism::Serial));

        runner.add_story(StoryBuilder::new("test 1").then(|_| {}).build());
        runner.add_story(StoryBuilder::new("test 2").then(|_| {}).build());

        let results = runner.run();

        assert_eq!(results.total, 2);
        assert_eq!(results.passed, 2);
        assert_eq!(results.failed, 0);
        assert!(results.all_passed());
    }

    #[test]
    fn test_runner_with_failure() {
        let mut runner = TestRunner::new(RunnerConfig::new().parallelism(Parallelism::Serial));

        runner.add_story(StoryBuilder::new("passing").then(|_| {}).build());
        runner.add_story(
            StoryBuilder::new("failing")
                .then(|_| {
                    panic!("intentional failure");
                })
                .build(),
        );

        let results = runner.run();

        assert_eq!(results.total, 2);
        assert_eq!(results.passed, 1);
        assert_eq!(results.failed, 1);
        assert!(results.has_failures());
    }

    #[test]
    fn test_runner_filter_by_name() {
        let mut runner = TestRunner::new(
            RunnerConfig::new()
                .parallelism(Parallelism::Serial)
                .filter("chat"),
        );

        runner.add_story(StoryBuilder::new("chat feature").then(|_| {}).build());
        runner.add_story(StoryBuilder::new("login feature").then(|_| {}).build());

        let results = runner.run();

        assert_eq!(results.total, 1);
        assert_eq!(results.results[0].name, "chat feature");
    }

    #[test]
    fn test_runner_filter_by_tags() {
        let mut runner = TestRunner::new(
            RunnerConfig::new()
                .parallelism(Parallelism::Serial)
                .tags(["smoke"]),
        );

        runner.add_story(
            StoryBuilder::new("with tag")
                .tagged("smoke")
                .then(|_| {})
                .build(),
        );
        runner.add_story(StoryBuilder::new("without tag").then(|_| {}).build());

        let results = runner.run();

        assert_eq!(results.total, 1);
        assert_eq!(results.results[0].name, "with tag");
    }

    #[test]
    fn test_results_pass_rate() {
        let results = TestResults {
            total: 10,
            passed: 8,
            failed: 2,
            skipped: 0,
            duration: Duration::from_secs(1),
            results: Vec::new(),
        };

        assert_eq!(results.pass_rate(), 80.0);
    }
}
