//! Error analysis and failure categorization for benchmark results.
//!
//! Provides tools for understanding why methods fail on specific tasks.

use std::collections::HashMap;
use std::path::Path;

/// Error category for failed tasks.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Context was too long for the model.
    ContextTooLong,
    /// Needle/answer was not found in response.
    AnswerNotFound,
    /// Response was in wrong format.
    WrongFormat,
    /// Method timed out.
    Timeout,
    /// Code execution failed (for CodeAct).
    CodeExecutionFailed,
    /// Sub-query failed (for RLM).
    SubQueryFailed,
    /// Empty or null response.
    EmptyResponse,
    /// Partial match (close but not exact).
    PartialMatch,
    /// Other/unknown error.
    Other(String),
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContextTooLong => write!(f, "Context Too Long"),
            Self::AnswerNotFound => write!(f, "Answer Not Found"),
            Self::WrongFormat => write!(f, "Wrong Format"),
            Self::Timeout => write!(f, "Timeout"),
            Self::CodeExecutionFailed => write!(f, "Code Execution Failed"),
            Self::SubQueryFailed => write!(f, "Sub-Query Failed"),
            Self::EmptyResponse => write!(f, "Empty Response"),
            Self::PartialMatch => write!(f, "Partial Match"),
            Self::Other(s) => write!(f, "Other: {}", s),
        }
    }
}

/// A failed task with categorization.
#[derive(Debug, Clone)]
pub struct FailedTask {
    /// Task ID.
    pub task_id: String,
    /// Dataset.
    pub dataset: String,
    /// Method used.
    pub method: String,
    /// Expected answer.
    pub expected: String,
    /// Actual response.
    pub actual: String,
    /// Error category.
    pub category: ErrorCategory,
    /// Score (0.0 for complete failure, partial for near-misses).
    pub score: f64,
    /// Context length in characters.
    pub context_length: Option<usize>,
}

/// Categorize a failure based on expected/actual values.
pub fn categorize_failure(
    expected: &str,
    actual: &str,
    context_length: Option<usize>,
    _method: &str,
) -> ErrorCategory {
    // Empty response
    if actual.trim().is_empty() {
        return ErrorCategory::EmptyResponse;
    }

    // Context too long (heuristic: >200K chars often causes issues)
    if let Some(len) = context_length {
        if len > 200_000 {
            return ErrorCategory::ContextTooLong;
        }
    }

    // Partial match - contains the answer but with extra text
    let expected_lower = expected.to_lowercase();
    let actual_lower = actual.to_lowercase();
    if actual_lower.contains(&expected_lower) {
        return ErrorCategory::PartialMatch;
    }

    // Wrong format - has structure but wrong content
    if actual.contains('[') || actual.contains('{') || actual.contains(':') {
        return ErrorCategory::WrongFormat;
    }

    // Default to answer not found
    ErrorCategory::AnswerNotFound
}

/// Collection of error analyses.
#[derive(Debug, Default)]
pub struct ErrorAnalysis {
    /// All failed tasks.
    pub failures: Vec<FailedTask>,
    /// Failures by category.
    by_category: HashMap<ErrorCategory, Vec<FailedTask>>,
    /// Failures by dataset.
    by_dataset: HashMap<String, Vec<FailedTask>>,
    /// Failures by method.
    by_method: HashMap<String, Vec<FailedTask>>,
}

impl ErrorAnalysis {
    /// Create a new empty analysis.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a failed task.
    pub fn add_failure(&mut self, failure: FailedTask) {
        self.by_category
            .entry(failure.category.clone())
            .or_default()
            .push(failure.clone());
        self.by_dataset
            .entry(failure.dataset.clone())
            .or_default()
            .push(failure.clone());
        self.by_method
            .entry(failure.method.clone())
            .or_default()
            .push(failure.clone());
        self.failures.push(failure);
    }

    /// Get failure counts by category.
    pub fn category_counts(&self) -> HashMap<ErrorCategory, usize> {
        self.by_category
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    /// Get failure counts by dataset.
    pub fn dataset_counts(&self) -> HashMap<String, usize> {
        self.by_dataset
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    /// Get failure counts by method.
    pub fn method_counts(&self) -> HashMap<String, usize> {
        self.by_method
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect()
    }

    /// Generate markdown report.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("## Error Analysis Report\n\n");
        output.push_str(&format!("Total failures: {}\n\n", self.failures.len()));

        // By category
        output.push_str("### Failures by Category\n\n");
        output.push_str("| Category | Count | % |\n");
        output.push_str("|----------|-------|---|\n");

        let total = self.failures.len() as f64;
        let mut categories: Vec<_> = self.category_counts().into_iter().collect();
        categories.sort_by(|a, b| b.1.cmp(&a.1));

        for (category, count) in categories {
            let pct = (count as f64 / total) * 100.0;
            output.push_str(&format!("| {} | {} | {:.1}% |\n", category, count, pct));
        }

        // By dataset
        output.push_str("\n### Failures by Dataset\n\n");
        output.push_str("| Dataset | Count | % |\n");
        output.push_str("|---------|-------|---|\n");

        let mut datasets: Vec<_> = self.dataset_counts().into_iter().collect();
        datasets.sort_by(|a, b| b.1.cmp(&a.1));

        for (dataset, count) in datasets {
            let pct = (count as f64 / total) * 100.0;
            output.push_str(&format!("| {} | {} | {:.1}% |\n", dataset, count, pct));
        }

        // By method
        output.push_str("\n### Failures by Method\n\n");
        output.push_str("| Method | Count | % |\n");
        output.push_str("|--------|-------|---|\n");

        let mut methods: Vec<_> = self.method_counts().into_iter().collect();
        methods.sort_by(|a, b| b.1.cmp(&a.1));

        for (method, count) in methods {
            let pct = (count as f64 / total) * 100.0;
            output.push_str(&format!("| {} | {} | {:.1}% |\n", method, count, pct));
        }

        // Sample failures
        output.push_str("\n### Sample Failures\n\n");
        for failure in self.failures.iter().take(5) {
            output.push_str(&format!(
                "**{}** ({} on {})\n",
                failure.task_id, failure.method, failure.dataset
            ));
            output.push_str(&format!("- Category: {}\n", failure.category));
            output.push_str(&format!(
                "- Expected: `{}`\n",
                truncate(&failure.expected, 50)
            ));
            output.push_str(&format!(
                "- Actual: `{}`\n",
                truncate(&failure.actual, 50)
            ));
            output.push('\n');
        }

        output
    }

    /// Export failures to JSONL for manual analysis.
    pub fn export_jsonl(&self, path: &Path) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;

        for failure in &self.failures {
            let json = serde_json::json!({
                "task_id": failure.task_id,
                "dataset": failure.dataset,
                "method": failure.method,
                "expected": failure.expected,
                "actual": failure.actual,
                "category": format!("{}", failure.category),
                "score": failure.score,
                "context_length": failure.context_length,
            });
            writeln!(file, "{}", json)?;
        }

        Ok(())
    }
}

/// Context length analysis.
#[derive(Debug)]
pub struct ContextLengthAnalysis {
    /// Accuracy by context length bucket.
    pub buckets: Vec<(usize, f64, usize)>, // (max_length, accuracy, n_samples)
}

impl ContextLengthAnalysis {
    /// Analyze performance by context length.
    pub fn from_results(
        results: &[(usize, f64)], // (context_length, score)
        bucket_boundaries: &[usize],
    ) -> Self {
        let mut bucket_data: HashMap<usize, (f64, usize)> = HashMap::new();

        for &(length, score) in results {
            // Find appropriate bucket
            let bucket = bucket_boundaries
                .iter()
                .find(|&&b| length <= b)
                .copied()
                .unwrap_or(*bucket_boundaries.last().unwrap_or(&100_000));

            let entry = bucket_data.entry(bucket).or_insert((0.0, 0));
            entry.0 += score;
            entry.1 += 1;
        }

        let mut buckets: Vec<_> = bucket_data
            .into_iter()
            .map(|(max_len, (sum, count))| {
                let avg = if count > 0 { sum / count as f64 } else { 0.0 };
                (max_len, avg, count)
            })
            .collect();

        buckets.sort_by_key(|&(max_len, _, _)| max_len);

        Self { buckets }
    }

    /// Generate markdown report.
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("### Performance by Context Length\n\n");
        output.push_str("| Context Length | Accuracy | N |\n");
        output.push_str("|----------------|----------|---|\n");

        for &(max_len, accuracy, n) in &self.buckets {
            let len_str = if max_len >= 1000 {
                format!("≤{}K", max_len / 1000)
            } else {
                format!("≤{}", max_len)
            };
            output.push_str(&format!("| {} | {:.1}% | {} |\n", len_str, accuracy * 100.0, n));
        }

        output
    }
}

/// Truncate a string with ellipsis.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Load failures from trajectory files.
pub fn load_failures_from_trajectories(
    results_dir: &Path,
) -> Result<ErrorAnalysis, Box<dyn std::error::Error>> {
    use std::fs;

    let mut analysis = ErrorAnalysis::new();

    for entry in fs::read_dir(results_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Look for trajectory files
            for traj_entry in fs::read_dir(&path)? {
                let traj_entry = traj_entry?;
                let traj_path = traj_entry.path();

                if traj_path
                    .file_name()
                    .map(|n| n.to_string_lossy().contains("trajectories"))
                    .unwrap_or(false)
                {
                    // Parse trajectory file for failures
                    // This is a simplified implementation
                    let dir_name = path.file_name().unwrap().to_string_lossy();
                    let parts: Vec<&str> = dir_name.split('-').collect();
                    if parts.len() >= 2 {
                        let method = parts[0].to_string();
                        let dataset = parts[1..].join("-");

                        // Would parse actual trajectories here
                        // For now, just note the file exists
                        tracing::debug!(
                            "Found trajectory file for {} on {}",
                            method,
                            dataset
                        );
                    }
                }
            }
        }
    }

    Ok(analysis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_failure() {
        // Empty response
        assert_eq!(
            categorize_failure("answer", "", None, "Base"),
            ErrorCategory::EmptyResponse
        );

        // Partial match
        assert_eq!(
            categorize_failure("42", "The answer is 42.", None, "Base"),
            ErrorCategory::PartialMatch
        );

        // Wrong format
        assert_eq!(
            categorize_failure("42", "{\"result\": 43}", None, "Base"),
            ErrorCategory::WrongFormat
        );
    }

    #[test]
    fn test_error_analysis() {
        let mut analysis = ErrorAnalysis::new();

        analysis.add_failure(FailedTask {
            task_id: "task-1".to_string(),
            dataset: "S-NIAH".to_string(),
            method: "Base".to_string(),
            expected: "ABC123".to_string(),
            actual: "".to_string(),
            category: ErrorCategory::EmptyResponse,
            score: 0.0,
            context_length: Some(50000),
        });

        analysis.add_failure(FailedTask {
            task_id: "task-2".to_string(),
            dataset: "S-NIAH".to_string(),
            method: "Base".to_string(),
            expected: "XYZ789".to_string(),
            actual: "The answer is XYZ789.".to_string(),
            category: ErrorCategory::PartialMatch,
            score: 0.5,
            context_length: Some(75000),
        });

        assert_eq!(analysis.failures.len(), 2);
        assert_eq!(analysis.category_counts().len(), 2);
    }

    #[test]
    fn test_context_length_analysis() {
        let results = vec![
            (10000, 0.90),
            (15000, 0.85),
            (30000, 0.75),
            (50000, 0.60),
            (80000, 0.45),
            (100000, 0.30),
        ];

        let buckets = vec![20000, 50000, 100000];
        let analysis = ContextLengthAnalysis::from_results(&results, &buckets);

        assert_eq!(analysis.buckets.len(), 3);
    }
}
