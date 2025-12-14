//! Console reporter for human-readable output.

use super::Reporter;
use crate::runner::{StoryResult, TestResults};
use crate::story::StoryOutcome;
use std::sync::Mutex;

/// Console reporter with colored output.
pub struct ConsoleReporter {
    /// Whether to use verbose output.
    verbose: bool,
    /// Current output (for testing).
    output: Mutex<Vec<String>>,
}

impl ConsoleReporter {
    /// Create a new console reporter.
    pub fn new() -> Self {
        Self {
            verbose: false,
            output: Mutex::new(Vec::new()),
        }
    }

    /// Enable verbose output.
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    /// Print a line to the console.
    fn println(&self, line: &str) {
        println!("{}", line);
        self.output.lock().unwrap().push(line.to_string());
    }

    /// Format duration in human-readable form.
    fn format_duration(duration: std::time::Duration) -> String {
        let millis = duration.as_millis();
        if millis < 1000 {
            format!("{}ms", millis)
        } else {
            format!("{:.2}s", duration.as_secs_f64())
        }
    }

    /// Get captured output (for testing).
    pub fn captured_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }
}

impl Default for ConsoleReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for ConsoleReporter {
    fn on_run_start(&self, total: usize) {
        self.println(&format!("\nRunning {} stories...\n", total));
    }

    fn on_story_start(&self, name: &str) {
        if self.verbose {
            self.println(&format!("  Starting: {}", name));
        }
    }

    fn on_story_complete(&self, result: &StoryResult) {
        let status = match &result.outcome {
            StoryOutcome::Passed => "PASS",
            StoryOutcome::Failed { .. } => "FAIL",
            StoryOutcome::Skipped { .. } => "SKIP",
        };

        let duration = Self::format_duration(result.duration);

        let line = format!(
            "  [{}] {} ({})",
            status, result.name, duration
        );

        self.println(&line);

        // Print failure details
        if let StoryOutcome::Failed { message, phase } = &result.outcome {
            self.println(&format!("         Failed in '{}' phase: {}", phase, message));
        }

        // Print skip reason
        if let StoryOutcome::Skipped { reason } = &result.outcome {
            self.println(&format!("         Skipped: {}", reason));
        }
    }

    fn on_run_complete(&self, results: &TestResults) {
        self.println("");
        self.println(&format!(
            "Results: {} passed, {} failed, {} skipped ({} total)",
            results.passed,
            results.failed,
            results.skipped,
            results.total
        ));
        self.println(&format!(
            "Duration: {}",
            Self::format_duration(results.duration)
        ));
        self.println(&format!("Pass rate: {:.1}%", results.pass_rate()));

        if results.has_failures() {
            self.println("\nFailed stories:");
            for failure in results.failures() {
                if let StoryOutcome::Failed { message, phase } = &failure.outcome {
                    self.println(&format!(
                        "  - {} ({}): {}",
                        failure.name, phase, message
                    ));
                }
            }
        }

        self.println("");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_console_reporter_format_duration() {
        assert_eq!(ConsoleReporter::format_duration(Duration::from_millis(50)), "50ms");
        assert_eq!(ConsoleReporter::format_duration(Duration::from_millis(999)), "999ms");
        assert_eq!(ConsoleReporter::format_duration(Duration::from_secs(1)), "1.00s");
        assert_eq!(ConsoleReporter::format_duration(Duration::from_millis(1500)), "1.50s");
    }

    #[test]
    fn test_console_reporter_captures_output() {
        let reporter = ConsoleReporter::new();

        reporter.on_run_start(5);

        let output = reporter.captured_output();
        assert!(!output.is_empty());
        assert!(output[0].contains("5 stories"));
    }

    #[test]
    fn test_console_reporter_story_complete() {
        let reporter = ConsoleReporter::new();

        let result = StoryResult {
            name: "test story".to_string(),
            tags: vec![],
            outcome: StoryOutcome::Passed,
            duration: Duration::from_millis(100),
        };

        reporter.on_story_complete(&result);

        let output = reporter.captured_output();
        assert!(output.iter().any(|l| l.contains("PASS")));
        assert!(output.iter().any(|l| l.contains("test story")));
    }

    #[test]
    fn test_console_reporter_failure() {
        let reporter = ConsoleReporter::new();

        let result = StoryResult {
            name: "failing story".to_string(),
            tags: vec![],
            outcome: StoryOutcome::Failed {
                message: "assertion failed".to_string(),
                phase: "then".to_string(),
            },
            duration: Duration::from_millis(50),
        };

        reporter.on_story_complete(&result);

        let output = reporter.captured_output();
        assert!(output.iter().any(|l| l.contains("FAIL")));
        assert!(output.iter().any(|l| l.contains("assertion failed")));
    }
}
