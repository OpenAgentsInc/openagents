//! Benchmark comparison and reporting.
//!
//! Generates human-readable and machine-readable comparison reports
//! between benchmark runs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Metric Delta Types
// ============================================================================

/// Delta for a single metric comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDelta {
    pub metric: String,
    pub baseline: f64,
    pub current: f64,
    pub delta: f64,
    pub delta_percent: f64,
    pub improved: bool,
}

impl MetricDelta {
    /// Compute a delta between baseline and current values.
    pub fn compute(metric: &str, baseline: f64, current: f64, lower_is_better: bool) -> Self {
        let delta = current - baseline;
        let delta_percent = if baseline != 0.0 {
            (delta / baseline) * 100.0
        } else {
            0.0
        };
        let improved = if lower_is_better {
            delta < 0.0
        } else {
            delta > 0.0
        };

        Self {
            metric: metric.to_string(),
            baseline,
            current,
            delta,
            delta_percent,
            improved,
        }
    }
}

// ============================================================================
// Task Comparison Types
// ============================================================================

/// Comparison of a single task between runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComparison {
    pub task_id: String,
    pub task_title: String,
    pub baseline_outcome: String,
    pub current_outcome: String,
    pub outcome_changed: bool,
    pub improved: bool,
    pub regressed: bool,
    pub turns_delta: i32,
    pub tokens_delta: i64,
    pub duration_delta_ms: i64,
}

/// Overall comparison verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComparisonVerdict {
    Improved,
    Regressed,
    Unchanged,
    Mixed,
}

impl ComparisonVerdict {
    pub fn emoji(&self) -> &'static str {
        match self {
            ComparisonVerdict::Improved => "✅",
            ComparisonVerdict::Regressed => "❌",
            ComparisonVerdict::Unchanged => "➖",
            ComparisonVerdict::Mixed => "⚠️",
        }
    }
}

// ============================================================================
// Run Metadata
// ============================================================================

/// Metadata about a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub run_id: String,
    pub model: String,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub git_branch: Option<String>,
    pub git_commit: Option<String>,
}

// ============================================================================
// Benchmark Summary
// ============================================================================

/// Summary statistics for a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    pub total_tasks: u32,
    pub successful_tasks: u32,
    pub failed_tasks: u32,
    pub timeout_tasks: u32,
    pub error_tasks: u32,
    pub task_completion_rate: f64,
    pub verification_pass_rate: f64,
    pub avg_tokens_per_task: f64,
    pub avg_turns_per_task: f64,
    pub avg_tool_calls_per_task: f64,
    pub retry_rate: f64,
    pub total_duration_ms: u64,
    pub tool_distribution: HashMap<String, u32>,
}

/// Task outcome from a benchmark run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskOutcome {
    Success,
    Failure,
    Timeout,
    Error,
}

impl TaskOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskOutcome::Success => "success",
            TaskOutcome::Failure => "failure",
            TaskOutcome::Timeout => "timeout",
            TaskOutcome::Error => "error",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            TaskOutcome::Success => "✅",
            TaskOutcome::Failure => "❌",
            TaskOutcome::Timeout => "⏱️",
            TaskOutcome::Error => "💥",
        }
    }
}

/// Task metrics from a benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub task_id: String,
    pub task_title: String,
    pub outcome: TaskOutcome,
    pub turns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_ms: u64,
}

impl TaskMetrics {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Complete benchmark results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub meta: RunMeta,
    pub summary: BenchmarkSummary,
    pub tasks: Vec<TaskMetrics>,
}

// ============================================================================
// Comparison Report
// ============================================================================

/// Full comparison report between two benchmark runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub generated_at: DateTime<Utc>,
    pub baseline: RunMeta,
    pub current: RunMeta,
    pub summary_deltas: Vec<MetricDelta>,
    pub task_comparisons: Vec<TaskComparison>,
    pub overall_verdict: ComparisonVerdict,
}

/// Compare two benchmark runs and generate a report.
pub fn compare_runs(baseline: &BenchmarkResults, current: &BenchmarkResults) -> ComparisonReport {
    // Compute summary deltas
    let summary_deltas = vec![
        MetricDelta::compute(
            "task_completion_rate",
            baseline.summary.task_completion_rate,
            current.summary.task_completion_rate,
            false, // higher is better
        ),
        MetricDelta::compute(
            "verification_pass_rate",
            baseline.summary.verification_pass_rate,
            current.summary.verification_pass_rate,
            false, // higher is better
        ),
        MetricDelta::compute(
            "avg_tokens_per_task",
            baseline.summary.avg_tokens_per_task,
            current.summary.avg_tokens_per_task,
            true, // lower is better
        ),
        MetricDelta::compute(
            "avg_turns_per_task",
            baseline.summary.avg_turns_per_task,
            current.summary.avg_turns_per_task,
            true, // lower is better
        ),
        MetricDelta::compute(
            "avg_tool_calls_per_task",
            baseline.summary.avg_tool_calls_per_task,
            current.summary.avg_tool_calls_per_task,
            true, // lower is better
        ),
        MetricDelta::compute(
            "retry_rate",
            baseline.summary.retry_rate,
            current.summary.retry_rate,
            true, // lower is better
        ),
        MetricDelta::compute(
            "total_duration_ms",
            baseline.summary.total_duration_ms as f64,
            current.summary.total_duration_ms as f64,
            true, // lower is better
        ),
    ];

    // Build task comparison map
    let baseline_task_map: HashMap<&str, &TaskMetrics> = baseline
        .tasks
        .iter()
        .map(|t| (t.task_id.as_str(), t))
        .collect();

    let mut task_comparisons = Vec::new();
    let mut improvements = 0;
    let mut regressions = 0;

    for current_task in &current.tasks {
        let baseline_task = baseline_task_map.get(current_task.task_id.as_str());

        if let Some(baseline_task) = baseline_task {
            let outcome_changed = baseline_task.outcome != current_task.outcome;
            let baseline_success = baseline_task.outcome == TaskOutcome::Success;
            let current_success = current_task.outcome == TaskOutcome::Success;

            let mut improved = false;
            let mut regressed = false;

            if outcome_changed {
                if !baseline_success && current_success {
                    improved = true;
                    improvements += 1;
                } else if baseline_success && !current_success {
                    regressed = true;
                    regressions += 1;
                }
            }

            task_comparisons.push(TaskComparison {
                task_id: current_task.task_id.clone(),
                task_title: current_task.task_title.clone(),
                baseline_outcome: baseline_task.outcome.as_str().to_string(),
                current_outcome: current_task.outcome.as_str().to_string(),
                outcome_changed,
                improved,
                regressed,
                turns_delta: current_task.turns as i32 - baseline_task.turns as i32,
                tokens_delta: current_task.total_tokens() as i64 - baseline_task.total_tokens() as i64,
                duration_delta_ms: current_task.duration_ms as i64 - baseline_task.duration_ms as i64,
            });
        } else {
            // New task, not in baseline
            task_comparisons.push(TaskComparison {
                task_id: current_task.task_id.clone(),
                task_title: current_task.task_title.clone(),
                baseline_outcome: "N/A".to_string(),
                current_outcome: current_task.outcome.as_str().to_string(),
                outcome_changed: true,
                improved: current_task.outcome == TaskOutcome::Success,
                regressed: false,
                turns_delta: 0,
                tokens_delta: 0,
                duration_delta_ms: 0,
            });
            if current_task.outcome == TaskOutcome::Success {
                improvements += 1;
            }
        }
    }

    // Determine overall verdict
    let completion_delta = current.summary.task_completion_rate - baseline.summary.task_completion_rate;

    let overall_verdict = if improvements > 0 && regressions == 0 && completion_delta >= 0.0 {
        ComparisonVerdict::Improved
    } else if regressions > 0 && improvements == 0 && completion_delta <= 0.0 {
        ComparisonVerdict::Regressed
    } else if improvements > 0 || regressions > 0 {
        ComparisonVerdict::Mixed
    } else {
        ComparisonVerdict::Unchanged
    };

    ComparisonReport {
        generated_at: Utc::now(),
        baseline: baseline.meta.clone(),
        current: current.meta.clone(),
        summary_deltas,
        task_comparisons,
        overall_verdict,
    }
}

// ============================================================================
// Formatters
// ============================================================================

fn format_percent(value: f64) -> String {
    let sign = if value >= 0.0 { "+" } else { "" };
    format!("{}{:.1}%", sign, value)
}

fn format_number(value: f64, decimals: usize) -> String {
    let sign = if value >= 0.0 { "+" } else { "" };
    format!("{}{:.prec$}", sign, value, prec = decimals)
}

fn format_duration(ms: i64) -> String {
    let abs_ms = ms.abs();
    let sign = if ms < 0 { "-" } else { "+" };
    if abs_ms < 1000 {
        format!("{}{}ms", sign, abs_ms)
    } else if abs_ms < 60000 {
        format!("{}{:.1}s", sign, abs_ms as f64 / 1000.0)
    } else {
        format!("{}{:.1}m", sign, abs_ms as f64 / 60000.0)
    }
}

fn format_duration_abs(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{:.1}m", ms as f64 / 60000.0)
    }
}

/// Format comparison report as markdown.
pub fn format_markdown_report(report: &ComparisonReport) -> String {
    let mut lines = Vec::new();

    lines.push("# Benchmark Comparison Report".to_string());
    lines.push(String::new());
    lines.push(format!("Generated: {}", report.generated_at.to_rfc3339()));
    lines.push(String::new());

    // Run info
    lines.push("## Runs Compared".to_string());
    lines.push(String::new());
    lines.push("| | Baseline | Current |".to_string());
    lines.push("|---|---|---|".to_string());
    lines.push(format!("| Run ID | {} | {} |", report.baseline.run_id, report.current.run_id));
    lines.push(format!("| Model | {} | {} |", report.baseline.model, report.current.model));
    lines.push(format!(
        "| Completed | {} | {} |",
        report.baseline.completed_at.format("%Y-%m-%d %H:%M"),
        report.current.completed_at.format("%Y-%m-%d %H:%M")
    ));
    lines.push(String::new());

    // Overall verdict
    lines.push(format!(
        "## Overall Verdict: {} {:?}",
        report.overall_verdict.emoji(),
        report.overall_verdict
    ).to_uppercase());
    lines.push(String::new());

    // Summary deltas
    lines.push("## Summary Metrics".to_string());
    lines.push(String::new());
    lines.push("| Metric | Baseline | Current | Delta | Trend |".to_string());
    lines.push("|--------|----------|---------|-------|-------|".to_string());

    for delta in &report.summary_deltas {
        let trend = if delta.improved {
            "⬆️"
        } else if delta.delta == 0.0 {
            "➖"
        } else {
            "⬇️"
        };

        let (baseline_str, current_str) = if delta.metric.contains("rate") {
            (
                format!("{:.1}%", delta.baseline * 100.0),
                format!("{:.1}%", delta.current * 100.0),
            )
        } else if delta.metric.contains("duration") {
            (
                format_duration_abs(delta.baseline as u64),
                format_duration_abs(delta.current as u64),
            )
        } else {
            (format!("{:.1}", delta.baseline), format!("{:.1}", delta.current))
        };

        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            delta.metric,
            baseline_str,
            current_str,
            format_percent(delta.delta_percent),
            trend
        ));
    }
    lines.push(String::new());

    // Task comparisons - outcome changes
    let changed_tasks: Vec<_> = report.task_comparisons.iter().filter(|t| t.outcome_changed).collect();
    if !changed_tasks.is_empty() {
        lines.push("## Task Outcome Changes".to_string());
        lines.push(String::new());
        lines.push("| Task | Baseline | Current | Status |".to_string());
        lines.push("|------|----------|---------|--------|".to_string());

        for task in changed_tasks {
            let status = if task.improved {
                "✅ Improved"
            } else if task.regressed {
                "❌ Regressed"
            } else {
                "➖ Changed"
            };
            lines.push(format!(
                "| {} | {} | {} | {} |",
                task.task_title, task.baseline_outcome, task.current_outcome, status
            ));
        }
        lines.push(String::new());
    }

    // Efficiency changes for successful tasks
    let successful_tasks: Vec<_> = report
        .task_comparisons
        .iter()
        .filter(|t| t.current_outcome == "success" && t.baseline_outcome == "success")
        .collect();
    if !successful_tasks.is_empty() {
        lines.push("## Efficiency Changes (Successful Tasks)".to_string());
        lines.push(String::new());
        lines.push("| Task | Turns Δ | Tokens Δ | Duration Δ |".to_string());
        lines.push("|------|---------|----------|------------|".to_string());

        for task in successful_tasks {
            lines.push(format!(
                "| {} | {} | {} | {} |",
                task.task_title,
                format_number(task.turns_delta as f64, 0),
                format_number(task.tokens_delta as f64, 0),
                format_duration(task.duration_delta_ms)
            ));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Format a single benchmark run as a summary markdown.
pub fn format_run_summary(results: &BenchmarkResults) -> String {
    let mut lines = Vec::new();

    lines.push("# Benchmark Run Summary".to_string());
    lines.push(String::new());
    lines.push(format!("- **Run ID:** {}", results.meta.run_id));
    lines.push(format!("- **Model:** {}", results.meta.model));
    if let Some(ref project_id) = results.meta.project_id {
        lines.push(format!("- **Project:** {}", project_id));
    }
    lines.push(format!("- **Started:** {}", results.meta.started_at.to_rfc3339()));
    lines.push(format!("- **Completed:** {}", results.meta.completed_at.to_rfc3339()));
    if let Some(ref branch) = results.meta.git_branch {
        lines.push(format!("- **Branch:** {}", branch));
    }
    if let Some(ref commit) = results.meta.git_commit {
        lines.push(format!("- **Commit:** {}", commit));
    }
    lines.push(String::new());

    lines.push("## Summary".to_string());
    lines.push(String::new());
    lines.push("| Metric | Value |".to_string());
    lines.push("|--------|-------|".to_string());
    lines.push(format!("| Total Tasks | {} |", results.summary.total_tasks));
    lines.push(format!("| Successful | {} |", results.summary.successful_tasks));
    lines.push(format!("| Failed | {} |", results.summary.failed_tasks));
    lines.push(format!("| Timeout | {} |", results.summary.timeout_tasks));
    lines.push(format!("| Error | {} |", results.summary.error_tasks));
    lines.push(format!(
        "| Completion Rate | {:.1}% |",
        results.summary.task_completion_rate * 100.0
    ));
    lines.push(format!(
        "| Verification Pass Rate | {:.1}% |",
        results.summary.verification_pass_rate * 100.0
    ));
    lines.push(format!(
        "| Avg Tokens/Task | {:.0} |",
        results.summary.avg_tokens_per_task
    ));
    lines.push(format!(
        "| Avg Turns/Task | {:.1} |",
        results.summary.avg_turns_per_task
    ));
    lines.push(format!(
        "| Avg Tool Calls/Task | {:.1} |",
        results.summary.avg_tool_calls_per_task
    ));
    lines.push(format!(
        "| Retry Rate | {:.1}% |",
        results.summary.retry_rate * 100.0
    ));
    lines.push(format!(
        "| Total Duration | {} |",
        format_duration_abs(results.summary.total_duration_ms)
    ));
    lines.push(String::new());

    // Tool distribution
    if !results.summary.tool_distribution.is_empty() {
        lines.push("## Tool Distribution".to_string());
        lines.push(String::new());
        lines.push("| Tool | Calls |".to_string());
        lines.push("|------|-------|".to_string());

        let mut sorted: Vec<_> = results.summary.tool_distribution.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (tool, count) in sorted {
            lines.push(format!("| {} | {} |", tool, count));
        }
        lines.push(String::new());
    }

    // Task details
    lines.push("## Task Results".to_string());
    lines.push(String::new());
    lines.push("| Task | Outcome | Turns | Tokens | Duration |".to_string());
    lines.push("|------|---------|-------|--------|----------|".to_string());

    for task in &results.tasks {
        lines.push(format!(
            "| {} | {} {} | {} | {} | {} |",
            task.task_title,
            task.outcome.emoji(),
            task.outcome.as_str(),
            task.turns,
            task.total_tokens(),
            format_duration_abs(task.duration_ms)
        ));
    }
    lines.push(String::new());

    lines.join("\n")
}

// ============================================================================
// Terminal-Bench Reporting
// ============================================================================

/// Task status for TB2 results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TB2Status {
    Pass,
    Fail,
    Timeout,
    Error,
    Skip,
}

/// TB2 task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2Result {
    pub task_id: String,
    pub status: TB2Status,
    pub duration_ms: u64,
    pub turns: u32,
    pub tokens_used: u64,
}

/// TB2 results container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2Results {
    pub suite_name: String,
    pub suite_version: String,
    pub model: String,
    pub timestamp: DateTime<Utc>,
    pub results: Vec<TB2Result>,
}

/// TB2 task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2Task {
    pub id: String,
    pub category: Option<String>,
}

/// TB2 suite definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2Suite {
    pub name: String,
    pub version: String,
    pub tasks: Vec<TB2Task>,
}

/// Category summary for TB2 reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2CategorySummary {
    pub category: String,
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub timeout: u32,
    pub error: u32,
    pub skipped: u32,
    pub pass_rate: f64,
    pub avg_duration_ms: f64,
    pub avg_turns: f64,
    pub total_tokens: u64,
}

impl TB2CategorySummary {
    fn compute(category: &str, results: &[&TB2Result]) -> Self {
        let total = results.len() as u32;
        let passed = results.iter().filter(|r| r.status == TB2Status::Pass).count() as u32;
        let failed = results.iter().filter(|r| r.status == TB2Status::Fail).count() as u32;
        let timeout = results.iter().filter(|r| r.status == TB2Status::Timeout).count() as u32;
        let error = results.iter().filter(|r| r.status == TB2Status::Error).count() as u32;
        let skipped = results.iter().filter(|r| r.status == TB2Status::Skip).count() as u32;

        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();
        let total_turns: u32 = results.iter().map(|r| r.turns).sum();
        let total_tokens: u64 = results.iter().map(|r| r.tokens_used).sum();

        Self {
            category: category.to_string(),
            total,
            passed,
            failed,
            timeout,
            error,
            skipped,
            pass_rate: if total > 0 { passed as f64 / total as f64 } else { 0.0 },
            avg_duration_ms: if total > 0 { total_duration as f64 / total as f64 } else { 0.0 },
            avg_turns: if total > 0 { total_turns as f64 / total as f64 } else { 0.0 },
            total_tokens,
        }
    }
}

/// Full TB2 report with category breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TB2Report {
    pub suite_name: String,
    pub suite_version: String,
    pub model: String,
    pub timestamp: DateTime<Utc>,
    pub overall: TB2CategorySummary,
    pub categories: Vec<TB2CategorySummary>,
}

/// Build a TB2 report with category breakdown.
pub fn build_terminal_bench_report(suite: &TB2Suite, results: &TB2Results) -> TB2Report {
    // Map task IDs to categories
    let category_map: HashMap<&str, &str> = suite
        .tasks
        .iter()
        .map(|t| (t.id.as_str(), t.category.as_deref().unwrap_or("uncategorized")))
        .collect();

    // Bucket results by category
    let mut bucket: HashMap<&str, Vec<&TB2Result>> = HashMap::new();
    for result in &results.results {
        let category = category_map.get(result.task_id.as_str()).copied().unwrap_or("uncategorized");
        bucket.entry(category).or_default().push(result);
    }

    // Compute category summaries
    let mut categories: Vec<_> = bucket
        .iter()
        .map(|(cat, res)| TB2CategorySummary::compute(cat, res))
        .collect();
    categories.sort_by(|a, b| a.category.cmp(&b.category));

    // Compute overall summary
    let all_results: Vec<_> = results.results.iter().collect();
    let overall = TB2CategorySummary::compute("overall", &all_results);

    TB2Report {
        suite_name: results.suite_name.clone(),
        suite_version: results.suite_version.clone(),
        model: results.model.clone(),
        timestamp: results.timestamp,
        overall,
        categories,
    }
}

/// Format TB2 report as markdown.
pub fn format_terminal_bench_markdown(report: &TB2Report) -> String {
    let mut lines = Vec::new();

    lines.push("# Terminal-Bench Report".to_string());
    lines.push(String::new());
    lines.push(format!("- **Suite:** {} (v{})", report.suite_name, report.suite_version));
    lines.push(format!("- **Model:** {}", report.model));
    lines.push(format!("- **Timestamp:** {}", report.timestamp.to_rfc3339()));
    lines.push(String::new());

    let overall = &report.overall;
    lines.push("## Overall Summary".to_string());
    lines.push(String::new());
    lines.push(
        "| Total | Passed | Failed | Timeout | Error | Skipped | Pass Rate | Avg Duration (ms) | Avg Turns | Total Tokens |"
            .to_string(),
    );
    lines.push(
        "|-------|--------|--------|---------|-------|---------|-----------|-------------------|-----------|--------------|"
            .to_string(),
    );
    lines.push(format!(
        "| {} | {} | {} | {} | {} | {} | {:.1}% | {:.1} | {:.1} | {} |",
        overall.total,
        overall.passed,
        overall.failed,
        overall.timeout,
        overall.error,
        overall.skipped,
        overall.pass_rate * 100.0,
        overall.avg_duration_ms,
        overall.avg_turns,
        overall.total_tokens
    ));
    lines.push(String::new());

    if !report.categories.is_empty() {
        lines.push("## By Category".to_string());
        lines.push(String::new());
        lines.push(
            "| Category | Total | Passed | Failed | Timeout | Error | Skipped | Pass Rate | Avg Duration (ms) | Avg Turns | Total Tokens |"
                .to_string(),
        );
        lines.push(
            "|----------|-------|--------|--------|---------|-------|---------|-----------|-------------------|-----------|--------------|"
                .to_string(),
        );
        for category in &report.categories {
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {:.1}% | {:.1} | {:.1} | {} |",
                category.category,
                category.total,
                category.passed,
                category.failed,
                category.timeout,
                category.error,
                category.skipped,
                category.pass_rate * 100.0,
                category.avg_duration_ms,
                category.avg_turns,
                category.total_tokens
            ));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results(run_id: &str, model: &str, success_count: u32) -> BenchmarkResults {
        let now = Utc::now();
        let mut tasks = Vec::new();

        for i in 0..4 {
            tasks.push(TaskMetrics {
                task_id: format!("task-{}", i + 1),
                task_title: format!("Task {}", i + 1),
                outcome: if i < success_count as usize {
                    TaskOutcome::Success
                } else {
                    TaskOutcome::Failure
                },
                turns: 5 + i as u32,
                input_tokens: 1000 + i as u64 * 100,
                output_tokens: 500 + i as u64 * 50,
                duration_ms: 10000 + i as u64 * 1000,
            });
        }

        BenchmarkResults {
            meta: RunMeta {
                run_id: run_id.to_string(),
                model: model.to_string(),
                project_id: Some("test".to_string()),
                started_at: now,
                completed_at: now,
                git_branch: None,
                git_commit: None,
            },
            summary: BenchmarkSummary {
                total_tasks: 4,
                successful_tasks: success_count,
                failed_tasks: 4 - success_count,
                timeout_tasks: 0,
                error_tasks: 0,
                task_completion_rate: success_count as f64 / 4.0,
                verification_pass_rate: success_count as f64 / 4.0,
                avg_tokens_per_task: 1500.0,
                avg_turns_per_task: 6.5,
                avg_tool_calls_per_task: 10.0,
                retry_rate: 0.1,
                total_duration_ms: 46000,
                tool_distribution: HashMap::new(),
            },
            tasks,
        }
    }

    #[test]
    fn test_metric_delta_compute() {
        let delta = MetricDelta::compute("test", 100.0, 80.0, true);
        assert!(delta.improved);
        assert_eq!(delta.delta, -20.0);

        let delta = MetricDelta::compute("test", 100.0, 120.0, false);
        assert!(delta.improved);
        assert_eq!(delta.delta, 20.0);
    }

    #[test]
    fn test_compare_runs_improved() {
        let baseline = sample_results("run-1", "fm", 2);
        let current = sample_results("run-2", "fm", 3);

        let report = compare_runs(&baseline, &current);

        assert_eq!(report.overall_verdict, ComparisonVerdict::Improved);
    }

    #[test]
    fn test_compare_runs_regressed() {
        let baseline = sample_results("run-1", "fm", 3);
        let current = sample_results("run-2", "fm", 2);

        let report = compare_runs(&baseline, &current);

        assert_eq!(report.overall_verdict, ComparisonVerdict::Regressed);
    }

    #[test]
    fn test_compare_runs_unchanged() {
        let baseline = sample_results("run-1", "fm", 2);
        let current = sample_results("run-2", "fm", 2);

        let report = compare_runs(&baseline, &current);

        assert_eq!(report.overall_verdict, ComparisonVerdict::Unchanged);
    }

    #[test]
    fn test_format_markdown_report() {
        let baseline = sample_results("run-1", "fm", 2);
        let current = sample_results("run-2", "fm", 3);
        let report = compare_runs(&baseline, &current);

        let markdown = format_markdown_report(&report);

        assert!(markdown.contains("Benchmark Comparison Report"));
        assert!(markdown.contains("Summary Metrics"));
        assert!(markdown.contains("run-1"));
        assert!(markdown.contains("run-2"));
    }

    #[test]
    fn test_format_run_summary() {
        let results = sample_results("run-1", "fm", 3);
        let markdown = format_run_summary(&results);

        assert!(markdown.contains("Benchmark Run Summary"));
        assert!(markdown.contains("run-1"));
        assert!(markdown.contains("Task Results"));
    }

    #[test]
    fn test_tb2_category_summary() {
        let results = vec![
            TB2Result {
                task_id: "t1".to_string(),
                status: TB2Status::Pass,
                duration_ms: 1000,
                turns: 5,
                tokens_used: 500,
            },
            TB2Result {
                task_id: "t2".to_string(),
                status: TB2Status::Fail,
                duration_ms: 2000,
                turns: 8,
                tokens_used: 800,
            },
        ];
        let refs: Vec<_> = results.iter().collect();
        let summary = TB2CategorySummary::compute("test", &refs);

        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.pass_rate, 0.5);
    }

    #[test]
    fn test_format_terminal_bench_markdown() {
        let report = TB2Report {
            suite_name: "regex-log".to_string(),
            suite_version: "1.0".to_string(),
            model: "fm".to_string(),
            timestamp: Utc::now(),
            overall: TB2CategorySummary {
                category: "overall".to_string(),
                total: 10,
                passed: 7,
                failed: 3,
                timeout: 0,
                error: 0,
                skipped: 0,
                pass_rate: 0.7,
                avg_duration_ms: 1500.0,
                avg_turns: 6.0,
                total_tokens: 10000,
            },
            categories: vec![],
        };

        let markdown = format_terminal_bench_markdown(&report);

        assert!(markdown.contains("Terminal-Bench Report"));
        assert!(markdown.contains("regex-log"));
        assert!(markdown.contains("70.0%"));
    }
}
