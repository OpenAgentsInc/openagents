//! Automated issue creation from anomaly detection
//!
//! This module implements Phase 4 of directive d-004: automatically creating
//! improvement issues when patterns of failures are detected in metrics analysis.
//!
//! When anomalies are detected across multiple sessions, this module:
//! - Groups similar anomalies into patterns
//! - Generates issue titles and descriptions with evidence
//! - Creates issues linked to d-004 directive
//! - Updates anomaly records with issue numbers for tracking
//!
//! # Issue Template Format
//!
//! All auto-generated improvement issues follow a consistent template:
//!
//! ```markdown
//! # Title Format
//! - Dimension-specific, descriptive title
//! - Includes occurrence count for context
//! - Example: "High tool error rate detected across 5 sessions"
//!
//! # Description Structure
//! ## Evidence
//! - Occurrence count
//! - Severity level (Critical/Error/Warning)
//! - Average deviation percentage
//! - Deviation range (min-max)
//!
//! ### Affected Sessions
//! - List of affected session IDs with metrics
//! - Expected vs actual values with percentage change
//! - Direct commands to view each session
//!
//! ### Trajectory Evidence
//! - Commands to find trajectory logs
//! - What to look for in trajectories
//! - Specific error patterns to identify
//!
//! ## Proposed Fix
//! - Severity indicators (⚠️ CRITICAL / HIGH IMPACT)
//! - Step-by-step fix recommendations
//! - SQL queries for investigation
//! - Specific guardrails or validations to add
//! - Target metrics post-fix
//!
//! ## Investigation Steps
//! - Commands to review metrics
//! - How to analyze tool errors
//! - Steps to verify fix effectiveness
//! ```
//!
//! # Priority Calculation
//!
//! Priority is calculated using a composite score:
//! - **Severity score** (1-3): Warning=1, Error=2, Critical=3
//! - **Frequency score** (0-2): <5 occurrences=0, 5-9=1, 10+=2
//! - **Deviation score** (0-2): <25%=0, 25-50%=1, >50%=2
//!
//! Total score maps to priority:
//! - 6-7 = urgent
//! - 4-5 = high
//! - 2-3 = medium
//! - 0-1 = low
//!
//! # Tracking Metadata
//!
//! Auto-created issues are marked with:
//! - `auto_created = true` in the database
//! - Linked to `directive_id = "d-004"`
//! - Anomaly records updated with `issue_number` for traceability

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

#[cfg(test)]
use chrono::Utc;

use crate::metrics::{Anomaly, AnomalySeverity, MetricsDb};
use crate::tool_patterns::{
    ToolErrorPattern, detect_tool_patterns, generate_tool_pattern_description,
    generate_tool_pattern_priority, generate_tool_pattern_title,
};

/// Pattern of related anomalies
#[derive(Debug, Clone)]
pub struct AnomalyPattern {
    /// Metric dimension (e.g., "tool_error_rate")
    pub dimension: String,
    /// Number of sessions with this anomaly
    pub occurrence_count: usize,
    /// Severity (worst seen)
    pub severity: AnomalySeverity,
    /// Average deviation from expected
    pub avg_deviation: f64,
    /// Related anomalies
    pub anomalies: Vec<Anomaly>,
}

/// Pattern type - either anomaly-based or tool-error-based
#[derive(Debug, Clone)]
pub enum Pattern {
    Anomaly(AnomalyPattern),
    ToolError(ToolErrorPattern),
}

/// Proposed improvement issue
#[derive(Debug, Clone)]
pub struct ImprovementIssue {
    /// Issue title
    pub title: String,
    /// Issue description (markdown)
    pub description: String,
    /// Priority (urgent, high, medium, low)
    pub priority: String,
    /// Related pattern
    pub pattern: Pattern,
}

/// Detect all patterns: both anomaly-based and tool-error-based
pub fn detect_all_patterns(db: &MetricsDb) -> Result<Vec<Pattern>> {
    let mut all_patterns = Vec::new();

    // Detect anomaly patterns
    let anomaly_patterns = detect_patterns(db)?;
    for pattern in anomaly_patterns {
        all_patterns.push(Pattern::Anomaly(pattern));
    }

    // Detect tool error patterns (min 10 calls to be significant)
    let tool_patterns = detect_tool_patterns(db, 10)?;
    for pattern in tool_patterns {
        all_patterns.push(Pattern::ToolError(pattern));
    }

    Ok(all_patterns)
}

/// Detect patterns from uninvestigated anomalies
pub fn detect_patterns(db: &MetricsDb) -> Result<Vec<AnomalyPattern>> {
    let anomalies = get_uninvestigated_anomalies(db)?;

    if anomalies.is_empty() {
        return Ok(Vec::new());
    }

    // Group by dimension
    let mut by_dimension: HashMap<String, Vec<Anomaly>> = HashMap::new();
    for anomaly in anomalies {
        by_dimension
            .entry(anomaly.dimension.clone())
            .or_default()
            .push(anomaly);
    }

    // Create patterns for dimensions with multiple occurrences
    let mut patterns = Vec::new();
    for (dimension, anomalies) in by_dimension {
        // Only create pattern if we have 2+ anomalies (indicates a real pattern, not a one-off)
        if anomalies.len() >= 2 {
            let severity = anomalies
                .iter()
                .map(|a| a.severity)
                .max_by_key(|s| match s {
                    AnomalySeverity::Critical => 3,
                    AnomalySeverity::Error => 2,
                    AnomalySeverity::Warning => 1,
                })
                .unwrap_or(AnomalySeverity::Warning);

            let avg_deviation = anomalies
                .iter()
                .map(|a| {
                    let deviation = ((a.actual_value - a.expected_value) / a.expected_value).abs();
                    deviation
                })
                .sum::<f64>()
                / anomalies.len() as f64;

            patterns.push(AnomalyPattern {
                dimension: dimension.clone(),
                occurrence_count: anomalies.len(),
                severity,
                avg_deviation,
                anomalies,
            });
        }
    }

    // Sort by severity and occurrence count
    patterns.sort_by(|a, b| {
        let severity_cmp = match (&b.severity, &a.severity) {
            (AnomalySeverity::Critical, AnomalySeverity::Critical) => std::cmp::Ordering::Equal,
            (AnomalySeverity::Critical, _) => std::cmp::Ordering::Greater,
            (_, AnomalySeverity::Critical) => std::cmp::Ordering::Less,
            (AnomalySeverity::Error, AnomalySeverity::Error) => std::cmp::Ordering::Equal,
            (AnomalySeverity::Error, _) => std::cmp::Ordering::Greater,
            (_, AnomalySeverity::Error) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Equal,
        };

        severity_cmp.then_with(|| b.occurrence_count.cmp(&a.occurrence_count))
    });

    Ok(patterns)
}

/// Get uninvestigated anomalies from the database
fn get_uninvestigated_anomalies(db: &MetricsDb) -> Result<Vec<Anomaly>> {
    let rows = db.get_uninvestigated_anomalies()?;

    let anomalies = rows
        .into_iter()
        .map(
            |(session_id, dimension, expected_value, actual_value, severity_str)| {
                let severity = match severity_str.as_str() {
                    "critical" => AnomalySeverity::Critical,
                    "error" => AnomalySeverity::Error,
                    _ => AnomalySeverity::Warning,
                };

                Anomaly {
                    session_id,
                    dimension,
                    expected_value,
                    actual_value,
                    severity,
                    investigated: false,
                    issue_number: None,
                }
            },
        )
        .collect();

    Ok(anomalies)
}

/// Generate improvement issues from patterns
pub fn generate_issues(patterns: Vec<Pattern>) -> Vec<ImprovementIssue> {
    patterns
        .into_iter()
        .map(|pattern| {
            let (title, description, priority) = match &pattern {
                Pattern::Anomaly(p) => generate_anomaly_issue_content(p),
                Pattern::ToolError(p) => (
                    generate_tool_pattern_title(p),
                    generate_tool_pattern_description(p),
                    generate_tool_pattern_priority(p),
                ),
            };
            ImprovementIssue {
                title,
                description,
                priority,
                pattern,
            }
        })
        .collect()
}

/// Generate issue title, description, and priority from an anomaly pattern
fn generate_anomaly_issue_content(pattern: &AnomalyPattern) -> (String, String, String) {
    let title = match pattern.dimension.as_str() {
        "tool_error_rate" => {
            format!(
                "High tool error rate detected across {} sessions",
                pattern.occurrence_count
            )
        }
        "tool_error_rate_zscore" => {
            format!(
                "Tool error rate spike: {} sessions with anomalous error rates",
                pattern.occurrence_count
            )
        }
        "tokens_per_issue" => {
            format!(
                "Token usage anomaly: {} sessions with unusual token consumption",
                pattern.occurrence_count
            )
        }
        "cost_per_issue" => {
            format!(
                "Cost efficiency issue: {} sessions with elevated costs",
                pattern.occurrence_count
            )
        }
        "completion_rate" => {
            format!(
                "Low task completion rate in {} sessions",
                pattern.occurrence_count
            )
        }
        "session_duration" => {
            format!(
                "Session duration anomaly in {} runs",
                pattern.occurrence_count
            )
        }
        _ => format!(
            "Performance anomaly in {}: {} occurrences",
            pattern.dimension, pattern.occurrence_count
        ),
    };

    let description = generate_description(pattern);

    // Enhanced priority calculation based on both severity AND frequency
    let priority = calculate_priority(pattern);

    (title, description, priority)
}

/// Calculate priority based on severity, frequency, and deviation magnitude
fn calculate_priority(pattern: &AnomalyPattern) -> String {
    // Base priority from severity
    let severity_score = match pattern.severity {
        AnomalySeverity::Critical => 3,
        AnomalySeverity::Error => 2,
        AnomalySeverity::Warning => 1,
    };

    // Frequency multiplier (more occurrences = higher priority)
    let frequency_score = if pattern.occurrence_count >= 10 {
        2 // Very frequent
    } else if pattern.occurrence_count >= 5 {
        1 // Frequent
    } else {
        0 // Occasional
    };

    // Deviation magnitude (larger deviation = higher priority)
    let deviation_score = if pattern.avg_deviation > 0.50 {
        2 // >50% deviation is severe
    } else if pattern.avg_deviation > 0.25 {
        1 // >25% deviation is significant
    } else {
        0 // <25% deviation
    };

    // Combined score: 0-7
    let total_score = severity_score + frequency_score + deviation_score;

    // Map to priority string
    match total_score {
        6..=7 => "urgent", // Critical + frequent + large deviation
        4..=5 => "high",   // Error level with high frequency or deviation
        2..=3 => "medium", // Warning or infrequent errors
        _ => "low",        // Edge cases
    }
    .to_string()
}

/// Generate detailed description with evidence and proposed fix
fn generate_description(pattern: &AnomalyPattern) -> String {
    let mut desc = String::new();

    // Summary
    let severity_str = match pattern.severity {
        AnomalySeverity::Critical => "critical",
        AnomalySeverity::Error => "error",
        AnomalySeverity::Warning => "warning",
    };
    desc.push_str(&format!(
        "Detected pattern of {} anomalies in **{}** across {} sessions.\n\n",
        severity_str, pattern.dimension, pattern.occurrence_count
    ));

    // Evidence
    desc.push_str("## Evidence\n\n");
    desc.push_str(&format!(
        "- **Occurrences**: {} sessions\n",
        pattern.occurrence_count
    ));
    desc.push_str(&format!("- **Severity**: {:?}\n", pattern.severity));
    desc.push_str(&format!(
        "- **Average deviation**: {:.1}%\n",
        pattern.avg_deviation * 100.0
    ));

    // Statistical confidence
    let (min_dev, max_dev) = pattern
        .anomalies
        .iter()
        .map(|a| ((a.actual_value - a.expected_value) / a.expected_value).abs())
        .fold((f64::MAX, f64::MIN), |(min, max), val| {
            (min.min(val), max.max(val))
        });
    desc.push_str(&format!(
        "- **Deviation range**: {:.1}% to {:.1}%\n\n",
        min_dev * 100.0,
        max_dev * 100.0
    ));

    // Sample anomalies with enhanced detail
    desc.push_str("### Affected Sessions\n\n");
    for (i, anomaly) in pattern.anomalies.iter().take(5).enumerate() {
        let pct_change =
            ((anomaly.actual_value - anomaly.expected_value) / anomaly.expected_value) * 100.0;
        desc.push_str(&format!(
            "{}. Session `{}...`\n   - Expected: {:.3}, Actual: {:.3} ({:+.1}%)\n",
            i + 1,
            &anomaly.session_id[..8.min(anomaly.session_id.len())],
            anomaly.expected_value,
            anomaly.actual_value,
            pct_change
        ));

        // Add context based on dimension
        match pattern.dimension.as_str() {
            "tool_error_rate" => {
                desc.push_str(&format!(
                    "   - View session: `cargo autopilot metrics show {}`\n",
                    anomaly.session_id
                ));
            }
            "tokens_per_issue" => {
                desc.push_str("   - Investigate token usage efficiency\n");
            }
            "cost_per_issue" => {
                desc.push_str("   - Review cost optimization opportunities\n");
            }
            _ => {}
        }
    }
    if pattern.anomalies.len() > 5 {
        desc.push_str(&format!(
            "\n...and {} more sessions\n",
            pattern.anomalies.len() - 5
        ));
    }
    desc.push_str("\n");

    // Trajectory log excerpts (if available)
    desc.push_str("### Trajectory Evidence\n\n");
    if !pattern.anomalies.is_empty() {
        let session_id_prefix =
            &pattern.anomalies[0].session_id[..8.min(pattern.anomalies[0].session_id.len())];
        desc.push_str(&format!(
            "Review trajectory logs for affected sessions in `docs/logs/` to identify specific:\n\
            - Tool calls that failed repeatedly\n\
            - Error messages and stack traces\n\
            - Patterns in agent reasoning before failures\n\
            - Context that might explain the anomaly\n\n\
            Example command:\n\
            ```bash\n\
            # Find trajectory for session\n\
            find docs/logs -name '*{}*.json' -o -name '*{}*.rlog'\n\
            ```\n\n",
            session_id_prefix, session_id_prefix
        ));
    } else {
        desc.push_str("No trajectory data available for this pattern.\n\n");
    }

    // Proposed fix
    desc.push_str("## Proposed Fix\n\n");
    desc.push_str(&generate_proposed_fix(pattern));

    // Investigation steps
    desc.push_str("\n## Investigation Steps\n\n");
    desc.push_str(&generate_investigation_steps(pattern));

    // Auto-generated footer
    desc.push_str("\n---\n\n");
    desc.push_str(
        "*This issue was automatically generated by autopilot metrics analysis (d-004).*\n",
    );
    desc.push_str(&format!(
        "*Pattern detected from {} sessions with {:.1}% average deviation.*\n",
        pattern.occurrence_count,
        pattern.avg_deviation * 100.0
    ));

    desc
}

/// Generate proposed fix based on pattern type
fn generate_proposed_fix(pattern: &AnomalyPattern) -> String {
    let severity_note = if pattern.severity == AnomalySeverity::Critical {
        "⚠️  **CRITICAL**: This issue is blocking efficient autopilot operation. Prioritize immediately.\n\n"
    } else if pattern.severity == AnomalySeverity::Error {
        "⚠️  **HIGH IMPACT**: This pattern significantly affects autopilot performance.\n\n"
    } else {
        ""
    };

    let mut fix = String::from(severity_note);

    match pattern.dimension.as_str() {
        "tool_error_rate" | "tool_error_rate_zscore" => {
            fix.push_str(&format!(
                "Investigate common tool errors (detected in {} sessions with {:.1}% avg deviation):\n\n\
                1. **Identify error types**:\n   \
                   ```sql\n   \
                   SELECT tool_name, error_type, COUNT(*) as count\n   \
                   FROM tool_calls tc\n   \
                   WHERE tc.session_id IN ({}) AND tc.success = 0\n   \
                   GROUP BY tool_name, error_type\n   \
                   ORDER BY count DESC;\n   \
                   ```\n\n\
                2. **Review specific failures**: Focus on tools with >10 errors\n\n\
                3. **Add targeted guardrails**:\n   \
                   - If EISDIR: Add directory detection before Read tool\n   \
                   - If ENOENT: Validate file existence before operations\n   \
                   - If permission errors: Check file permissions in hooks\n\n\
                4. **Update system prompts**: Add examples of correct tool usage\n\n\
                5. **Implement pre-call validation**: Block invalid tool calls before execution\n",
                pattern.occurrence_count,
                pattern.avg_deviation * 100.0,
                pattern.anomalies.iter().take(3)
                    .map(|a| format!("'{}'", a.session_id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        "tokens_per_issue" => {
            let avg_actual: f64 = pattern
                .anomalies
                .iter()
                .map(|a| a.actual_value)
                .sum::<f64>()
                / pattern.anomalies.len() as f64;
            let avg_expected: f64 = pattern
                .anomalies
                .iter()
                .map(|a| a.expected_value)
                .sum::<f64>()
                / pattern.anomalies.len() as f64;

            fix.push_str(&format!(
                "Analyze token consumption (average: {:.0} actual vs {:.0} expected):\n\n\
                1. **Compare token usage patterns**:\n   \
                   - Review thinking block sizes\n   \
                   - Check for redundant tool calls\n   \
                   - Identify excessive context repetition\n\n\
                2. **Optimize compaction**: Current avg deviation {:.1}%\n   \
                   - May need more aggressive summarization\n   \
                   - Review what context is being preserved\n\n\
                3. **Review tool efficiency**:\n   \
                   - Are agents reading files multiple times?\n   \
                   - Could parallel tool calls reduce turns?\n\n\
                4. **Model selection**: Consider if tasks could use haiku model\n\n\
                5. **Set token budgets**: Implement per-issue token limits\n",
                avg_actual,
                avg_expected,
                pattern.avg_deviation * 100.0
            ));
        }
        "cost_per_issue" => {
            let avg_actual: f64 = pattern
                .anomalies
                .iter()
                .map(|a| a.actual_value)
                .sum::<f64>()
                / pattern.anomalies.len() as f64;

            fix.push_str(&format!(
                "Optimize cost efficiency (average ${:.4} per issue, {:.1}% over baseline):\n\n\
                1. **Audit high-cost sessions**: Review top 5 affected sessions\n\n\
                2. **Cache optimization**:\n   \
                   - Check cache hit rates in affected sessions\n   \
                   - Low cache hits directly increase costs\n   \
                   - Target >80% cache hit rate\n\n\
                3. **Model selection tuning**:\n   \
                   - Identify tasks suitable for haiku (simpler, lower cost)\n   \
                   - Reserve sonnet/opus for complex tasks\n\n\
                4. **Reduce wasted tokens**:\n   \
                   - Minimize failed tool calls (each failure wastes tokens)\n   \
                   - Optimize thinking efficiency\n\n\
                5. **Cost budgets**: Set ${:.4} target per issue\n",
                avg_actual,
                pattern.avg_deviation * 100.0,
                avg_actual * 0.8 // Target 20% reduction
            ));
        }
        "completion_rate" => {
            fix.push_str(&format!(
                "Improve task completion (detected in {} sessions):\n\n\
                1. **Root cause analysis**:\n   \
                   - Review final_status of affected sessions\n   \
                   - Identify common failure patterns\n   \
                   - Check if issues are too complex\n\n\
                2. **Remove blockers**:\n   \
                   - Missing dependencies or files\n   \
                   - Unclear issue descriptions\n   \
                   - Insufficient context in system prompts\n\n\
                3. **Adjust resource limits**:\n   \
                   - Review if max_turns is too restrictive\n   \
                   - Check token budget limits\n   \
                   - Consider allowing more retries\n\n\
                4. **Better task decomposition**:\n   \
                   - Split complex issues into smaller tasks\n   \
                   - Improve issue templates\n\n\
                5. **Add fallback strategies**: When stuck, escalate or skip\n",
                pattern.occurrence_count
            ));
        }
        "session_duration" => {
            fix.push_str(&format!(
                "Investigate duration anomalies ({} sessions affected):\n\n\
                1. **Correlate with outcomes**:\n   \
                   - Do longer sessions succeed or fail?\n   \
                   - Identify if duration indicates stuck processes\n\n\
                2. **Identify slow operations**:\n   \
                   - Query tool_calls for high duration_ms\n   \
                   - Find bottleneck tools\n\n\
                3. **Check for infinite loops**:\n   \
                   - Review sessions with >30min duration\n   \
                   - Look for repeated failed attempts\n\n\
                4. **Optimize parallelization**:\n   \
                   - Are independent tool calls being serialized?\n   \
                   - Could batch operations reduce wall time?\n\n\
                5. **Add stall detection**: Timeout after {}min of no progress\n",
                pattern.occurrence_count,
                5 // Suggested timeout
            ));
        }
        _ => {
            fix.push_str(&format!(
                "Generic investigation ({} affected sessions):\n\n\
                1. **Deep dive into trajectories**:\n   \
                   ```bash\n   \
                   cargo autopilot metrics show <session-id>\n   \
                   ```\n\n\
                2. **Compare normal vs anomalous**:\n   \
                   - What differs in affected sessions?\n   \
                   - Is there a common trigger?\n\n\
                3. **Pattern recognition**:\n   \
                   - Use `grep` to find similarities in logs\n   \
                   - Look for repeated error messages\n\n\
                4. **Implement targeted fix**:\n   \
                   - Based on findings, add validation\n   \
                   - Update prompts or hooks\n\n\
                5. **Verify improvement**:\n   \
                   - Monitor metrics post-fix\n   \
                   - Update baselines if successful\n",
                pattern.occurrence_count
            ));
        }
    }

    fix
}

/// Generate investigation steps
fn generate_investigation_steps(pattern: &AnomalyPattern) -> String {
    let mut steps = String::new();

    steps.push_str("1. Review detailed metrics for affected sessions:\n   ```bash\n");
    for (i, anomaly) in pattern.anomalies.iter().take(3).enumerate() {
        steps.push_str(&format!(
            "   cargo autopilot metrics show {}  # Session {}\n",
            anomaly.session_id,
            i + 1
        ));
    }
    steps.push_str("   ```\n\n");

    steps.push_str(
        "2. Query tool error breakdown:\n   ```bash\n   \
         cargo autopilot metrics analyze --period 7d\n   \
         ```\n\n",
    );

    steps.push_str(
        "3. Review trajectory logs for common patterns:\n   ```bash\n   \
         grep -r \"error\" docs/logs/*/\n   \
         ```\n\n",
    );

    steps.push_str(
        "4. After implementing fix:\n   \
         - Mark this issue as complete\n   \
         - Update baselines: `cargo autopilot metrics analyze --period 7d`\n   \
         - Monitor next week's metrics to confirm improvement\n",
    );

    steps
}

/// Parse priority string to Priority enum
fn parse_priority(priority_str: &str) -> issues::issue::Priority {
    match priority_str.to_lowercase().as_str() {
        "urgent" => issues::issue::Priority::Urgent,
        "high" => issues::issue::Priority::High,
        "low" => issues::issue::Priority::Low,
        _ => issues::issue::Priority::Medium,
    }
}

/// Create issues in the database
pub fn create_issues(
    issues_db_path: &Path,
    improvement_issues: &[ImprovementIssue],
    metrics_db: &MetricsDb,
) -> Result<Vec<i32>> {
    let conn = Connection::open(issues_db_path).context("Failed to open issues database")?;

    let mut created_issue_numbers = Vec::new();

    for issue in improvement_issues {
        // Use the proper issues API to create issues with auto_created flag
        // This ensures:
        // 1. Proper UUID generation for id column
        // 2. Correct use of next_issue_number() which uses the trigger
        // 3. All required columns are populated
        // 4. No double-incrementing of the counter
        // 5. auto_created flag is set to track automated detection
        let created_issue = issues::issue::create_issue_with_auto(
            &conn,
            &issue.title,
            Some(&issue.description),
            parse_priority(&issue.priority),
            issues::issue::IssueType::Task,
            Some("claude"),
            Some("d-004"),
            None, // No project_id for autopilot-generated issues
            true, // Mark as auto-created by anomaly detection
        )?;

        created_issue_numbers.push(created_issue.number);

        // Mark anomalies as investigated and link to issue (only for anomaly patterns)
        match &issue.pattern {
            Pattern::Anomaly(p) => {
                mark_anomalies_with_issue(metrics_db, &p.anomalies, created_issue.number)?;
            }
            Pattern::ToolError(_) => {
                // Tool error patterns don't have anomalies to mark
            }
        }

        println!(
            "  ✓ Created issue #{}: {} [{}]",
            created_issue.number, issue.title, issue.priority
        );
    }

    Ok(created_issue_numbers)
}

/// Mark anomalies as having an associated issue
fn mark_anomalies_with_issue(
    db: &MetricsDb,
    anomalies: &[Anomaly],
    issue_number: i32,
) -> Result<()> {
    let session_ids: Vec<_> = anomalies
        .iter()
        .map(|a| (a.session_id.clone(), a.dimension.clone()))
        .collect();

    db.mark_anomalies_investigated(&session_ids, issue_number)
}

// Note: AnomalySeverity::to_string() is already implemented in metrics.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus};

    #[test]
    fn test_detect_patterns() {
        let db = MetricsDb::in_memory().unwrap();

        // Create some sessions with high error rates
        for i in 0..5 {
            let session = SessionMetrics {
                id: format!("session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test".to_string(),
                duration_seconds: 100.0,
                tokens_in: 1000,
                tokens_out: 500,
                tokens_cached: 0,
                cost_usd: 0.05,
                issues_claimed: 1,
                issues_completed: 1,
                tool_calls: 100,
                tool_errors: 25, // 25% error rate - anomalous
                final_status: SessionStatus::Completed,
                messages: 10,
                apm: None,
                source: "autopilot".to_string(),
                issue_numbers: None,
                directive_id: None,
            };
            db.store_session(&session).unwrap();

            // Detect and store anomalies
            let anomalies = db.detect_anomalies(&session).unwrap();
            for anomaly in anomalies {
                db.store_anomaly(&anomaly).unwrap();
            }
        }

        // Detect patterns
        let patterns = detect_patterns(&db).unwrap();

        // Should have at least one pattern for tool_error_rate
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|p| p.dimension == "tool_error_rate"));
    }

    #[test]
    fn test_generate_issues() {
        let pattern = AnomalyPattern {
            dimension: "tool_error_rate".to_string(),
            occurrence_count: 5,
            severity: AnomalySeverity::Error,
            avg_deviation: 0.15, // 15% deviation
            anomalies: vec![],
        };

        let issues = generate_issues(vec![Pattern::Anomaly(pattern)]);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].title.contains("tool error rate"));
        assert!(issues[0].description.contains("Evidence"));
        assert!(issues[0].description.contains("Proposed Fix"));
        // Enhanced priority calculation: severity(2) + frequency(1) + deviation(0) = 3 -> medium
        assert_eq!(issues[0].priority, "medium");
    }

    #[test]
    fn test_generate_issues_urgent_priority() {
        // Test urgent priority: Critical + high frequency + high deviation
        let pattern = AnomalyPattern {
            dimension: "tool_error_rate".to_string(),
            occurrence_count: 10,                // frequency_score = 2
            severity: AnomalySeverity::Critical, // severity_score = 3
            avg_deviation: 0.60,                 // 60% deviation, deviation_score = 2
            anomalies: vec![],
        };

        let issues = generate_issues(vec![Pattern::Anomaly(pattern)]);

        assert_eq!(issues.len(), 1);
        // Total score: 3 + 2 + 2 = 7 -> urgent
        assert_eq!(issues[0].priority, "urgent");
    }

    #[test]
    fn test_generate_issues_high_priority() {
        // Test high priority: Error + moderate frequency + significant deviation
        let pattern = AnomalyPattern {
            dimension: "cost_per_issue".to_string(),
            occurrence_count: 5,              // frequency_score = 1
            severity: AnomalySeverity::Error, // severity_score = 2
            avg_deviation: 0.30,              // 30% deviation, deviation_score = 1
            anomalies: vec![],
        };

        let issues = generate_issues(vec![Pattern::Anomaly(pattern)]);

        assert_eq!(issues.len(), 1);
        // Total score: 2 + 1 + 1 = 4 -> high
        assert_eq!(issues[0].priority, "high");
    }

    #[test]
    fn test_create_issues_end_to_end() {
        use issues::db::init_memory_db;

        // Setup metrics database with anomalies
        let metrics_db = MetricsDb::in_memory().unwrap();

        // Create sessions with high error rates to trigger anomalies
        for i in 0..3 {
            let session = SessionMetrics {
                id: format!("test-session-{}", i),
                timestamp: Utc::now(),
                model: "sonnet".to_string(),
                prompt: "Test task".to_string(),
                duration_seconds: 100.0,
                tokens_in: 1000,
                tokens_out: 500,
                tokens_cached: 0,
                cost_usd: 0.05,
                issues_claimed: 1,
                issues_completed: 1,
                tool_calls: 100,
                tool_errors: 30, // 30% error rate - anomalous
                final_status: SessionStatus::Completed,
                messages: 10,
                apm: None,
                source: "autopilot".to_string(),
                issue_numbers: None,
                directive_id: None,
            };
            metrics_db.store_session(&session).unwrap();

            // Detect and store anomalies
            let anomalies = metrics_db.detect_anomalies(&session).unwrap();
            for anomaly in anomalies {
                metrics_db.store_anomaly(&anomaly).unwrap();
            }
        }

        // Setup issues database
        let _issues_conn = init_memory_db().unwrap();

        // Detect patterns
        let patterns = detect_all_patterns(&metrics_db).unwrap();
        assert!(!patterns.is_empty(), "Should detect at least one pattern");

        // Generate issues
        let improvement_issues = generate_issues(patterns);
        assert!(
            !improvement_issues.is_empty(),
            "Should generate at least one issue"
        );

        // Create a temporary file path for the issues database
        let temp_dir = std::env::temp_dir();
        let issues_db_path = temp_dir.join("test_auto_issues.db");

        // Create issues database file
        {
            use issues::db::init_db;
            let _conn = init_db(&issues_db_path).unwrap();
        }

        // Create issues using the create_issues function
        let issue_numbers =
            create_issues(&issues_db_path, &improvement_issues, &metrics_db).unwrap();

        assert!(
            !issue_numbers.is_empty(),
            "Should create at least one issue"
        );

        // Verify issues were created with auto_created flag
        let conn = Connection::open(&issues_db_path).unwrap();
        for issue_number in &issue_numbers {
            let auto_created: bool = conn
                .query_row(
                    "SELECT auto_created FROM issues WHERE number = ?",
                    [issue_number],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(auto_created, "Issue should be marked as auto_created");

            let directive_id: Option<String> = conn
                .query_row(
                    "SELECT directive_id FROM issues WHERE number = ?",
                    [issue_number],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                directive_id,
                Some("d-004".to_string()),
                "Issue should be linked to d-004"
            );
        }

        // Cleanup
        std::fs::remove_file(&issues_db_path).ok();
    }
}
