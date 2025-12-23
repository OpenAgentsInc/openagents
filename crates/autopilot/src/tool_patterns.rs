//! Tool error pattern detection for specific failure types
//!
//! This module complements auto_issues.rs by detecting specific error patterns
//! from tool call data, such as:
//! - Tools with >10% error rate
//! - Specific error types (EISDIR, ENOENT, etc.)
//! - File-not-read errors
//! - Tool-specific failure patterns

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;

use crate::metrics::MetricsDb;

/// Pattern of tool-specific errors
#[derive(Debug, Clone)]
pub struct ToolErrorPattern {
    /// Tool name (e.g., "Read", "Write", "Bash")
    pub tool_name: String,
    /// Error type (e.g., "EISDIR", "ENOENT", or "HighErrorRate")
    pub error_type: String,
    /// Total calls to this tool
    pub total_calls: usize,
    /// Failed calls
    pub failed_calls: usize,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
    /// Specific error type counts
    pub error_breakdown: HashMap<String, usize>,
    /// Sample error messages
    pub sample_errors: Vec<String>,
}

/// Severity of tool error pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl ToolErrorPattern {
    /// Determine severity based on error rate and occurrence
    pub fn severity(&self) -> ToolErrorSeverity {
        if self.error_rate > 0.25 || self.failed_calls > 50 {
            ToolErrorSeverity::Critical
        } else if self.error_rate > 0.15 || self.failed_calls > 20 {
            ToolErrorSeverity::High
        } else if self.error_rate > 0.10 || self.failed_calls > 10 {
            ToolErrorSeverity::Medium
        } else {
            ToolErrorSeverity::Low
        }
    }
}

/// Detect tool error patterns from metrics database
pub fn detect_tool_patterns(db: &MetricsDb, min_calls: usize) -> Result<Vec<ToolErrorPattern>> {
    let conn = db.connection();

    // Get all tool calls with their error status
    let mut stmt = conn.prepare(
        r#"
        SELECT tool_name, success, error_type
        FROM tool_calls
        ORDER BY tool_name
        "#,
    )?;

    let mut tool_stats: HashMap<String, (usize, usize, HashMap<String, usize>)> = HashMap::new();
    let mut tool_errors: HashMap<String, Vec<String>> = HashMap::new();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i32>(1)? != 0, // success
            row.get::<_, Option<String>>(2)?, // error_type
        ))
    })?;

    for row in rows {
        let (tool_name, success, error_type) = row?;

        let entry = tool_stats.entry(tool_name.clone()).or_insert((0, 0, HashMap::new()));
        entry.0 += 1; // total calls

        if !success {
            entry.1 += 1; // failed calls

            if let Some(ref err_type) = error_type {
                *entry.2.entry(err_type.clone()).or_insert(0) += 1;

                // Store sample error messages (limit to 5 per tool)
                let errors = tool_errors.entry(tool_name.clone()).or_default();
                if errors.len() < 5 {
                    errors.push(err_type.clone());
                }
            }
        }
    }

    // Convert to patterns
    let mut patterns = Vec::new();

    for (tool_name, (total_calls, failed_calls, error_breakdown)) in tool_stats {
        // Skip tools with very few calls
        if total_calls < min_calls {
            continue;
        }

        let error_rate = failed_calls as f64 / total_calls as f64;

        // Only create pattern if error rate is significant (>5%)
        if error_rate > 0.05 {
            let sample_errors = tool_errors.get(&tool_name).cloned().unwrap_or_default();

            // Determine primary error type
            let primary_error = if error_rate > 0.10 {
                "HighErrorRate".to_string()
            } else if let Some((err_type, _)) = error_breakdown.iter().max_by_key(|(_, count)| *count) {
                err_type.clone()
            } else {
                "Unknown".to_string()
            };

            patterns.push(ToolErrorPattern {
                tool_name,
                error_type: primary_error,
                total_calls,
                failed_calls,
                error_rate,
                error_breakdown,
                sample_errors,
            });
        }
    }

    // Sort by severity and error rate
    patterns.sort_by(|a, b| {
        b.severity().cmp(&a.severity())
            .then_with(|| b.error_rate.partial_cmp(&a.error_rate).unwrap())
    });

    Ok(patterns)
}

/// Detect specific error type patterns (EISDIR, ENOENT, etc.)
pub fn detect_error_type_patterns(db: &MetricsDb, min_occurrences: usize) -> Result<Vec<(String, usize, Vec<String>)>> {
    let conn = get_connection(db)?;

    let mut stmt = conn.prepare(
        r#"
        SELECT error_type, COUNT(*) as count, GROUP_CONCAT(tool_name, ',') as tools
        FROM tool_calls
        WHERE success = 0 AND error_type IS NOT NULL
        GROUP BY error_type
        HAVING count >= ?1
        ORDER BY count DESC
        "#,
    )?;

    let patterns = stmt
        .query_map([min_occurrences], |row| {
            let error_type: String = row.get(0)?;
            let count: usize = row.get(1)?;
            let tools_str: String = row.get(2)?;
            let tools: Vec<String> = tools_str.split(',')
                .map(|s| s.to_string())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            Ok((error_type, count, tools))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(patterns)
}

/// Generate issue title for tool error pattern
pub fn generate_tool_pattern_title(pattern: &ToolErrorPattern) -> String {
    match pattern.error_type.as_str() {
        "HighErrorRate" => {
            format!(
                "{} tool has {:.1}% error rate ({} failures in {} calls)",
                pattern.tool_name,
                pattern.error_rate * 100.0,
                pattern.failed_calls,
                pattern.total_calls
            )
        }
        "EISDIR" => {
            format!(
                "{} tool: {} EISDIR errors (attempting to read directories)",
                pattern.tool_name,
                pattern.error_breakdown.get("EISDIR").unwrap_or(&0)
            )
        }
        "ENOENT" => {
            format!(
                "{} tool: {} ENOENT errors (file not found)",
                pattern.tool_name,
                pattern.error_breakdown.get("ENOENT").unwrap_or(&0)
            )
        }
        "NonZeroExit" => {
            format!(
                "{} tool: {} non-zero exit errors",
                pattern.tool_name,
                pattern.error_breakdown.get("NonZeroExit").unwrap_or(&0)
            )
        }
        _ => {
            format!(
                "{} tool: {} {} errors ({:.1}% error rate)",
                pattern.tool_name,
                pattern.failed_calls,
                pattern.error_type,
                pattern.error_rate * 100.0
            )
        }
    }
}

/// Generate issue description for tool error pattern
pub fn generate_tool_pattern_description(pattern: &ToolErrorPattern) -> String {
    let mut desc = String::new();

    desc.push_str(&format!(
        "Detected high error rate for **{}** tool across {} calls.\n\n",
        pattern.tool_name, pattern.total_calls
    ));

    desc.push_str("## Statistics\n\n");
    desc.push_str(&format!("- **Total Calls**: {}\n", pattern.total_calls));
    desc.push_str(&format!("- **Failed Calls**: {}\n", pattern.failed_calls));
    desc.push_str(&format!("- **Error Rate**: {:.1}%\n", pattern.error_rate * 100.0));
    desc.push_str(&format!("- **Severity**: {:?}\n\n", pattern.severity()));

    if !pattern.error_breakdown.is_empty() {
        desc.push_str("### Error Type Breakdown\n\n");
        let mut errors: Vec<_> = pattern.error_breakdown.iter().collect();
        errors.sort_by_key(|(_, count)| std::cmp::Reverse(**count));

        for (error_type, count) in errors {
            let percentage = (*count as f64 / pattern.failed_calls as f64) * 100.0;
            desc.push_str(&format!("- **{}**: {} ({:.1}% of failures)\n", error_type, count, percentage));
        }
        desc.push_str("\n");
    }

    if !pattern.sample_errors.is_empty() {
        desc.push_str("### Sample Errors\n\n");
        for (i, error) in pattern.sample_errors.iter().take(5).enumerate() {
            desc.push_str(&format!("{}. `{}`\n", i + 1, error));
        }
        desc.push_str("\n");
    }

    desc.push_str("## Root Cause Analysis\n\n");
    desc.push_str(&generate_root_cause(pattern));

    desc.push_str("\n## Proposed Fix\n\n");
    desc.push_str(&generate_tool_fix(pattern));

    desc.push_str("\n## Investigation Commands\n\n");
    desc.push_str(&generate_investigation_commands(pattern));

    desc.push_str("\n---\n\n");
    desc.push_str("*This issue was automatically generated by tool pattern detection (d-004).*\n");

    desc
}

/// Generate root cause analysis
fn generate_root_cause(pattern: &ToolErrorPattern) -> String {
    match pattern.tool_name.as_str() {
        "Read" => {
            if pattern.error_breakdown.contains_key("EISDIR") {
                "The Read tool is being called on directory paths instead of file paths. This suggests:\n\
                - Glob patterns may be resolving to directories\n\
                - Path validation is missing before Read calls\n\
                - Agent may be confused about file vs directory targets\n".to_string()
            } else if pattern.error_breakdown.contains_key("ENOENT") {
                "Files are being read before they exist or with incorrect paths. This suggests:\n\
                - Race conditions between file creation and reading\n\
                - Incorrect path construction or resolution\n\
                - Missing error handling for non-existent files\n".to_string()
            } else {
                "Generic Read tool failures. Investigate specific error types.\n".to_string()
            }
        }
        "Write" | "Edit" => {
            "File modification failures. Common causes:\n\
            - Permission issues\n\
            - Invalid file paths\n\
            - Attempting to edit files that haven't been read first\n".to_string()
        }
        "Bash" => {
            if pattern.error_breakdown.contains_key("NonZeroExit") {
                "Commands are exiting with non-zero status. This suggests:\n\
                - Commands are failing due to invalid arguments\n\
                - Required tools or files are missing\n\
                - Commands need better error handling\n".to_string()
            } else {
                "Bash command execution failures. Review command construction and validation.\n".to_string()
            }
        }
        "Glob" => {
            "File pattern matching failures. This suggests:\n\
            - Invalid glob patterns\n\
            - Searching in non-existent directories\n\
            - Pattern syntax errors\n".to_string()
        }
        _ => {
            format!("High error rate for {} tool. Requires investigation.\n", pattern.tool_name)
        }
    }
}

/// Generate fix recommendations
fn generate_tool_fix(pattern: &ToolErrorPattern) -> String {
    match pattern.tool_name.as_str() {
        "Read" => {
            if pattern.error_breakdown.contains_key("EISDIR") {
                "1. Add directory detection before Read tool calls:\n   \
                   ```rust\n   \
                   if path.is_dir() { /* use Glob or list files */ }\n   \
                   else { /* use Read */ }\n   \
                   ```\n\
                2. Update system prompt to clarify when to use Glob vs Read\n\
                3. Add validation in Read tool implementation\n\
                4. Consider auto-converting directory reads to `ls` commands\n".to_string()
            } else if pattern.error_breakdown.contains_key("ENOENT") {
                "1. Add file existence check before Read calls\n\
                2. Improve error messages to suggest using Glob first\n\
                3. Add retry logic with delay for race conditions\n\
                4. Update system prompt about checking file existence\n".to_string()
            } else {
                "1. Review Read tool error logs for patterns\n\
                2. Add better error messages with suggested fixes\n\
                3. Improve path validation and normalization\n".to_string()
            }
        }
        "Bash" => {
            "1. Add command validation before execution\n\
            2. Improve error messages to include command output\n\
            3. Add retry logic for transient failures\n\
            4. Update system prompt about command construction\n\
            5. Consider adding a whitelist of safe commands\n".to_string()
        }
        _ => {
            format!(
                "1. Review {} tool implementation for bugs\n\
                2. Improve error handling and messages\n\
                3. Add validation for tool inputs\n\
                4. Update system prompt with usage guidelines\n",
                pattern.tool_name
            )
        }
    }
}

/// Generate investigation commands
fn generate_investigation_commands(pattern: &ToolErrorPattern) -> String {
    format!(
        "```bash\n\
        # Query tool calls for this tool\n\
        sqlite3 autopilot-metrics.db \"SELECT session_id, timestamp, success, error_type \
        FROM tool_calls WHERE tool_name = '{}' AND success = 0 LIMIT 20\"\n\
        \n\
        # Get detailed error breakdown\n\
        sqlite3 autopilot-metrics.db \"SELECT error_type, COUNT(*) as count \
        FROM tool_calls WHERE tool_name = '{}' AND success = 0 \
        GROUP BY error_type ORDER BY count DESC\"\n\
        \n\
        # Find sessions with high error rates\n\
        sqlite3 autopilot-metrics.db \"SELECT s.id, s.tool_calls, s.tool_errors, \
        (s.tool_errors * 1.0 / s.tool_calls) as error_rate \
        FROM sessions s WHERE s.tool_errors > 5 \
        ORDER BY error_rate DESC LIMIT 10\"\n\
        ```\n",
        pattern.tool_name,
        pattern.tool_name
    )
}

/// Generate priority for tool error pattern
pub fn generate_tool_pattern_priority(pattern: &ToolErrorPattern) -> String {
    match pattern.severity() {
        ToolErrorSeverity::Critical => "urgent",
        ToolErrorSeverity::High => "high",
        ToolErrorSeverity::Medium => "medium",
        ToolErrorSeverity::Low => "low",
    }.to_string()
}

/// Get database connection (helper)
fn get_connection(db: &MetricsDb) -> Result<Connection> {
    Connection::open(db.path()).context("Failed to open metrics database connection")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{MetricsDb, SessionMetrics, SessionStatus, ToolCallMetrics};
    use chrono::Utc;

    #[test]
    fn test_detect_tool_patterns() {
        let db = MetricsDb::in_memory().unwrap();

        // Create a session
        let mut session = SessionMetrics {
            id: "test-session".to_string(),
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
            tool_calls: 20,
            tool_errors: 5,
            final_status: SessionStatus::Completed,
            messages: 15,
            apm: None,
            source: "autopilot".to_string(),
            issue_numbers: None,
            directive_id: None,
        };
        session.calculate_apm();
        db.store_session(&session).unwrap();

        // Add tool calls with errors
        for i in 0..20 {
            let tc = ToolCallMetrics {
                session_id: "test-session".to_string(),
                timestamp: Utc::now(),
                tool_name: "Read".to_string(),
                duration_ms: 100,
                success: i >= 5, // First 5 fail
                error_type: if i < 5 { Some("EISDIR".to_string()) } else { None },
                tokens_in: 10,
                tokens_out: 5,
            };
            db.store_tool_call(&tc).unwrap();
        }

        // Detect patterns
        let patterns = detect_tool_patterns(&db, 10).unwrap();

        // Should have one pattern for Read tool
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].tool_name, "Read");
        assert_eq!(patterns[0].failed_calls, 5);
        assert_eq!(patterns[0].total_calls, 20);
    }
}
