//! JSON reporter for machine-readable output.

use super::Reporter;
use crate::runner::{StoryResult, TestResults};
use crate::story::StoryOutcome;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Mutex;

/// JSON reporter for CI integration.
pub struct JsonReporter {
    /// Output writer (defaults to stdout).
    output: Mutex<Box<dyn Write + Send>>,
    /// Pretty print JSON.
    pretty: bool,
    /// Collected results for final output.
    results: Mutex<Vec<JsonStoryResult>>,
}

/// JSON-serializable story result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonStoryResult {
    /// Story name.
    pub name: String,
    /// Story tags.
    pub tags: Vec<String>,
    /// Status: "passed", "failed", or "skipped".
    pub status: String,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Error message (if failed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Phase where failure occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_phase: Option<String>,
    /// Skip reason (if skipped).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

/// JSON-serializable test run summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonTestResults {
    /// Total number of stories.
    pub total: usize,
    /// Number passed.
    pub passed: usize,
    /// Number failed.
    pub failed: usize,
    /// Number skipped.
    pub skipped: usize,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Pass rate percentage.
    pub pass_rate: f64,
    /// Individual story results.
    pub stories: Vec<JsonStoryResult>,
}

impl JsonReporter {
    /// Create a new JSON reporter writing to stdout.
    pub fn new() -> Self {
        Self {
            output: Mutex::new(Box::new(std::io::stdout())),
            pretty: false,
            results: Mutex::new(Vec::new()),
        }
    }

    /// Create a JSON reporter writing to a custom writer.
    pub fn with_writer<W: Write + Send + 'static>(writer: W) -> Self {
        Self {
            output: Mutex::new(Box::new(writer)),
            pretty: false,
            results: Mutex::new(Vec::new()),
        }
    }

    /// Enable pretty-printed JSON output.
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Write JSON to output.
    fn write_json<T: Serialize>(&self, value: &T) {
        let json = if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        };

        if let Ok(json) = json {
            let mut output = self.output.lock().unwrap();
            let _ = writeln!(output, "{}", json);
        }
    }
}

impl Default for JsonReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl Reporter for JsonReporter {
    fn on_run_start(&self, _total: usize) {
        // Clear previous results
        self.results.lock().unwrap().clear();
    }

    fn on_story_start(&self, _name: &str) {
        // JSON reporter doesn't output per-story start
    }

    fn on_story_complete(&self, result: &StoryResult) {
        let json_result = JsonStoryResult {
            name: result.name.clone(),
            tags: result.tags.clone(),
            status: match &result.outcome {
                StoryOutcome::Passed => "passed".to_string(),
                StoryOutcome::Failed { .. } => "failed".to_string(),
                StoryOutcome::Skipped { .. } => "skipped".to_string(),
            },
            duration_ms: result.duration.as_millis() as u64,
            error: match &result.outcome {
                StoryOutcome::Failed { message, .. } => Some(message.clone()),
                _ => None,
            },
            failed_phase: match &result.outcome {
                StoryOutcome::Failed { phase, .. } => Some(phase.clone()),
                _ => None,
            },
            skip_reason: match &result.outcome {
                StoryOutcome::Skipped { reason } => Some(reason.clone()),
                _ => None,
            },
        };

        self.results.lock().unwrap().push(json_result);
    }

    fn on_run_complete(&self, results: &TestResults) {
        let json_results = JsonTestResults {
            total: results.total,
            passed: results.passed,
            failed: results.failed,
            skipped: results.skipped,
            duration_ms: results.duration.as_millis() as u64,
            pass_rate: results.pass_rate(),
            stories: self.results.lock().unwrap().clone(),
        };

        self.write_json(&json_results);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_json_story_result_serialization() {
        let result = JsonStoryResult {
            name: "test".to_string(),
            tags: vec!["smoke".to_string()],
            status: "passed".to_string(),
            duration_ms: 100,
            error: None,
            failed_phase: None,
            skip_reason: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"status\":\"passed\""));
        // error, failed_phase, skip_reason should not appear
        assert!(!json.contains("error"));
    }

    #[test]
    fn test_json_story_result_with_failure() {
        let result = JsonStoryResult {
            name: "failing".to_string(),
            tags: vec![],
            status: "failed".to_string(),
            duration_ms: 50,
            error: Some("assertion failed".to_string()),
            failed_phase: Some("then".to_string()),
            skip_reason: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"error\":\"assertion failed\""));
        assert!(json.contains("\"failed_phase\":\"then\""));
    }

    #[test]
    fn test_json_reporter_to_buffer() {
        let buffer = std::io::Cursor::new(Vec::new());
        let reporter = JsonReporter::with_writer(buffer);

        reporter.on_run_start(1);

        let result = StoryResult {
            name: "test story".to_string(),
            tags: vec!["unit".to_string()],
            outcome: StoryOutcome::Passed,
            duration: Duration::from_millis(100),
        };
        reporter.on_story_complete(&result);

        let test_results = TestResults {
            total: 1,
            passed: 1,
            failed: 0,
            skipped: 0,
            duration: Duration::from_millis(100),
            results: vec![],
        };
        reporter.on_run_complete(&test_results);

        // Verify results were collected
        let collected = reporter.results.lock().unwrap();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].name, "test story");
    }
}
