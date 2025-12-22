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

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

use crate::metrics::{Anomaly, AnomalySeverity, MetricsDb};
use crate::tool_patterns::{
    detect_tool_patterns, generate_tool_pattern_description, generate_tool_pattern_priority,
    generate_tool_pattern_title, ToolErrorPattern,
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
        .map(|(session_id, dimension, expected_value, actual_value, severity_str)| {
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
        })
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

    let priority = match pattern.severity {
        AnomalySeverity::Critical => "urgent",
        AnomalySeverity::Error => "high",
        AnomalySeverity::Warning => "medium",
    };

    (title, description, priority.to_string())
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
        severity_str,
        pattern.dimension,
        pattern.occurrence_count
    ));

    // Evidence
    desc.push_str("## Evidence\n\n");
    desc.push_str(&format!(
        "- **Occurrences**: {} sessions\n",
        pattern.occurrence_count
    ));
    desc.push_str(&format!(
        "- **Severity**: {:?}\n",
        pattern.severity
    ));
    desc.push_str(&format!(
        "- **Average deviation**: {:.1}%\n\n",
        pattern.avg_deviation * 100.0
    ));

    // Sample anomalies
    desc.push_str("### Affected Sessions\n\n");
    for (i, anomaly) in pattern.anomalies.iter().take(5).enumerate() {
        desc.push_str(&format!(
            "{}. Session `{}...` - Expected: {:.3}, Actual: {:.3} ({:+.1}%)\n",
            i + 1,
            &anomaly.session_id[..8.min(anomaly.session_id.len())],
            anomaly.expected_value,
            anomaly.actual_value,
            ((anomaly.actual_value - anomaly.expected_value) / anomaly.expected_value) * 100.0
        ));
    }
    if pattern.anomalies.len() > 5 {
        desc.push_str(&format!(
            "\n...and {} more sessions\n",
            pattern.anomalies.len() - 5
        ));
    }
    desc.push_str("\n");

    // Proposed fix
    desc.push_str("## Proposed Fix\n\n");
    desc.push_str(&generate_proposed_fix(pattern));

    // Investigation steps
    desc.push_str("\n## Investigation Steps\n\n");
    desc.push_str(&generate_investigation_steps(pattern));

    // Auto-generated footer
    desc.push_str("\n---\n\n");
    desc.push_str("*This issue was automatically generated by autopilot metrics analysis (d-004).*\n");

    desc
}

/// Generate proposed fix based on pattern type
fn generate_proposed_fix(pattern: &AnomalyPattern) -> String {
    match pattern.dimension.as_str() {
        "tool_error_rate" | "tool_error_rate_zscore" => {
            "Investigate common tool errors:\n\
            1. Query metrics database for most frequent error types in affected sessions\n\
            2. Identify if errors are concentrated in specific tools (Read, Write, Bash, etc.)\n\
            3. Review trajectory logs for error patterns (EISDIR, ENOENT, permission denied, etc.)\n\
            4. Add validation or guardrails to prevent these errors\n\
            5. Update system prompts or hooks if errors indicate instruction non-compliance\n".to_string()
        }
        "tokens_per_issue" => {
            "Analyze token consumption patterns:\n\
            1. Compare affected sessions to baseline sessions\n\
            2. Identify if token usage correlates with specific issue types or complexity\n\
            3. Review for inefficient tool usage patterns (redundant reads, excessive thinking)\n\
            4. Consider if compaction is triggering appropriately\n\
            5. Evaluate if model selection rules need tuning\n".to_string()
        }
        "cost_per_issue" => {
            "Optimize cost efficiency:\n\
            1. Review high-cost sessions for wasteful patterns\n\
            2. Check cache hit rates - low cache usage increases costs\n\
            3. Evaluate if cheaper models (haiku) could handle simpler tasks\n\
            4. Identify opportunities for better task batching\n\
            5. Consider implementing cost budgets per issue type\n".to_string()
        }
        "completion_rate" => {
            "Improve task completion:\n\
            1. Analyze why issues were claimed but not completed\n\
            2. Check for common blockers (missing files, unclear requirements)\n\
            3. Review if max_turns or budget limits are too restrictive\n\
            4. Evaluate if tasks need better decomposition\n\
            5. Consider adding retry logic or fallback strategies\n".to_string()
        }
        "session_duration" => {
            "Investigate duration anomalies:\n\
            1. Determine if longer durations correlate with success or failure\n\
            2. Check for stuck processes or infinite loops\n\
            3. Review tool latency - identify slow operations\n\
            4. Evaluate if parallelization is being utilized\n\
            5. Consider timeout mechanisms for outlier cases\n".to_string()
        }
        _ => {
            "Generic investigation steps:\n\
            1. Review affected sessions in detail using `cargo autopilot metrics show <session-id>`\n\
            2. Compare trajectories to identify common patterns\n\
            3. Propose specific improvements based on findings\n\
            4. Update baselines after implementing fixes\n\
            5. Monitor metrics to confirm improvement\n".to_string()
        }
    }
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
         ```\n\n"
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

/// Create issues in the database
pub fn create_issues(
    issues_db_path: &Path,
    improvement_issues: &[ImprovementIssue],
    metrics_db: &MetricsDb,
) -> Result<Vec<i32>> {
    let conn = Connection::open(issues_db_path)
        .context("Failed to open issues database")?;

    let mut created_issue_numbers = Vec::new();

    for issue in improvement_issues {
        // Get next issue number
        let next_number: i32 = conn.query_row(
            "SELECT next_number FROM issue_counter WHERE id = 1",
            [],
            |row| row.get(0),
        )?;

        // Create issue
        let now = Utc::now().to_rfc3339();
        conn.execute(
            r#"
            INSERT INTO issues
            (number, title, description, status, priority, issue_type, agent,
             is_blocked, blocked_reason, claimed_by, claimed_at, created_at,
             updated_at, completed_at, directive_id)
            VALUES (?1, ?2, ?3, 'open', ?4, 'task', 'claude', 0, '', NULL, NULL, ?5, ?5, NULL, 'd-004')
            "#,
            rusqlite::params![
                next_number,
                issue.title,
                issue.description,
                issue.priority,
                now,
            ],
        )?;

        // Increment counter
        conn.execute(
            "UPDATE issue_counter SET next_number = next_number + 1 WHERE id = 1",
            [],
        )?;

        created_issue_numbers.push(next_number);

        // Mark anomalies as investigated and link to issue (only for anomaly patterns)
        match &issue.pattern {
            Pattern::Anomaly(p) => {
                mark_anomalies_with_issue(metrics_db, &p.anomalies, next_number)?;
            }
            Pattern::ToolError(_) => {
                // Tool error patterns don't have anomalies to mark
            }
        }

        println!(
            "  ✓ Created issue #{}: {} [{}]",
            next_number, issue.title, issue.priority
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
        assert!(patterns
            .iter()
            .any(|p| p.dimension == "tool_error_rate"));
    }

    #[test]
    fn test_generate_issues() {
        let pattern = AnomalyPattern {
            dimension: "tool_error_rate".to_string(),
            occurrence_count: 5,
            severity: AnomalySeverity::Error,
            avg_deviation: 0.15,
            anomalies: vec![],
        };

        let issues = generate_issues(vec![Pattern::Anomaly(pattern)]);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].title.contains("tool error rate"));
        assert!(issues[0].description.contains("Evidence"));
        assert!(issues[0].description.contains("Proposed Fix"));
        assert_eq!(issues[0].priority, "high");
    }
}
