//! Trajectory analysis and metrics computation for autopilot performance monitoring.
//!
//! This module provides comprehensive analysis of autopilot execution trajectories,
//! extracting performance, cost, quality, and error metrics from trajectory logs.
//! Used by the `cargo autopilot analyze` command to generate reports and detect
//! regressions.
//!
//! # Key Types
//!
//! - [`TrajectoryAnalysis`]: Complete analysis results for a single trajectory
//! - [`PerformanceMetrics`]: Timing and duration measurements
//! - [`CostMetrics`]: Token usage and API cost calculations
//! - [`ErrorMetrics`]: Tool errors and failure patterns
//! - [`QualityMetrics`]: Task completion and success indicators
//! - [`ToolUsageMetrics`]: Per-tool usage statistics
//!
//! # Usage
//!
//! ```no_run
//! use autopilot::analyze::analyze_trajectory_file;
//! use std::path::Path;
//!
//! let trajectory_path = Path::new("docs/logs/20251222/094938-task.json");
//! let analysis = analyze_trajectory_file(trajectory_path)?;
//!
//! println!("Duration: {}ms", analysis.performance.total_duration_ms);
//! println!("Tokens: {}", analysis.cost.total_tokens);
//! println!("Error rate: {:.1}%", analysis.errors.error_rate * 100.0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Related Modules
//!
//! - [`crate::dashboard`]: Web UI for viewing analysis results
//! - [`crate::trajectory`]: Trajectory log parsing and types
//! - [`crate::learning`]: Apply learnings from analysis

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use colored::*;
use serde::{Deserialize, Serialize};

use crate::trajectory::{StepType, Trajectory};

/// Complete analysis of a single trajectory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryAnalysis {
    pub session_id: String,
    pub model: String,
    pub prompt_preview: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub performance: PerformanceMetrics,
    pub cost: CostMetrics,
    pub errors: ErrorMetrics,
    pub quality: QualityMetrics,
    pub tool_usage: ToolUsageMetrics,
}

/// Performance timing metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_duration_ms: u64,
    pub tool_latency_stats: LatencyStats,
    pub parallel_tool_batches: u32,
    pub total_tool_calls: u32,
}

/// Latency statistics for distributions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    pub min_ms: u64,
    pub max_ms: u64,
    pub mean_ms: f64,
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub count: usize,
}

/// Cost and token metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostMetrics {
    pub total_cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_hit_rate: f64,
    pub tokens_by_step_type: HashMap<String, u64>,
}

/// Error and failure metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub success: bool,
    pub total_tool_calls: u32,
    pub failed_tool_calls: u32,
    pub tool_error_rate: f64,
    pub errors_by_tool: HashMap<String, u32>,
}

/// Quality and behavioral metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub num_turns: u32,
    pub thinking_blocks: u32,
    pub avg_thinking_length: f64,
    pub tool_diversity: f64,
    pub unique_tools: u32,
}

/// Tool usage breakdown
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolUsageMetrics {
    pub total_calls: u32,
    pub calls_by_tool: HashMap<String, u32>,
    pub success_rate_by_tool: HashMap<String, f64>,
    pub most_used_tool: Option<String>,
}

/// Aggregated metrics across multiple trajectories
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateAnalysis {
    pub trajectory_count: usize,
    pub date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub by_model: HashMap<String, ModelStats>,
    pub total_cost_usd: f64,
    pub avg_cost_usd: f64,
    pub avg_duration_ms: f64,
    pub overall_success_rate: f64,
    pub avg_tool_error_rate: f64,
    pub avg_cache_hit_rate: f64,
    pub total_tool_calls: u32,
    pub top_error_tools: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelStats {
    pub count: usize,
    pub avg_cost: f64,
    pub avg_duration_ms: f64,
    pub success_rate: f64,
}

/// Load a trajectory from a JSON file
pub fn load_trajectory(path: &Path) -> anyhow::Result<Trajectory> {
    let content = std::fs::read_to_string(path)?;
    let trajectory: Trajectory = serde_json::from_str(&content)?;
    Ok(trajectory)
}

/// Analyze a single trajectory
pub fn analyze_trajectory(trajectory: &Trajectory) -> TrajectoryAnalysis {
    let performance = compute_performance(trajectory);
    let cost = compute_cost(trajectory);
    let errors = compute_errors(trajectory);
    let quality = compute_quality(trajectory);
    let tool_usage = compute_tool_usage(trajectory);

    TrajectoryAnalysis {
        session_id: trajectory.session_id.clone(),
        model: trajectory.model.clone(),
        prompt_preview: truncate(&trajectory.prompt, 80),
        started_at: trajectory.started_at,
        ended_at: trajectory.ended_at,
        performance,
        cost,
        errors,
        quality,
        tool_usage,
    }
}

/// Compute performance metrics
fn compute_performance(trajectory: &Trajectory) -> PerformanceMetrics {
    let total_duration_ms = trajectory
        .result
        .as_ref()
        .map(|r| r.duration_ms)
        .unwrap_or(0);

    // Track tool call timestamps to compute latencies
    let mut pending_calls: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut latencies: Vec<u64> = Vec::new();
    let mut tool_intervals: Vec<(DateTime<Utc>, DateTime<Utc>)> = Vec::new();
    let mut total_tool_calls = 0u32;

    for step in &trajectory.steps {
        match &step.step_type {
            StepType::ToolCall { tool_id, .. } => {
                total_tool_calls += 1;
                pending_calls.insert(tool_id.clone(), step.timestamp);
            }
            StepType::ToolResult { tool_id, .. } => {
                if let Some(start_time) = pending_calls.remove(tool_id) {
                    let latency = (step.timestamp - start_time).num_milliseconds().max(0) as u64;
                    latencies.push(latency);
                    tool_intervals.push((start_time, step.timestamp));
                }
            }
            _ => {}
        }
    }

    let tool_latency_stats = compute_latency_stats(&latencies);
    let parallel_tool_batches = detect_parallel_batches(&tool_intervals);

    PerformanceMetrics {
        total_duration_ms,
        tool_latency_stats,
        parallel_tool_batches,
        total_tool_calls,
    }
}

/// Compute latency statistics from a list of latencies
fn compute_latency_stats(latencies: &[u64]) -> LatencyStats {
    if latencies.is_empty() {
        return LatencyStats::default();
    }

    let mut sorted = latencies.to_vec();
    sorted.sort();

    let count = sorted.len();
    let sum: u64 = sorted.iter().sum();

    LatencyStats {
        min_ms: sorted[0],
        max_ms: sorted[count - 1],
        mean_ms: sum as f64 / count as f64,
        p50_ms: sorted[count / 2],
        p95_ms: sorted[(count * 95 / 100).min(count - 1)],
        count,
    }
}

/// Detect number of parallel tool batches (overlapping tool calls)
fn detect_parallel_batches(intervals: &[(DateTime<Utc>, DateTime<Utc>)]) -> u32 {
    if intervals.is_empty() {
        return 0;
    }

    let mut sorted: Vec<_> = intervals.to_vec();
    sorted.sort_by_key(|(start, _)| *start);

    let mut batches = 1u32;
    let mut current_batch_end = sorted[0].1;

    for (start, end) in sorted.iter().skip(1) {
        if *start > current_batch_end {
            // New batch - no overlap
            batches += 1;
            current_batch_end = *end;
        } else {
            // Overlapping - extend batch
            current_batch_end = std::cmp::max(current_batch_end, *end);
        }
    }

    batches
}

/// Compute cost metrics
fn compute_cost(trajectory: &Trajectory) -> CostMetrics {
    let usage = &trajectory.usage;

    // Compute cache hit rate
    let total_input = usage.input_tokens + usage.cache_read_tokens;
    let cache_hit_rate = if total_input > 0 {
        usage.cache_read_tokens as f64 / total_input as f64
    } else {
        0.0
    };

    // Tokens by step type
    let mut tokens_by_step_type: HashMap<String, u64> = HashMap::new();

    for step in &trajectory.steps {
        let step_name = match &step.step_type {
            StepType::Thinking { .. } => "thinking",
            StepType::Assistant { .. } => "assistant",
            StepType::ToolCall { .. } => "tool_call",
            StepType::ToolResult { .. } => "tool_result",
            StepType::User { .. } => "user",
            StepType::SystemInit { .. } => "system_init",
            StepType::SystemStatus { .. } => "system_status",
            StepType::Subagent { .. } => "subagent",
        };

        let entry = tokens_by_step_type.entry(step_name.to_string()).or_insert(0);
        if let Some(t) = step.tokens_out {
            *entry += t;
        }
    }

    CostMetrics {
        total_cost_usd: usage.cost_usd,
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_read_tokens: usage.cache_read_tokens,
        cache_hit_rate,
        tokens_by_step_type,
    }
}

/// Compute error metrics
fn compute_errors(trajectory: &Trajectory) -> ErrorMetrics {
    let mut total_tool_calls = 0u32;
    let mut failed_tool_calls = 0u32;
    let mut errors_by_tool: HashMap<String, u32> = HashMap::new();
    let mut pending_tools: HashMap<String, String> = HashMap::new(); // tool_id -> tool_name

    for step in &trajectory.steps {
        match &step.step_type {
            StepType::ToolCall { tool, tool_id, .. } => {
                total_tool_calls += 1;
                pending_tools.insert(tool_id.clone(), tool.clone());
            }
            StepType::ToolResult { tool_id, success, .. } => {
                if !*success {
                    failed_tool_calls += 1;
                    if let Some(tool_name) = pending_tools.get(tool_id) {
                        *errors_by_tool.entry(tool_name.clone()).or_insert(0) += 1;
                    }
                }
            }
            _ => {}
        }
    }

    let tool_error_rate = if total_tool_calls > 0 {
        failed_tool_calls as f64 / total_tool_calls as f64
    } else {
        0.0
    };

    ErrorMetrics {
        success: trajectory.result.as_ref().map(|r| r.success).unwrap_or(false),
        total_tool_calls,
        failed_tool_calls,
        tool_error_rate,
        errors_by_tool,
    }
}

/// Compute quality metrics
fn compute_quality(trajectory: &Trajectory) -> QualityMetrics {
    let mut thinking_blocks = 0u32;
    let mut thinking_lengths: Vec<usize> = Vec::new();
    let mut unique_tools: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut tool_call_count = 0u32;

    for step in &trajectory.steps {
        match &step.step_type {
            StepType::Thinking { content, .. } => {
                thinking_blocks += 1;
                thinking_lengths.push(content.len());
            }
            StepType::ToolCall { tool, .. } => {
                tool_call_count += 1;
                unique_tools.insert(tool.clone());
            }
            _ => {}
        }
    }

    let avg_thinking_length = if thinking_lengths.is_empty() {
        0.0
    } else {
        thinking_lengths.iter().sum::<usize>() as f64 / thinking_lengths.len() as f64
    };

    let tool_diversity = if tool_call_count > 0 {
        unique_tools.len() as f64 / tool_call_count as f64
    } else {
        0.0
    };

    QualityMetrics {
        num_turns: trajectory.result.as_ref().map(|r| r.num_turns).unwrap_or(0),
        thinking_blocks,
        avg_thinking_length,
        tool_diversity,
        unique_tools: unique_tools.len() as u32,
    }
}

/// Compute tool usage metrics
fn compute_tool_usage(trajectory: &Trajectory) -> ToolUsageMetrics {
    let mut calls_by_tool: HashMap<String, u32> = HashMap::new();
    let mut successes_by_tool: HashMap<String, u32> = HashMap::new();
    let mut pending_tools: HashMap<String, String> = HashMap::new();

    for step in &trajectory.steps {
        match &step.step_type {
            StepType::ToolCall { tool, tool_id, .. } => {
                *calls_by_tool.entry(tool.clone()).or_insert(0) += 1;
                pending_tools.insert(tool_id.clone(), tool.clone());
            }
            StepType::ToolResult { tool_id, success, .. } => {
                if *success {
                    if let Some(tool_name) = pending_tools.get(tool_id) {
                        *successes_by_tool.entry(tool_name.clone()).or_insert(0) += 1;
                    }
                }
            }
            _ => {}
        }
    }

    let total_calls: u32 = calls_by_tool.values().sum();

    let success_rate_by_tool: HashMap<String, f64> = calls_by_tool
        .iter()
        .map(|(tool, count)| {
            let successes = successes_by_tool.get(tool).copied().unwrap_or(0);
            let rate = if *count > 0 {
                successes as f64 / *count as f64
            } else {
                0.0
            };
            (tool.clone(), rate)
        })
        .collect();

    let most_used_tool = calls_by_tool
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(tool, _)| tool.clone());

    ToolUsageMetrics {
        total_calls,
        calls_by_tool,
        success_rate_by_tool,
        most_used_tool,
    }
}

/// Aggregate analysis across multiple trajectories
pub fn aggregate_analyses(analyses: &[TrajectoryAnalysis]) -> AggregateAnalysis {
    if analyses.is_empty() {
        return AggregateAnalysis::default();
    }

    let trajectory_count = analyses.len();

    // Date range
    let min_date = analyses.iter().map(|a| a.started_at).min();
    let max_date = analyses.iter().filter_map(|a| a.ended_at).max();
    let date_range = min_date.and_then(|min| max_date.map(|max| (min, max)));

    // By model stats
    let mut by_model: HashMap<String, Vec<&TrajectoryAnalysis>> = HashMap::new();
    for analysis in analyses {
        by_model
            .entry(analysis.model.clone())
            .or_default()
            .push(analysis);
    }

    let by_model_stats: HashMap<String, ModelStats> = by_model
        .iter()
        .map(|(model, runs)| {
            let count = runs.len();
            let avg_cost = runs.iter().map(|r| r.cost.total_cost_usd).sum::<f64>() / count as f64;
            let avg_duration =
                runs.iter().map(|r| r.performance.total_duration_ms).sum::<u64>() as f64
                    / count as f64;
            let success_rate = runs.iter().filter(|r| r.errors.success).count() as f64 / count as f64;
            (
                model.clone(),
                ModelStats {
                    count,
                    avg_cost,
                    avg_duration_ms: avg_duration,
                    success_rate,
                },
            )
        })
        .collect();

    // Aggregates
    let total_cost_usd: f64 = analyses.iter().map(|a| a.cost.total_cost_usd).sum();
    let avg_cost_usd = total_cost_usd / trajectory_count as f64;
    let avg_duration_ms = analyses
        .iter()
        .map(|a| a.performance.total_duration_ms)
        .sum::<u64>() as f64
        / trajectory_count as f64;
    let overall_success_rate =
        analyses.iter().filter(|a| a.errors.success).count() as f64 / trajectory_count as f64;
    let avg_tool_error_rate =
        analyses.iter().map(|a| a.errors.tool_error_rate).sum::<f64>() / trajectory_count as f64;
    let avg_cache_hit_rate =
        analyses.iter().map(|a| a.cost.cache_hit_rate).sum::<f64>() / trajectory_count as f64;
    let total_tool_calls: u32 = analyses.iter().map(|a| a.tool_usage.total_calls).sum();

    // Top error tools
    let mut error_tools: HashMap<String, u32> = HashMap::new();
    for analysis in analyses {
        for (tool, count) in &analysis.errors.errors_by_tool {
            *error_tools.entry(tool.clone()).or_insert(0) += count;
        }
    }
    let mut top_error_tools: Vec<(String, u32)> = error_tools.into_iter().collect();
    top_error_tools.sort_by(|a, b| b.1.cmp(&a.1));
    top_error_tools.truncate(5);

    AggregateAnalysis {
        trajectory_count,
        date_range,
        by_model: by_model_stats,
        total_cost_usd,
        avg_cost_usd,
        avg_duration_ms,
        overall_success_rate,
        avg_tool_error_rate,
        avg_cache_hit_rate,
        total_tool_calls,
        top_error_tools,
    }
}

/// Print analysis in human-readable format
pub fn print_analysis(analysis: &TrajectoryAnalysis) {
    let sep = "=".repeat(80);
    println!("{}", sep.bright_blue());
    println!(
        "{}",
        format!("Trajectory Analysis: {}", &analysis.session_id[..8.min(analysis.session_id.len())])
            .bright_white()
            .bold()
    );
    println!("{}", sep.bright_blue());

    // Performance
    println!("\n{}", "PERFORMANCE".yellow().bold());
    println!(
        "  Duration:          {:.1}s",
        analysis.performance.total_duration_ms as f64 / 1000.0
    );
    if analysis.performance.tool_latency_stats.count > 0 {
        let stats = &analysis.performance.tool_latency_stats;
        println!(
            "  Tool Latency:      p50={}ms, p95={}ms (n={})",
            stats.p50_ms, stats.p95_ms, stats.count
        );
    }
    println!(
        "  Parallel Batches:  {}",
        analysis.performance.parallel_tool_batches
    );
    println!("  Total Tool Calls:  {}", analysis.performance.total_tool_calls);

    // Cost
    println!("\n{}", "COST".yellow().bold());
    println!("  Total:             ${:.4}", analysis.cost.total_cost_usd);
    println!(
        "  Input Tokens:      {}",
        format_tokens(analysis.cost.input_tokens)
    );
    println!(
        "  Output Tokens:     {}",
        format_tokens(analysis.cost.output_tokens)
    );
    println!("  Cache Hit Rate:    {:.1}%", analysis.cost.cache_hit_rate * 100.0);

    // Errors
    println!("\n{}", "ERRORS".yellow().bold());
    let success_str = if analysis.errors.success {
        "YES".green()
    } else {
        "NO".red()
    };
    println!("  Success:           {}", success_str);
    println!(
        "  Tool Error Rate:   {:.1}% ({}/{})",
        analysis.errors.tool_error_rate * 100.0,
        analysis.errors.failed_tool_calls,
        analysis.errors.total_tool_calls
    );
    if !analysis.errors.errors_by_tool.is_empty() {
        println!("  Errors by Tool:");
        for (tool, count) in &analysis.errors.errors_by_tool {
            println!("    {}: {}", tool, count);
        }
    }

    // Quality
    println!("\n{}", "QUALITY".yellow().bold());
    println!("  Turns:             {}", analysis.quality.num_turns);
    println!("  Thinking Blocks:   {}", analysis.quality.thinking_blocks);
    println!(
        "  Tool Diversity:    {:.0}% ({} unique)",
        analysis.quality.tool_diversity * 100.0,
        analysis.quality.unique_tools
    );

    // Tool Usage
    if !analysis.tool_usage.calls_by_tool.is_empty() {
        println!("\n{}", "TOOL USAGE".yellow().bold());
        let mut tools: Vec<_> = analysis.tool_usage.calls_by_tool.iter().collect();
        tools.sort_by(|a, b| b.1.cmp(a.1));
        for (tool, count) in tools.iter().take(10) {
            let success_rate = analysis
                .tool_usage
                .success_rate_by_tool
                .get(*tool)
                .copied()
                .unwrap_or(0.0);
            println!("  {:20} {} calls, {:.0}% success", tool, count, success_rate * 100.0);
        }
    }

    println!("\n{}", sep.bright_blue());
}

/// Print aggregate analysis
pub fn print_aggregate(analysis: &AggregateAnalysis) {
    let sep = "=".repeat(80);
    println!("{}", sep.bright_blue());
    println!(
        "{}",
        format!("Aggregate Analysis: {} trajectories", analysis.trajectory_count)
            .bright_white()
            .bold()
    );
    println!("{}", sep.bright_blue());

    // Overview
    println!("\n{}", "OVERVIEW".yellow().bold());
    println!("  Trajectories:      {}", analysis.trajectory_count);
    println!("  Total Cost:        ${:.4}", analysis.total_cost_usd);
    println!("  Avg Cost:          ${:.4}", analysis.avg_cost_usd);
    println!("  Avg Duration:      {:.1}s", analysis.avg_duration_ms / 1000.0);
    println!("  Success Rate:      {:.1}%", analysis.overall_success_rate * 100.0);
    println!("  Avg Tool Errors:   {:.1}%", analysis.avg_tool_error_rate * 100.0);
    println!("  Avg Cache Hit:     {:.1}%", analysis.avg_cache_hit_rate * 100.0);
    println!("  Total Tool Calls:  {}", analysis.total_tool_calls);

    // By Model
    if !analysis.by_model.is_empty() {
        println!("\n{}", "BY MODEL".yellow().bold());
        for (model, stats) in &analysis.by_model {
            let model_short = model.split('-').take(3).collect::<Vec<_>>().join("-");
            println!(
                "  {:30} {} runs, ${:.4} avg, {:.1}% success",
                model_short,
                stats.count,
                stats.avg_cost,
                stats.success_rate * 100.0
            );
        }
    }

    // Top Error Tools
    if !analysis.top_error_tools.is_empty() {
        println!("\n{}", "TOP ERROR TOOLS".yellow().bold());
        for (tool, count) in &analysis.top_error_tools {
            println!("  {:20} {} errors", tool, count);
        }
    }

    println!("\n{}", sep.bright_blue());
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

//
// ===== Aggregate Metrics Analysis =====
//

use crate::metrics::{MetricsDb, SessionMetrics};

/// Aggregate statistics for a metric dimension from metrics database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAggregateStats {
    pub dimension: String,
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
    pub stddev: f64,
}

/// Trend direction for a metric
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

/// Trend analysis for a metric over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricTrend {
    pub dimension: String,
    pub direction: TrendDirection,
    pub percent_change: f64,
    pub recent: MetricAggregateStats,
    pub baseline: Option<MetricAggregateStats>,
}

/// Detected regression in metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    pub dimension: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub percent_worse: f64,
    pub is_critical: bool,
    pub severity: RegressionSeverity,
    pub deviation_sigma: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionSeverity {
    Warning,
    Error,
    Critical,
}

/// Time period for analysis
#[derive(Debug, Clone, Copy)]
pub enum TimePeriod {
    Last7Days,
    Last30Days,
    LastWeek,
    ThisWeek,
    Custom { start: DateTime<Utc>, end: DateTime<Utc> },
}

impl TimePeriod {
    /// Get the start and end timestamps for this period
    pub fn bounds(&self) -> (DateTime<Utc>, DateTime<Utc>) {
        use chrono::Datelike;

        let now = Utc::now();
        match self {
            TimePeriod::Last7Days => (now - chrono::Duration::days(7), now),
            TimePeriod::Last30Days => (now - chrono::Duration::days(30), now),
            TimePeriod::LastWeek => {
                let days_since_monday = now.weekday().num_days_from_monday();
                let last_monday = now - chrono::Duration::days((days_since_monday + 7) as i64);
                let last_sunday = last_monday + chrono::Duration::days(6);
                (last_monday, last_sunday)
            }
            TimePeriod::ThisWeek => {
                let days_since_monday = now.weekday().num_days_from_monday();
                let this_monday = now - chrono::Duration::days(days_since_monday as i64);
                (this_monday, now)
            }
            TimePeriod::Custom { start, end } => (*start, *end),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TimePeriod::Last7Days => "Last 7 Days",
            TimePeriod::Last30Days => "Last 30 Days",
            TimePeriod::LastWeek => "Last Week",
            TimePeriod::ThisWeek => "This Week",
            TimePeriod::Custom { .. } => "Custom Period",
        }
    }
}

/// Calculate aggregate statistics from a list of values
fn calculate_metric_stats(dimension: &str, values: &[f64]) -> Option<MetricAggregateStats> {
    if values.is_empty() {
        return None;
    }

    use statrs::statistics::{Data, Distribution, OrderStatistics, Min, Max};

    let mut data = Data::new(values.to_vec());
    let mean = data.mean().unwrap_or(0.0);
    let stddev = data.std_dev().unwrap_or(0.0);
    let median = data.median();
    let p90 = data.percentile(90);
    let p99 = data.percentile(99);
    let min = data.min();
    let max = data.max();

    Some(MetricAggregateStats {
        dimension: dimension.to_string(),
        count: values.len(),
        mean,
        median,
        p90,
        p99,
        min,
        max,
        stddev,
    })
}

/// Get sessions within a time period
pub fn get_sessions_in_period(
    db: &MetricsDb,
    period: TimePeriod,
) -> anyhow::Result<Vec<SessionMetrics>> {
    let (start, end) = period.bounds();
    let all_sessions = db.get_all_sessions()?;

    Ok(all_sessions
        .into_iter()
        .filter(|s| s.timestamp >= start && s.timestamp <= end)
        .collect())
}

/// Get sessions between two specific dates
pub fn get_sessions_between_dates(
    db: &MetricsDb,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> anyhow::Result<Vec<SessionMetrics>> {
    let all_sessions = db.get_all_sessions()?;

    Ok(all_sessions
        .into_iter()
        .filter(|s| s.timestamp >= start && s.timestamp <= end)
        .collect())
}

/// Calculate aggregate statistics for all metrics from sessions
pub fn calculate_aggregate_stats_from_sessions(sessions: &[SessionMetrics]) -> HashMap<String, MetricAggregateStats> {
    let mut stats_map = HashMap::new();

    // Tool error rate
    let error_rates: Vec<f64> = sessions
        .iter()
        .filter(|s| s.tool_calls > 0)
        .map(|s| (s.tool_errors as f64) / (s.tool_calls as f64))
        .collect();
    if let Some(stats) = calculate_metric_stats("tool_error_rate", &error_rates) {
        stats_map.insert("tool_error_rate".to_string(), stats);
    }

    // Tokens per issue
    let tokens_per_issue: Vec<f64> = sessions
        .iter()
        .filter(|s| s.issues_completed > 0)
        .map(|s| ((s.tokens_in + s.tokens_out) as f64) / (s.issues_completed as f64))
        .collect();
    if let Some(stats) = calculate_metric_stats("tokens_per_issue", &tokens_per_issue) {
        stats_map.insert("tokens_per_issue".to_string(), stats);
    }

    // Duration per issue
    let duration_per_issue: Vec<f64> = sessions
        .iter()
        .filter(|s| s.issues_completed > 0)
        .map(|s| s.duration_seconds / (s.issues_completed as f64))
        .collect();
    if let Some(stats) = calculate_metric_stats("duration_per_issue", &duration_per_issue) {
        stats_map.insert("duration_per_issue".to_string(), stats);
    }

    // Cost per issue
    let cost_per_issue: Vec<f64> = sessions
        .iter()
        .filter(|s| s.issues_completed > 0)
        .map(|s| s.cost_usd / (s.issues_completed as f64))
        .collect();
    if let Some(stats) = calculate_metric_stats("cost_per_issue", &cost_per_issue) {
        stats_map.insert("cost_per_issue".to_string(), stats);
    }

    // Session duration
    let durations: Vec<f64> = sessions.iter().map(|s| s.duration_seconds).collect();
    if let Some(stats) = calculate_metric_stats("session_duration", &durations) {
        stats_map.insert("session_duration".to_string(), stats);
    }

    // Completion rate
    let completion_rates: Vec<f64> = sessions
        .iter()
        .filter(|s| s.issues_claimed > 0)
        .map(|s| (s.issues_completed as f64) / (s.issues_claimed as f64))
        .collect();
    if let Some(stats) = calculate_metric_stats("completion_rate", &completion_rates) {
        stats_map.insert("completion_rate".to_string(), stats);
    }

    stats_map
}

/// Detect trends by comparing recent vs baseline periods
pub fn detect_trends(
    db: &MetricsDb,
    recent_period: TimePeriod,
    baseline_period: Option<TimePeriod>,
) -> anyhow::Result<Vec<MetricTrend>> {
    let recent_sessions = get_sessions_in_period(db, recent_period)?;
    let recent_stats = calculate_aggregate_stats_from_sessions(&recent_sessions);

    let baseline_sessions = if let Some(bp) = baseline_period {
        Some(get_sessions_in_period(db, bp)?)
    } else {
        None
    };

    let baseline_stats = baseline_sessions
        .as_ref()
        .map(|sessions| calculate_aggregate_stats_from_sessions(sessions));

    let mut trends = Vec::new();

    for (dimension, recent) in recent_stats {
        let baseline = baseline_stats
            .as_ref()
            .and_then(|map| map.get(&dimension).cloned());

        let (direction, percent_change) = if let Some(ref base) = baseline {
            let change = if base.mean != 0.0 {
                ((recent.mean - base.mean) / base.mean) * 100.0
            } else {
                0.0
            };

            // Determine if change is improvement or degradation based on metric type
            let direction = match dimension.as_str() {
                "tool_error_rate" | "cost_per_issue" | "duration_per_issue" => {
                    // Lower is better
                    if change < -5.0 {
                        TrendDirection::Improving
                    } else if change > 5.0 {
                        TrendDirection::Degrading
                    } else {
                        TrendDirection::Stable
                    }
                }
                "completion_rate" => {
                    // Higher is better
                    if change > 5.0 {
                        TrendDirection::Improving
                    } else if change < -5.0 {
                        TrendDirection::Degrading
                    } else {
                        TrendDirection::Stable
                    }
                }
                _ => TrendDirection::Stable,
            };

            (direction, change)
        } else {
            (TrendDirection::Stable, 0.0)
        };

        trends.push(MetricTrend {
            dimension,
            direction,
            percent_change,
            recent,
            baseline,
        });
    }

    Ok(trends)
}

/// Detect regressions by comparing against baselines
pub fn detect_regressions(db: &MetricsDb, period: TimePeriod) -> anyhow::Result<Vec<Regression>> {
    let sessions = get_sessions_in_period(db, period)?;
    let current_stats = calculate_aggregate_stats_from_sessions(&sessions);
    let mut regressions = Vec::new();

    // Check each dimension against stored baseline
    for (dimension, current) in current_stats {
        if let Some(baseline) = db.get_baseline(&dimension)? {
            // Calculate standard deviation-based regression threshold
            // A metric is regressed if it's >2 standard deviations worse than baseline
            let deviation_sigma = if baseline.stddev > 0.0 {
                (current.mean - baseline.mean).abs() / baseline.stddev
            } else {
                0.0
            };

            let percent_worse = match dimension.as_str() {
                "tool_error_rate" | "cost_per_issue" | "duration_per_issue" => {
                    // Lower is better - check if current is higher than baseline
                    if current.mean > baseline.mean && baseline.mean > 0.0 {
                        ((current.mean - baseline.mean) / baseline.mean) * 100.0
                    } else {
                        continue; // Not a regression
                    }
                }
                "completion_rate" => {
                    // Higher is better - check if current is lower than baseline
                    if current.mean < baseline.mean && baseline.mean > 0.0 {
                        ((baseline.mean - current.mean) / baseline.mean) * 100.0
                    } else {
                        continue; // Not a regression
                    }
                }
                _ => continue,
            };

            // Determine severity based on both percent change and standard deviations
            let severity = if percent_worse > 50.0 || deviation_sigma > 3.0 {
                RegressionSeverity::Critical
            } else if percent_worse > 25.0 || deviation_sigma > 2.5 {
                RegressionSeverity::Error
            } else if percent_worse > 10.0 || deviation_sigma > 2.0 {
                RegressionSeverity::Warning
            } else {
                continue; // Not significant enough to report
            };

            regressions.push(Regression {
                dimension,
                baseline_value: baseline.mean,
                current_value: current.mean,
                percent_worse,
                is_critical: percent_worse > 50.0,
                severity,
                deviation_sigma,
            });
        }
    }

    Ok(regressions)
}

/// Get top error tools from sessions
pub fn get_top_error_tools(db: &MetricsDb, period: TimePeriod, limit: usize) -> anyhow::Result<Vec<(String, u32)>> {
    let sessions = get_sessions_in_period(db, period)?;
    let mut error_counts: HashMap<String, u32> = HashMap::new();

    for session in &sessions {
        let tool_calls = db.get_tool_calls(&session.id)?;
        for tc in tool_calls {
            if !tc.success {
                *error_counts.entry(tc.tool_name).or_insert(0) += 1;
            }
        }
    }

    let mut errors: Vec<(String, u32)> = error_counts.into_iter().collect();
    errors.sort_by(|a, b| b.1.cmp(&a.1));
    errors.truncate(limit);

    Ok(errors)
}

/// Get slowest tools by average duration
pub fn get_slowest_tools(db: &MetricsDb, period: TimePeriod, limit: usize) -> anyhow::Result<Vec<(String, f64, usize)>> {
    let sessions = get_sessions_in_period(db, period)?;
    let mut tool_durations: HashMap<String, Vec<i64>> = HashMap::new();

    for session in &sessions {
        let tool_calls = db.get_tool_calls(&session.id)?;
        for tc in tool_calls {
            tool_durations
                .entry(tc.tool_name)
                .or_default()
                .push(tc.duration_ms);
        }
    }

    let mut slowest: Vec<(String, f64, usize)> = tool_durations
        .into_iter()
        .map(|(tool, durations)| {
            let count = durations.len();
            let avg = durations.iter().sum::<i64>() as f64 / count as f64;
            (tool, avg, count)
        })
        .collect();

    slowest.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    slowest.truncate(limit);

    Ok(slowest)
}

/// Calculate improvement velocity from trends
pub fn calculate_velocity(
    db: &MetricsDb,
    period: TimePeriod,
) -> anyhow::Result<crate::metrics::VelocitySnapshot> {
    use crate::metrics::{MetricVelocity, VelocitySnapshot};
    use chrono::Utc;

    // Get trends comparing this period vs previous period
    let baseline_period = match period {
        TimePeriod::Last7Days | TimePeriod::ThisWeek => Some(TimePeriod::LastWeek),
        TimePeriod::Last30Days => Some(TimePeriod::Last30Days), // Compare vs same period
        _ => None,
    };

    let trends = detect_trends(db, period, baseline_period)?;

    let mut improving_count = 0;
    let mut degrading_count = 0;
    let mut stable_count = 0;
    let mut key_metrics = Vec::new();

    // Key metrics to track for velocity
    let key_metric_names = ["tool_error_rate",
        "completion_rate",
        "cost_per_issue",
        "duration_per_issue"];

    for trend in trends {
        // Only include key metrics in the snapshot
        if key_metric_names.contains(&trend.dimension.as_str()) {
            let direction_str = match trend.direction {
                TrendDirection::Improving => {
                    improving_count += 1;
                    "improving"
                }
                TrendDirection::Degrading => {
                    degrading_count += 1;
                    "degrading"
                }
                TrendDirection::Stable => {
                    stable_count += 1;
                    "stable"
                }
            };

            key_metrics.push(MetricVelocity {
                dimension: trend.dimension.clone(),
                percent_change: trend.percent_change,
                direction: direction_str.to_string(),
            });
        } else {
            // Still count for the totals
            match trend.direction {
                TrendDirection::Improving => improving_count += 1,
                TrendDirection::Degrading => degrading_count += 1,
                TrendDirection::Stable => stable_count += 1,
            }
        }
    }

    // Calculate velocity score (-1.0 to 1.0)
    // Weighted: improving metrics add positive, degrading subtract
    let total_metrics = (improving_count + degrading_count + stable_count) as f64;
    let velocity_score = if total_metrics > 0.0 {
        (improving_count as f64 - degrading_count as f64) / total_metrics
    } else {
        0.0
    };

    // Count total issues completed in period
    let sessions = get_sessions_in_period(db, period)?;
    let issues_completed: i32 = sessions.iter().map(|s| s.issues_completed).sum();

    Ok(VelocitySnapshot {
        timestamp: Utc::now(),
        period: period.name().to_string(),
        velocity_score,
        improving_metrics: improving_count,
        degrading_metrics: degrading_count,
        stable_metrics: stable_count,
        issues_completed,
        key_metrics,
    })
}
