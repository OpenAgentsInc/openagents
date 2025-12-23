//! Automated weekly trend reports for autopilot metrics
//!
//! This module implements automated weekly report generation for Phase 3 of d-004.
//! Reports include week-over-week comparisons, regression detection, and key metrics.
//!
//! # Usage
//!
//! ```no_run
//! use autopilot::reports::generate_weekly_report;
//! use autopilot::metrics::MetricsDb;
//!
//! let db = MetricsDb::open("autopilot-metrics.db")?;
//! let report = generate_weekly_report(&db)?;
//!
//! // Save to file
//! std::fs::write("docs/reports/weekly-20251223.md", report)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::Result;
use chrono::{DateTime, Utc};
use colored::*;
use std::path::PathBuf;

use crate::analyze::{
    calculate_aggregate_stats_from_sessions, detect_regressions, get_sessions_in_period,
    get_top_error_tools, Regression, RegressionSeverity, TimePeriod,
};
use crate::metrics::MetricsDb;

/// Weekly report containing all key metrics and trends
#[derive(Debug)]
pub struct WeeklyReport {
    pub week_start: DateTime<Utc>,
    pub week_end: DateTime<Utc>,
    pub current_week_stats: std::collections::HashMap<String, f64>,
    pub previous_week_stats: std::collections::HashMap<String, f64>,
    pub regressions: Vec<Regression>,
    pub top_errors: Vec<(String, u32)>,
    pub total_sessions: usize,
    pub total_issues_completed: i32,
    pub total_cost_usd: f64,
    pub sessions_comparison: SessionComparison,
    pub personal_bests_achieved: Vec<crate::metrics::PersonalBest>,
}

/// Week-over-week session comparison
#[derive(Debug)]
pub struct SessionComparison {
    pub current_week_sessions: usize,
    pub previous_week_sessions: usize,
    pub current_week_issues: i32,
    pub previous_week_issues: i32,
    pub current_week_cost: f64,
    pub previous_week_cost: f64,
}

/// Generate a comprehensive weekly report
pub fn generate_weekly_report(db: &MetricsDb) -> Result<WeeklyReport> {
    let current_week_period = TimePeriod::ThisWeek;
    let previous_week_period = TimePeriod::LastWeek;

    // Get sessions for both weeks
    let current_sessions = get_sessions_in_period(db, current_week_period)?;
    let previous_sessions = get_sessions_in_period(db, previous_week_period)?;

    // Calculate aggregate stats
    let current_stats = calculate_aggregate_stats_from_sessions(&current_sessions);
    let previous_stats = calculate_aggregate_stats_from_sessions(&previous_sessions);

    // Extract mean values for comparison
    let current_week_stats: std::collections::HashMap<String, f64> = current_stats
        .iter()
        .map(|(k, v)| (k.clone(), v.mean))
        .collect();

    let previous_week_stats: std::collections::HashMap<String, f64> = previous_stats
        .iter()
        .map(|(k, v)| (k.clone(), v.mean))
        .collect();

    // Detect regressions
    let regressions = detect_regressions(db, current_week_period)?;

    // Get top error tools
    let top_errors = get_top_error_tools(db, current_week_period, 5)?;

    // Calculate totals
    let total_sessions = current_sessions.len();
    let total_issues_completed: i32 = current_sessions.iter().map(|s| s.issues_completed).sum();
    let total_cost_usd: f64 = current_sessions.iter().map(|s| s.cost_usd).sum();

    // Session comparison
    let sessions_comparison = SessionComparison {
        current_week_sessions: current_sessions.len(),
        previous_week_sessions: previous_sessions.len(),
        current_week_issues: current_sessions.iter().map(|s| s.issues_completed).sum(),
        previous_week_issues: previous_sessions.iter().map(|s| s.issues_completed).sum(),
        current_week_cost: current_sessions.iter().map(|s| s.cost_usd).sum(),
        previous_week_cost: previous_sessions.iter().map(|s| s.cost_usd).sum(),
    };

    // Get week boundaries
    let (week_start, week_end) = current_week_period.bounds();

    // Get personal bests achieved this week (handle case where table doesn't exist yet)
    let personal_bests_achieved: Vec<crate::metrics::PersonalBest> = db
        .get_all_personal_bests()
        .unwrap_or_else(|_| vec![])
        .into_iter()
        .filter(|best| best.timestamp >= week_start && best.timestamp <= week_end)
        .collect();

    Ok(WeeklyReport {
        week_start,
        week_end,
        current_week_stats,
        previous_week_stats,
        regressions,
        top_errors,
        total_sessions,
        total_issues_completed,
        total_cost_usd,
        sessions_comparison,
        personal_bests_achieved,
    })
}

/// Format weekly report as markdown
pub fn format_report_markdown(report: &WeeklyReport) -> String {
    let mut output = String::new();

    // Header
    output.push_str("# Weekly Autopilot Metrics Report\n\n");
    output.push_str(&format!(
        "**Week:** {} to {}\n\n",
        report.week_start.format("%Y-%m-%d"),
        report.week_end.format("%Y-%m-%d")
    ));
    output.push_str(&format!("**Generated:** {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    output.push_str("---\n\n");

    // Executive Summary
    output.push_str("## Executive Summary\n\n");
    output.push_str(&format!("- **Total Sessions:** {}\n", report.total_sessions));
    output.push_str(&format!("- **Issues Completed:** {}\n", report.total_issues_completed));
    output.push_str(&format!("- **Total Cost:** ${:.4}\n", report.total_cost_usd));

    if !report.regressions.is_empty() {
        let critical_count = report.regressions.iter()
            .filter(|r| matches!(r.severity, RegressionSeverity::Critical))
            .count();
        output.push_str(&format!("- **⚠️ Regressions Detected:** {} ({} critical)\n",
            report.regressions.len(), critical_count));
    } else {
        output.push_str("- **✅ No Regressions:** All metrics within acceptable ranges\n");
    }

    output.push_str("\n");

    // Personal Bests Achieved
    if !report.personal_bests_achieved.is_empty() {
        output.push_str("## 🏆 Personal Bests Achieved This Week\n\n");
        for best in &report.personal_bests_achieved {
            output.push_str(&format!("- **{}:** {:.2}", best.metric, best.value));
            if let Some(ref context) = best.context {
                output.push_str(&format!(" ({})", context));
            }
            output.push_str("\n");
        }
        output.push_str("\n");
    }

    // Week-over-Week Comparison
    output.push_str("## Week-over-Week Comparison\n\n");

    let comp = &report.sessions_comparison;
    let sessions_change = if comp.previous_week_sessions > 0 {
        ((comp.current_week_sessions as f64 - comp.previous_week_sessions as f64)
            / comp.previous_week_sessions as f64) * 100.0
    } else {
        0.0
    };

    let issues_change = if comp.previous_week_issues > 0 {
        ((comp.current_week_issues as f64 - comp.previous_week_issues as f64)
            / comp.previous_week_issues as f64) * 100.0
    } else {
        0.0
    };

    let cost_change = if comp.previous_week_cost > 0.0 {
        ((comp.current_week_cost - comp.previous_week_cost)
            / comp.previous_week_cost) * 100.0
    } else {
        0.0
    };

    output.push_str("| Metric | This Week | Last Week | Change |\n");
    output.push_str("|--------|-----------|-----------|--------|\n");
    output.push_str(&format!(
        "| Sessions | {} | {} | {:+.1}% |\n",
        comp.current_week_sessions, comp.previous_week_sessions, sessions_change
    ));
    output.push_str(&format!(
        "| Issues Completed | {} | {} | {:+.1}% |\n",
        comp.current_week_issues, comp.previous_week_issues, issues_change
    ));
    output.push_str(&format!(
        "| Total Cost | ${:.4} | ${:.4} | {:+.1}% |\n",
        comp.current_week_cost, comp.previous_week_cost, cost_change
    ));

    output.push_str("\n");

    // Key Metrics
    output.push_str("## Key Metrics\n\n");

    let metrics_to_report = vec![
        ("tool_error_rate", "Tool Error Rate", "%", 100.0, true),
        ("completion_rate", "Completion Rate", "%", 100.0, false),
        ("cost_per_issue", "Cost per Issue", "$", 1.0, true),
        ("duration_per_issue", "Duration per Issue", "s", 1.0, true),
        ("tokens_per_issue", "Tokens per Issue", "", 1.0, true),
    ];

    output.push_str("| Metric | This Week | Last Week | Change | Status |\n");
    output.push_str("|--------|-----------|-----------|--------|--------|\n");

    for (key, name, unit, multiplier, lower_is_better) in metrics_to_report {
        let current_val = report.current_week_stats.get(key).copied().unwrap_or(0.0) * multiplier;
        let previous_val = report.previous_week_stats.get(key).copied().unwrap_or(0.0) * multiplier;

        let change = if previous_val > 0.0 {
            ((current_val - previous_val) / previous_val) * 100.0
        } else {
            0.0
        };

        let status = if change.abs() < 5.0 {
            "→ Stable"
        } else if lower_is_better {
            if change < 0.0 { "✅ Improving" } else { "⚠️ Degrading" }
        } else {
            if change > 0.0 { "✅ Improving" } else { "⚠️ Degrading" }
        };

        let current_formatted = if unit == "$" {
            format!("{}{:.4}", unit, current_val)
        } else if unit == "%" {
            format!("{:.1}{}", current_val, unit)
        } else {
            format!("{:.1}{}", current_val, unit)
        };

        let previous_formatted = if unit == "$" {
            format!("{}{:.4}", unit, previous_val)
        } else if unit == "%" {
            format!("{:.1}{}", previous_val, unit)
        } else {
            format!("{:.1}{}", previous_val, unit)
        };

        output.push_str(&format!(
            "| {} | {} | {} | {:+.1}% | {} |\n",
            name, current_formatted, previous_formatted, change, status
        ));
    }

    output.push_str("\n");

    // Regression Alerts
    if !report.regressions.is_empty() {
        output.push_str("## ⚠️ Regression Alerts\n\n");

        for reg in &report.regressions {
            let severity_emoji = match reg.severity {
                RegressionSeverity::Critical => "🔴",
                RegressionSeverity::Error => "🟠",
                RegressionSeverity::Warning => "🟡",
            };

            output.push_str(&format!(
                "### {} {} - {:?}\n\n",
                severity_emoji,
                reg.dimension.replace("_", " ").to_uppercase(),
                reg.severity
            ));
            output.push_str(&format!("- **Baseline:** {:.4}\n", reg.baseline_value));
            output.push_str(&format!("- **Current:** {:.4}\n", reg.current_value));
            output.push_str(&format!("- **Degradation:** {:.1}%\n", reg.percent_worse));
            output.push_str(&format!("- **Deviation:** {:.2}σ\n", reg.deviation_sigma));
            output.push_str("\n");
        }
    }

    // Top Errors
    if !report.top_errors.is_empty() {
        output.push_str("## Top Error Tools (This Week)\n\n");
        output.push_str("| Tool | Error Count |\n");
        output.push_str("|------|-------------|\n");

        for (tool, count) in &report.top_errors {
            output.push_str(&format!("| {} | {} |\n", tool, count));
        }

        output.push_str("\n");
    }

    // APM Analysis (if available)
    if let Some(current_apm) = report.current_week_stats.get("apm") {
        if let Some(previous_apm) = report.previous_week_stats.get("apm") {
            output.push_str("## Actions Per Minute (APM) Trend\n\n");

            let apm_change = if *previous_apm > 0.0 {
                ((current_apm - previous_apm) / previous_apm) * 100.0
            } else {
                0.0
            };

            output.push_str(&format!("- **This Week:** {:.1} APM\n", current_apm));
            output.push_str(&format!("- **Last Week:** {:.1} APM\n", previous_apm));
            output.push_str(&format!("- **Change:** {:+.1}%\n\n", apm_change));

            if apm_change > 10.0 {
                output.push_str("✨ **Velocity increasing!** Autopilot is working faster.\n\n");
            } else if apm_change < -10.0 {
                output.push_str("⚠️ **Velocity decreasing.** Consider investigating performance bottlenecks.\n\n");
            }
        }
    }

    // Recommendations
    output.push_str("## Recommendations\n\n");

    if report.regressions.is_empty() && report.top_errors.is_empty() {
        output.push_str("✅ All metrics look good! Keep up the great work.\n\n");
    } else {
        if !report.regressions.is_empty() {
            output.push_str("1. **Address Regressions:** Investigate the metrics that have degraded and identify root causes.\n");
        }

        if !report.top_errors.is_empty() {
            output.push_str("2. **Reduce Tool Errors:** Focus on the top error-prone tools listed above.\n");
        }

        if let Some(error_rate) = report.current_week_stats.get("tool_error_rate") {
            if *error_rate > 0.10 {
                output.push_str("3. **High Error Rate:** Tool error rate exceeds 10%. Review error logs and improve error handling.\n");
            }
        }

        output.push_str("\n");
    }

    // Footer
    output.push_str("---\n\n");
    output.push_str("*This report was automatically generated by the autopilot metrics system (d-004).*\n");
    output.push_str("*Run `cargo autopilot metrics report` to generate a new report.*\n");

    output
}

/// Print weekly report to console with colors
pub fn print_report_console(report: &WeeklyReport) {
    println!("{}", "=".repeat(80).cyan());
    println!("{}", "WEEKLY AUTOPILOT METRICS REPORT".cyan().bold());
    println!("{}", "=".repeat(80).cyan());
    println!();

    println!("{} {} to {}",
        "Week:".bold(),
        report.week_start.format("%Y-%m-%d"),
        report.week_end.format("%Y-%m-%d")
    );
    println!("{} {}", "Generated:".bold(), Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
    println!();

    // Executive Summary
    println!("{}", "EXECUTIVE SUMMARY".yellow().bold());
    println!("  Sessions:          {}", report.total_sessions.to_string().cyan());
    println!("  Issues Completed:  {}", report.total_issues_completed.to_string().cyan());
    println!("  Total Cost:        {}", format!("${:.4}", report.total_cost_usd).cyan());

    if !report.regressions.is_empty() {
        let critical = report.regressions.iter()
            .filter(|r| matches!(r.severity, RegressionSeverity::Critical))
            .count();
        println!("  Regressions:       {} ({} critical)",
            report.regressions.len().to_string().red(),
            critical.to_string().red().bold()
        );
    } else {
        println!("  Regressions:       {}", "None".green());
    }
    println!();

    // Personal Bests
    if !report.personal_bests_achieved.is_empty() {
        println!("{}", "🏆 PERSONAL BESTS ACHIEVED THIS WEEK".yellow().bold());
        for best in &report.personal_bests_achieved {
            print!("  {}: {:.2}", best.metric.cyan().bold(), best.value.to_string().green().bold());
            if let Some(ref context) = best.context {
                print!(" ({})", context.dimmed());
            }
            println!();
        }
        println!();
    }

    // Week-over-Week
    println!("{}", "WEEK-OVER-WEEK COMPARISON".yellow().bold());

    let comp = &report.sessions_comparison;
    let sessions_change = if comp.previous_week_sessions > 0 {
        ((comp.current_week_sessions as f64 - comp.previous_week_sessions as f64)
            / comp.previous_week_sessions as f64) * 100.0
    } else {
        0.0
    };

    println!("  Sessions:   {} → {} ({:+.1}%)",
        comp.previous_week_sessions,
        comp.current_week_sessions,
        sessions_change
    );

    let issues_change = if comp.previous_week_issues > 0 {
        ((comp.current_week_issues as f64 - comp.previous_week_issues as f64)
            / comp.previous_week_issues as f64) * 100.0
    } else {
        0.0
    };

    println!("  Issues:     {} → {} ({:+.1}%)",
        comp.previous_week_issues,
        comp.current_week_issues,
        issues_change
    );

    let cost_change = if comp.previous_week_cost > 0.0 {
        ((comp.current_week_cost - comp.previous_week_cost) / comp.previous_week_cost) * 100.0
    } else {
        0.0
    };

    println!("  Cost:       ${:.4} → ${:.4} ({:+.1}%)",
        comp.previous_week_cost,
        comp.current_week_cost,
        cost_change
    );
    println!();

    // Regressions
    if !report.regressions.is_empty() {
        println!("{}", "REGRESSIONS DETECTED".red().bold());
        for reg in &report.regressions {
            let severity_str = match reg.severity {
                RegressionSeverity::Critical => "CRITICAL".red().bold(),
                RegressionSeverity::Error => "ERROR".red(),
                RegressionSeverity::Warning => "WARNING".yellow(),
            };

            println!("  {} {}: {:.4} → {:.4} ({:+.1}%)",
                severity_str,
                reg.dimension,
                reg.baseline_value,
                reg.current_value,
                reg.percent_worse
            );
        }
        println!();
    }

    // Top Errors
    if !report.top_errors.is_empty() {
        println!("{}", "TOP ERROR TOOLS".yellow().bold());
        for (tool, count) in &report.top_errors {
            println!("  {:20} {} errors", tool, count);
        }
        println!();
    }

    println!("{}", "=".repeat(80).cyan());
}

/// Save report to a file in docs/reports/
pub fn save_report_to_file(report: &WeeklyReport) -> Result<PathBuf> {
    use std::fs;
    use std::path::PathBuf;

    // Create reports directory if it doesn't exist
    let reports_dir = PathBuf::from("docs/reports");
    fs::create_dir_all(&reports_dir)?;

    // Generate filename: weekly-YYYYMMDD.md
    let filename = format!("weekly-{}.md", report.week_end.format("%Y%m%d"));
    let filepath = reports_dir.join(&filename);

    // Generate markdown content
    let content = format_report_markdown(report);

    // Write to file
    fs::write(&filepath, content)?;

    Ok(filepath)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_report_markdown() {
        use std::collections::HashMap;

        let mut current_stats = HashMap::new();
        current_stats.insert("tool_error_rate".to_string(), 0.05);
        current_stats.insert("completion_rate".to_string(), 0.95);

        let mut previous_stats = HashMap::new();
        previous_stats.insert("tool_error_rate".to_string(), 0.08);
        previous_stats.insert("completion_rate".to_string(), 0.90);

        let report = WeeklyReport {
            week_start: Utc::now() - chrono::Duration::days(7),
            week_end: Utc::now(),
            current_week_stats: current_stats,
            previous_week_stats: previous_stats,
            regressions: vec![],
            top_errors: vec![],
            total_sessions: 10,
            total_issues_completed: 25,
            total_cost_usd: 1.5,
            sessions_comparison: SessionComparison {
                current_week_sessions: 10,
                previous_week_sessions: 8,
                current_week_issues: 25,
                previous_week_issues: 20,
                current_week_cost: 1.5,
                previous_week_cost: 1.2,
            },
            personal_bests_achieved: vec![],
        };

        let markdown = format_report_markdown(&report);

        assert!(markdown.contains("# Weekly Autopilot Metrics Report"));
        assert!(markdown.contains("Executive Summary"));
        assert!(markdown.contains("Week-over-Week Comparison"));
        assert!(markdown.contains("Key Metrics"));
    }
}
