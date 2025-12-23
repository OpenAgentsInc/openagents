//! Automated weekly trend report generation
//!
//! Generates weekly summary reports showing metric trends and improvements.
//! Reports compare the current week to the previous week for all key metrics.

use chrono::{Datelike, Utc};
use std::path::{Path, PathBuf};

use crate::analyze::{detect_trends, detect_regressions, TimePeriod};
use crate::metrics::MetricsDb;

/// Generate a weekly trend report
///
/// Compares current week to previous week for all metrics.
/// Saves report to docs/autopilot/reports/YYYY-WW.md
///
/// # Arguments
/// * `db` - Metrics database
/// * `output_dir` - Base directory for reports (defaults to docs/autopilot/reports)
///
/// # Returns
/// Path to the generated report file
pub fn generate_weekly_report(
    db: &MetricsDb,
    output_dir: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    let now = Utc::now();
    let iso_week = now.iso_week();
    let year = iso_week.year();
    let week = iso_week.week();

    // Determine output path
    let base_dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/autopilot/reports"));

    std::fs::create_dir_all(&base_dir)?;

    let report_path = base_dir.join(format!("{}-W{:02}.md", year, week));

    // Calculate trends comparing this week to last week
    let trends = detect_trends(db, TimePeriod::ThisWeek, Some(TimePeriod::LastWeek))?;

    // Detect regressions for this week
    let regressions = detect_regressions(db, TimePeriod::ThisWeek)?;

    // Build report content
    let mut report = String::new();

    // Header
    report.push_str(&format!(
        "# Autopilot Weekly Report - {} Week {}\n\n",
        year, week
    ));
    report.push_str(&format!("Generated: {}\n\n", now.format("%Y-%m-%d %H:%M:%S UTC")));

    // Executive Summary
    report.push_str("## Executive Summary\n\n");

    // Calculate trend counts for use later
    let degrading = trends
        .iter()
        .filter(|t| matches!(t.direction, crate::analyze::TrendDirection::Degrading))
        .count();

    if trends.is_empty() {
        report.push_str("No metrics data available for this period.\n\n");
    } else {
        let improving = trends
            .iter()
            .filter(|t| matches!(t.direction, crate::analyze::TrendDirection::Improving))
            .count();
        let stable = trends
            .iter()
            .filter(|t| matches!(t.direction, crate::analyze::TrendDirection::Stable))
            .count();

        report.push_str(&format!("- {} metrics improving\n", improving));
        report.push_str(&format!("- {} metrics stable\n", stable));
        report.push_str(&format!("- {} metrics degrading\n\n", degrading));

        if !regressions.is_empty() {
            report.push_str(&format!(
                "⚠️  **{}  regressions detected** - immediate attention recommended\n\n",
                regressions.len()
            ));
        }
    }

    // Metric Trends
    report.push_str("## Metric Trends\n\n");
    report.push_str("Comparison: This week vs. Last week\n\n");

    if !trends.is_empty() {
        report.push_str("| Metric | Direction | Change | This Week | Last Week |\n");
        report.push_str("|--------|-----------|--------|-----------|------------|\n");

        for trend in &trends {
            let direction_icon = match trend.direction {
                crate::analyze::TrendDirection::Improving => "✅",
                crate::analyze::TrendDirection::Stable => "➖",
                crate::analyze::TrendDirection::Degrading => "⚠️",
            };

            let change_str = if trend.percent_change.abs() < 0.1 {
                "~0%".to_string()
            } else {
                format!("{:+.1}%", trend.percent_change)
            };

            report.push_str(&format!(
                "| {} | {} | {} | {:.2} | {:.2} |\n",
                trend.dimension,
                direction_icon,
                change_str,
                trend.recent.mean,
                trend.baseline.as_ref().map(|b| b.mean).unwrap_or(0.0)
            ));
        }
        report.push_str("\n");
    } else {
        report.push_str("_No trend data available_\n\n");
    }

    // Regressions
    if !regressions.is_empty() {
        report.push_str("## ⚠️ Regressions Detected\n\n");
        report.push_str("The following metrics have regressed significantly:\n\n");

        for regression in &regressions {
            let severity_label = match regression.severity {
                crate::analyze::RegressionSeverity::Critical => "🔴 CRITICAL",
                crate::analyze::RegressionSeverity::Error => "🟠 ERROR",
                crate::analyze::RegressionSeverity::Warning => "🟡 WARNING",
            };

            report.push_str(&format!("### {} - {}\n\n", severity_label, regression.dimension));
            report.push_str(&format!("- **Baseline**: {:.2}\n", regression.baseline_value));
            report.push_str(&format!("- **Current**: {:.2}\n", regression.current_value));
            report.push_str(&format!(
                "- **Degradation**: {:.1}% worse\n",
                regression.percent_worse
            ));
            report.push_str(&format!(
                "- **Statistical significance**: {:.1}σ\n\n",
                regression.deviation_sigma
            ));
        }
    }

    // Best/Worst Sessions (if we have session data)
    // This requires querying recent sessions - we'll add this in a follow-up
    report.push_str("## Session Highlights\n\n");
    report.push_str("_Session analysis coming soon_\n\n");

    // Recommendations
    report.push_str("## Recommendations\n\n");

    if regressions.is_empty() && degrading == 0 {
        report.push_str("✅ All metrics stable or improving. Continue current practices.\n\n");
    } else {
        report.push_str("Based on this week's data:\n\n");

        for regression in &regressions {
            match regression.dimension.as_str() {
                "tool_error_rate" => {
                    report.push_str("- **Tool Error Rate**: Investigate failing tools. Check recent error logs and consider updating tool implementations.\n");
                }
                "cost_per_issue" => {
                    report.push_str("- **Cost Per Issue**: Review prompt efficiency and consider using smaller models for simpler tasks.\n");
                }
                "duration_per_issue" => {
                    report.push_str("- **Duration Per Issue**: Analyze slow sessions to identify bottlenecks. Consider parallel execution optimizations.\n");
                }
                "completion_rate" => {
                    report.push_str("- **Completion Rate**: Review failed sessions to understand blocking issues. Improve error recovery strategies.\n");
                }
                _ => {
                    report.push_str(&format!("- **{}**: Investigate cause of regression.\n", regression.dimension));
                }
            }
        }
        report.push_str("\n");
    }

    // Footer
    report.push_str("---\n\n");
    report.push_str(&format!(
        "_This report was automatically generated by `openagents autopilot metrics report` at {}_\n",
        now.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Write report to file
    std::fs::write(&report_path, report)?;

    Ok(report_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsDb;
    use tempfile::TempDir;

    #[test]
    fn test_generate_weekly_report_empty_db() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = MetricsDb::open(&db_path).unwrap();

        let output_dir = temp_dir.path().join("reports");
        let report_path = generate_weekly_report(&db, Some(&output_dir)).unwrap();

        assert!(report_path.exists());
        let content = std::fs::read_to_string(&report_path).unwrap();
        assert!(content.contains("Autopilot Weekly Report"));
        assert!(content.contains("No metrics data available"));
    }

    #[test]
    fn test_report_path_format() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = MetricsDb::open(&db_path).unwrap();

        let output_dir = temp_dir.path().join("reports");
        let report_path = generate_weekly_report(&db, Some(&output_dir)).unwrap();

        let filename = report_path.file_name().unwrap().to_str().unwrap();
        // Should match pattern YYYY-WWW.md (e.g., 2025-W51.md)
        assert!(filename.contains("-W"));
        assert!(filename.ends_with(".md"));
    }
}
