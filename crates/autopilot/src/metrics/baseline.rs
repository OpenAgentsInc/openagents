//! Baseline metrics tracking and regression detection
//!
//! This module implements automated baseline calculation and comparison
//! for autopilot metrics. It enables tracking of performance over time
//! and automatic detection of regressions.
//!
//! # CLI Commands
//!
//! ```bash
//! # Update baselines from recent sessions (last 100 by default)
//! cargo autopilot metrics baseline update
//!
//! # Update with custom session count
//! cargo autopilot metrics baseline update --sessions 200
//!
//! # Show current baselines
//! cargo autopilot metrics baseline show
//!
//! # Show as JSON
//! cargo autopilot metrics baseline show --format json
//!
//! # Check for regressions
//! cargo autopilot metrics baseline check
//!
//! # Generate baseline report
//! cargo autopilot metrics baseline report
//! ```
//!
//! # Automated Updates
//!
//! Baselines can be updated automatically:
//! - Via CLI: `cargo autopilot metrics baseline update`
//! - Via API: `BaselineCalculator::update_all_baselines()`
//! - Schedule weekly updates with cron or systemd timer
//!
//! # Baseline History
//!
//! Baselines are versioned by timestamp in the `updated_at` field.
//! Historical tracking can detect performance trends over time.
//! The database maintains one current baseline per dimension.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Baseline, MetricsDb, SessionMetrics};

/// Metric dimension types for baseline tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricDimension {
    /// Tool error rate (errors / total calls)
    ToolErrorRate,
    /// Task completion rate (completed / claimed)
    CompletionRate,
    /// Average duration per session (seconds)
    AvgDuration,
    /// Average cost per session (USD)
    AvgCost,
    /// Average tokens per session
    AvgTokens,
    /// Cache hit rate (cached / total input)
    CacheHitRate,
}

impl MetricDimension {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricDimension::ToolErrorRate => "tool_error_rate",
            MetricDimension::CompletionRate => "completion_rate",
            MetricDimension::AvgDuration => "avg_duration",
            MetricDimension::AvgCost => "avg_cost",
            MetricDimension::AvgTokens => "avg_tokens",
            MetricDimension::CacheHitRate => "cache_hit_rate",
        }
    }

    pub fn all() -> Vec<MetricDimension> {
        vec![
            MetricDimension::ToolErrorRate,
            MetricDimension::CompletionRate,
            MetricDimension::AvgDuration,
            MetricDimension::AvgCost,
            MetricDimension::AvgTokens,
            MetricDimension::CacheHitRate,
        ]
    }
}

/// Baseline calculator
pub struct BaselineCalculator<'a> {
    db: &'a MetricsDb,
}

impl<'a> BaselineCalculator<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Calculate baselines for all dimensions from recent sessions
    pub fn calculate_baselines(&self, sessions: &[SessionMetrics]) -> Result<HashMap<String, Baseline>> {
        let mut baselines = HashMap::new();

        for dimension in MetricDimension::all() {
            let values = self.extract_values(dimension, sessions);
            if values.is_empty() {
                continue;
            }

            let baseline = self.compute_baseline(dimension.as_str(), &values)?;
            baselines.insert(dimension.as_str().to_string(), baseline);
        }

        Ok(baselines)
    }

    /// Extract metric values for a specific dimension
    fn extract_values(&self, dimension: MetricDimension, sessions: &[SessionMetrics]) -> Vec<f64> {
        sessions
            .iter()
            .filter_map(|s| match dimension {
                MetricDimension::ToolErrorRate => {
                    if s.tool_calls > 0 {
                        Some(s.tool_errors as f64 / s.tool_calls as f64)
                    } else {
                        None
                    }
                }
                MetricDimension::CompletionRate => {
                    if s.issues_claimed > 0 {
                        Some(s.issues_completed as f64 / s.issues_claimed as f64)
                    } else {
                        None
                    }
                }
                MetricDimension::AvgDuration => Some(s.duration_seconds),
                MetricDimension::AvgCost => Some(s.cost_usd),
                MetricDimension::AvgTokens => Some((s.tokens_in + s.tokens_out) as f64),
                MetricDimension::CacheHitRate => {
                    let total_input = s.tokens_in + s.tokens_cached;
                    if total_input > 0 {
                        Some(s.tokens_cached as f64 / total_input as f64)
                    } else {
                        None
                    }
                }
            })
            .collect()
    }

    /// Compute baseline statistics from values
    fn compute_baseline(&self, dimension: &str, values: &[f64]) -> Result<Baseline> {
        if values.is_empty() {
            anyhow::bail!("No values to compute baseline");
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean = values.iter().sum::<f64>() / values.len() as f64;

        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;
        let stddev = variance.sqrt();

        let p50 = percentile(&sorted, 0.50);
        let p90 = percentile(&sorted, 0.90);
        let p99 = percentile(&sorted, 0.99);

        Ok(Baseline {
            dimension: dimension.to_string(),
            mean,
            stddev,
            p50,
            p90,
            p99,
            sample_count: values.len() as i32,
            updated_at: Utc::now(),
        })
    }

    /// Update all baselines in the database
    pub fn update_all_baselines(&self) -> Result<usize> {
        // Get last 100 sessions for baseline calculation
        let sessions = self.db.get_recent_sessions(100)?;

        if sessions.is_empty() {
            return Ok(0);
        }

        let baselines = self.calculate_baselines(&sessions)?;

        for baseline in baselines.values() {
            self.db.store_baseline(baseline)?;
        }

        Ok(baselines.len())
    }
}

/// Baseline comparator for regression detection
pub struct BaselineComparator<'a> {
    db: &'a MetricsDb,
}

impl<'a> BaselineComparator<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Compare current metrics against baselines and detect regressions
    pub fn detect_regressions(&self, sessions: &[SessionMetrics]) -> Result<Vec<Regression>> {
        let mut regressions = Vec::new();

        let calculator = BaselineCalculator::new(self.db);
        let current_baselines = calculator.calculate_baselines(sessions)?;

        for dimension in MetricDimension::all() {
            let dim_str = dimension.as_str();

            // Get stored baseline
            let stored_baseline = match self.db.get_baseline(dim_str)? {
                Some(b) => b,
                None => continue, // No baseline to compare against
            };

            // Get current baseline
            let current_baseline = match current_baselines.get(dim_str) {
                Some(b) => b,
                None => continue,
            };

            // Check for regression (metric is worse than baseline)
            let regression_detected = match dimension {
                // Lower is better
                MetricDimension::ToolErrorRate | MetricDimension::AvgDuration | MetricDimension::AvgCost => {
                    current_baseline.mean > stored_baseline.mean * 1.10 // >10% worse
                }
                // Higher is better
                MetricDimension::CompletionRate | MetricDimension::CacheHitRate => {
                    current_baseline.mean < stored_baseline.mean * 0.90 // <10% of baseline
                }
                // Stable is better
                MetricDimension::AvgTokens => {
                    (current_baseline.mean - stored_baseline.mean).abs() > stored_baseline.mean * 0.20 // >20% change
                }
            };

            if regression_detected {
                let percent_change = ((current_baseline.mean - stored_baseline.mean) / stored_baseline.mean) * 100.0;

                regressions.push(Regression {
                    dimension: dimension,
                    baseline_value: stored_baseline.mean,
                    current_value: current_baseline.mean,
                    percent_change,
                    severity: if percent_change.abs() > 50.0 {
                        RegressionSeverity::Critical
                    } else if percent_change.abs() > 25.0 {
                        RegressionSeverity::High
                    } else {
                        RegressionSeverity::Medium
                    },
                });
            }
        }

        Ok(regressions)
    }
}

/// Detected regression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regression {
    pub dimension: MetricDimension,
    pub baseline_value: f64,
    pub current_value: f64,
    pub percent_change: f64,
    pub severity: RegressionSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionSeverity {
    Medium,
    High,
    Critical,
}

/// Calculate percentile from sorted values
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Weekly baseline report generator
pub struct BaselineReportGenerator<'a> {
    db: &'a MetricsDb,
}

impl<'a> BaselineReportGenerator<'a> {
    pub fn new(db: &'a MetricsDb) -> Self {
        Self { db }
    }

    /// Generate a markdown report of current baselines
    pub fn generate_report(&self) -> Result<String> {
        let mut report = String::new();

        report.push_str("# Autopilot Baseline Metrics\n\n");
        report.push_str(&format!("Generated: {}\n\n", Utc::now().format("%Y-%m-%d %H:%M UTC")));

        report.push_str("## Current Baselines\n\n");
        report.push_str("| Dimension | Mean | StdDev | p50 | p90 | p99 | Samples |\n");
        report.push_str("|-----------|------|--------|-----|-----|-----|--------|\n");

        for dimension in MetricDimension::all() {
            if let Some(baseline) = self.db.get_baseline(dimension.as_str())? {
                report.push_str(&format!(
                    "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {} |\n",
                    dimension.as_str(),
                    baseline.mean,
                    baseline.stddev,
                    baseline.p50,
                    baseline.p90,
                    baseline.p99,
                    baseline.sample_count
                ));
            }
        }

        report.push_str("\n## Interpretation\n\n");
        report.push_str("- **tool_error_rate**: Lower is better (target: <0.05 or 5%)\n");
        report.push_str("- **completion_rate**: Higher is better (target: >0.95 or 95%)\n");
        report.push_str("- **avg_duration**: Depends on task complexity\n");
        report.push_str("- **avg_cost**: Lower is better (efficiency)\n");
        report.push_str("- **avg_tokens**: Stable is better (consistency)\n");
        report.push_str("- **cache_hit_rate**: Higher is better (target: >0.90 or 90%)\n");

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::SessionStatus;

    #[test]
    fn test_baseline_calculation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_baseline.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Create test sessions
        let mut sessions = Vec::new();
        for i in 0..20 {
            let session = SessionMetrics {
                id: format!("session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0 + (i as f64 * 5.0),
                tokens_in: 1000,
                tokens_out: 500,
                tokens_cached: 200,
                cost_usd: 0.05,
                issues_claimed: 2,
                issues_completed: 2,
                tool_calls: 20,
                tool_errors: i % 5, // Varying error rates
                final_status: SessionStatus::Completed,
            messages: 10,
            apm: None,
            source: "autopilot".to_string(),
                issue_numbers: None,
                directive_id: None,
        };
            sessions.push(session);
        }

        let calculator = BaselineCalculator::new(&db);
        let baselines = calculator.calculate_baselines(&sessions).unwrap();

        assert!(baselines.contains_key("tool_error_rate"));
        assert!(baselines.contains_key("completion_rate"));
        assert_eq!(baselines.get("completion_rate").unwrap().mean, 1.0); // All completed
    }

    #[test]
    fn test_regression_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_regression.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Create baseline
        let baseline = Baseline {
            dimension: "tool_error_rate".to_string(),
            mean: 0.05, // 5% error rate
            stddev: 0.02,
            p50: 0.04,
            p90: 0.08,
            p99: 0.10,
            sample_count: 100,
            updated_at: Utc::now(),
        };
        db.store_baseline(&baseline).unwrap();

        // Create sessions with high error rate (regression)
        let mut sessions = Vec::new();
        for i in 0..10 {
            let session = SessionMetrics {
                id: format!("session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0,
                tokens_in: 1000,
                tokens_out: 500,
                tokens_cached: 200,
                cost_usd: 0.05,
                issues_claimed: 2,
                issues_completed: 2,
                tool_calls: 20,
                tool_errors: 3, // 15% error rate (3x baseline)
                final_status: SessionStatus::Completed,
            messages: 10,
            apm: None,
            source: "autopilot".to_string(),
                issue_numbers: None,
                directive_id: None,
        };
            sessions.push(session);
        }

        let comparator = BaselineComparator::new(&db);
        let regressions = comparator.detect_regressions(&sessions).unwrap();

        assert!(!regressions.is_empty());
        assert_eq!(regressions[0].dimension, MetricDimension::ToolErrorRate);
        assert!(regressions[0].percent_change > 0.0); // Positive = worse
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        // p50: 0.5 * (10-1) = 4.5, rounds to 5, values[5] = 6.0
        assert_eq!(percentile(&values, 0.50), 6.0);
        // p90: 0.9 * (10-1) = 8.1, rounds to 8, values[8] = 9.0
        assert_eq!(percentile(&values, 0.90), 9.0);
        // p99: 0.99 * (10-1) = 8.91, rounds to 9, values[9] = 10.0
        assert_eq!(percentile(&values, 0.99), 10.0);
    }

    #[test]
    fn test_report_generation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_report.db");
        let db = MetricsDb::open(&db_path).unwrap();

        // Store a baseline
        let baseline = Baseline {
            dimension: "tool_error_rate".to_string(),
            mean: 0.05,
            stddev: 0.02,
            p50: 0.04,
            p90: 0.08,
            p99: 0.10,
            sample_count: 100,
            updated_at: Utc::now(),
        };
        db.store_baseline(&baseline).unwrap();

        let generator = BaselineReportGenerator::new(&db);
        let report = generator.generate_report().unwrap();

        assert!(report.contains("# Autopilot Baseline Metrics"));
        assert!(report.contains("tool_error_rate"));
        assert!(report.contains("0.0500"));
    }
}
