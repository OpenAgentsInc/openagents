//! Compare trajectory tool calls to git diffs
//!
//! Verifies that the diff in a PR matches the tool calls recorded in the trajectory.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Result of comparing trajectory to diff
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonResult {
    /// Overall match status
    pub status: MatchStatus,
    /// Files that appear in both trajectory and diff
    pub matched_files: Vec<String>,
    /// Files in trajectory but not in diff
    pub missing_in_diff: Vec<String>,
    /// Files in diff but not in trajectory
    pub extra_in_diff: Vec<String>,
    /// Detailed comparison notes
    pub notes: Vec<String>,
}

/// Match status between trajectory and diff
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    /// Perfect match
    FullMatch,
    /// Minor discrepancies (whitespace, formatting)
    MinorDiscrepancy,
    /// Major discrepancies (missing files, different changes)
    MajorDiscrepancy,
}

/// File modification from trajectory tool calls
#[derive(Debug, Clone)]
pub struct TrajectoryModification {
    pub file_path: String,
    pub tool: ToolType,
    pub line_count: Option<usize>,
}

/// Type of tool used in trajectory
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolType {
    Read,
    Edit,
    Write,
    MultiEdit,
    NotebookEdit,
    Other(String),
}

/// File change from git diff
#[derive(Debug, Clone)]
pub struct DiffChange {
    pub file_path: String,
    pub additions: usize,
    pub deletions: usize,
}

/// Compare trajectory tool calls to git diff
///
/// # Examples
///
/// ```no_run
/// use gitafter::trajectory::{compare_trajectory_to_diff, TrajectoryModification, ToolType};
///
/// let trajectory_mods = vec![
///     TrajectoryModification {
///         file_path: "src/main.rs".to_string(),
///         tool: ToolType::Edit,
///         line_count: Some(5),
///     },
/// ];
///
/// let diff_text = r#"
/// diff --git a/src/main.rs b/src/main.rs
/// +++ b/src/main.rs
/// @@ -1,3 +1,8 @@
/// +fn main() {
/// +    println!("Hello");
/// +}
/// "#;
///
/// let result = compare_trajectory_to_diff(&trajectory_mods, diff_text).unwrap();
/// println!("Match status: {:?}", result.status);
/// ```
pub fn compare_trajectory_to_diff(
    trajectory_mods: &[TrajectoryModification],
    diff_text: &str,
) -> Result<ComparisonResult> {
    // Extract files modified in trajectory
    let trajectory_files: HashSet<String> = trajectory_mods
        .iter()
        .filter(|m| {
            matches!(
                m.tool,
                ToolType::Edit | ToolType::Write | ToolType::MultiEdit | ToolType::NotebookEdit
            )
        })
        .map(|m| normalize_path(&m.file_path))
        .collect();

    // Parse diff to extract changed files
    let diff_changes = parse_diff(diff_text)?;
    let diff_files: HashSet<String> = diff_changes
        .iter()
        .map(|c| normalize_path(&c.file_path))
        .collect();

    // Compare file sets
    let matched: Vec<String> = trajectory_files
        .intersection(&diff_files)
        .cloned()
        .collect();

    let missing_in_diff: Vec<String> = trajectory_files.difference(&diff_files).cloned().collect();

    let extra_in_diff: Vec<String> = diff_files.difference(&trajectory_files).cloned().collect();

    // Determine status and generate notes
    let mut notes = Vec::new();
    let status = if missing_in_diff.is_empty() && extra_in_diff.is_empty() {
        notes.push(format!(
            "All {} file(s) match between trajectory and diff",
            matched.len()
        ));
        MatchStatus::FullMatch
    } else if !missing_in_diff.is_empty() {
        notes.push(format!(
            "{} file(s) in trajectory but not in diff: {}",
            missing_in_diff.len(),
            missing_in_diff.join(", ")
        ));
        MatchStatus::MajorDiscrepancy
    } else {
        notes.push(format!(
            "{} file(s) in diff but not in trajectory (may be auto-generated or manual edits)",
            extra_in_diff.len()
        ));
        MatchStatus::MinorDiscrepancy
    };

    Ok(ComparisonResult {
        status,
        matched_files: matched,
        missing_in_diff,
        extra_in_diff,
        notes,
    })
}

/// Parse trajectory events JSON to extract file modifications
///
/// # Examples
///
/// ```
/// use gitafter::trajectory::parse_trajectory_events;
///
/// let events_json = vec![
///     r#"{"type":"ToolUse","tool":"Edit","input":{"file_path":"src/main.rs"}}"#.to_string(),
///     r#"{"type":"ToolResult","success":true}"#.to_string(),
/// ];
///
/// let mods = parse_trajectory_events(&events_json).unwrap();
/// assert_eq!(mods.len(), 1);
/// assert_eq!(mods[0].file_path, "src/main.rs");
/// ```
pub fn parse_trajectory_events(events_json: &[String]) -> Result<Vec<TrajectoryModification>> {
    let mut modifications = Vec::new();

    for event_str in events_json {
        let event: serde_json::Value = serde_json::from_str(event_str)?;

        // Check if this is a ToolUse event
        if event.get("type").and_then(|t| t.as_str()) != Some("ToolUse") {
            continue;
        }

        let tool_name = event
            .get("tool")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        let tool = match tool_name {
            "Read" => ToolType::Read,
            "Edit" => ToolType::Edit,
            "Write" => ToolType::Write,
            "MultiEdit" => ToolType::MultiEdit,
            "NotebookEdit" => ToolType::NotebookEdit,
            other => ToolType::Other(other.to_string()),
        };

        // Extract file path from input
        if let Some(input) = event.get("input") {
            if let Some(file_path) = input.get("file_path").and_then(|f| f.as_str()) {
                modifications.push(TrajectoryModification {
                    file_path: file_path.to_string(),
                    tool,
                    line_count: None, // Could extract from Edit tool's old_string/new_string
                });
            }
        }
    }

    Ok(modifications)
}

/// Parse a unified diff to extract file changes
fn parse_diff(diff_text: &str) -> Result<Vec<DiffChange>> {
    let mut changes = Vec::new();
    let mut current_file: Option<String> = None;
    let mut additions = 0;
    let mut deletions = 0;

    for line in diff_text.lines() {
        if line.starts_with("diff --git") {
            // Save previous file if exists
            if let Some(file) = current_file.take() {
                changes.push(DiffChange {
                    file_path: file,
                    additions,
                    deletions,
                });
                additions = 0;
                deletions = 0;
            }

            // Parse new file path
            // Format: diff --git a/path/to/file b/path/to/file
            if let Some(path) = line.split_whitespace().nth(2) {
                current_file = Some(path.trim_start_matches("a/").to_string());
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }

    // Don't forget the last file
    if let Some(file) = current_file {
        changes.push(DiffChange {
            file_path: file,
            additions,
            deletions,
        });
    }

    Ok(changes)
}

/// Normalize file path for comparison
fn normalize_path(path: &str) -> String {
    // Remove leading slashes, ./, etc.
    path.trim_start_matches('/')
        .trim_start_matches("./")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trajectory_events() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Edit","input":{"file_path":"src/main.rs","old_string":"old","new_string":"new"}}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
            r#"{"type":"ToolUse","tool":"Write","input":{"file_path":"src/lib.rs","content":"code"}}"#.to_string(),
        ];

        let mods = parse_trajectory_events(&events).unwrap();
        assert_eq!(mods.len(), 2);
        assert_eq!(mods[0].file_path, "src/main.rs");
        assert_eq!(mods[0].tool, ToolType::Edit);
        assert_eq!(mods[1].file_path, "src/lib.rs");
        assert_eq!(mods[1].tool, ToolType::Write);
    }

    #[test]
    fn test_parse_diff() {
        let diff = r#"
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
+    println!("Hello");
+    println!("World");
 }
-// old comment
"#;

        let changes = parse_diff(diff).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].file_path, "src/main.rs");
        assert_eq!(changes[0].additions, 2);
        assert_eq!(changes[0].deletions, 1);
    }

    #[test]
    fn test_compare_full_match() {
        let trajectory_mods = vec![TrajectoryModification {
            file_path: "src/main.rs".to_string(),
            tool: ToolType::Edit,
            line_count: None,
        }];

        let diff = r#"
diff --git a/src/main.rs b/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
+new line
"#;

        let result = compare_trajectory_to_diff(&trajectory_mods, diff).unwrap();
        assert_eq!(result.status, MatchStatus::FullMatch);
        assert_eq!(result.matched_files.len(), 1);
        assert_eq!(result.missing_in_diff.len(), 0);
        assert_eq!(result.extra_in_diff.len(), 0);
    }

    #[test]
    fn test_compare_major_discrepancy() {
        let trajectory_mods = vec![
            TrajectoryModification {
                file_path: "src/main.rs".to_string(),
                tool: ToolType::Edit,
                line_count: None,
            },
            TrajectoryModification {
                file_path: "src/lib.rs".to_string(),
                tool: ToolType::Write,
                line_count: None,
            },
        ];

        let diff = r#"
diff --git a/src/main.rs b/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
+new line
"#;

        let result = compare_trajectory_to_diff(&trajectory_mods, diff).unwrap();
        assert_eq!(result.status, MatchStatus::MajorDiscrepancy);
        assert_eq!(result.matched_files.len(), 1);
        assert_eq!(result.missing_in_diff.len(), 1);
        assert!(result.missing_in_diff.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/src/main.rs"), "src/main.rs");
        assert_eq!(normalize_path("./src/main.rs"), "src/main.rs");
        assert_eq!(normalize_path("src/main.rs"), "src/main.rs");
    }
}
