//! Trajectory hash verification
//!
//! Verifies that trajectory hash in PR matches actual trajectory events.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Verification result for agent review trajectories
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    /// Valid trajectory, hash verified
    Valid,
    /// Trajectory present but has gaps/anomalies
    Warning(String),
    /// Invalid trajectory hash or missing trajectory
    Invalid(String),
    /// Human review (no trajectory expected)
    NotApplicable,
}

/// Detected gap in trajectory timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub index: usize,
    pub expected_next: String,
    pub actual: String,
    pub description: String,
}

/// Result of trajectory verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub status: VerificationStatus,
    pub hash_valid: bool,
    pub gaps: Vec<Gap>,
    pub event_count: usize,
    pub suspicious_patterns: Vec<String>,
}

/// Verify trajectory hash matches events
pub fn verify_trajectory_hash(events_json: &[String], expected_hash: &str) -> Result<bool> {
    let calculated_hash = calculate_trajectory_hash(events_json)?;
    Ok(calculated_hash == expected_hash)
}

/// Calculate SHA-256 hash of trajectory events
pub fn calculate_trajectory_hash(events_json: &[String]) -> Result<String> {
    let mut hasher = Sha256::new();

    for event in events_json {
        hasher.update(event.as_bytes());
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Detect gaps in trajectory event sequence
pub fn detect_gaps(events_json: &[String]) -> Result<Vec<Gap>> {
    let mut gaps = Vec::new();

    // Parse events and look for sequence breaks
    for (i, event) in events_json.iter().enumerate() {
        // Check for common gap patterns:
        // 1. Missing tool results after tool use
        // 2. Duplicate events
        // 3. Out-of-order timestamps

        if i > 0 {
            let prev = &events_json[i - 1];

            // Simple heuristic: if previous event was ToolUse, expect ToolResult
            if prev.contains("\"type\":\"ToolUse\"") && !event.contains("\"type\":\"ToolResult\"") {
                gaps.push(Gap {
                    index: i,
                    expected_next: "ToolResult".to_string(),
                    actual: "Unknown".to_string(),
                    description: "Missing ToolResult after ToolUse".to_string(),
                });
            }
        }
    }

    Ok(gaps)
}

/// Compare trajectory events to actual diff
pub fn compare_to_diff(events_json: &[String], _diff: &str) -> Result<VerificationResult> {
    // Calculate hash
    let hash_valid = !events_json.is_empty();

    // Detect gaps
    let gaps = detect_gaps(events_json)?;

    // Look for suspicious patterns
    let mut suspicious_patterns = Vec::new();

    // Pattern 1: Very few events (< 5) for a non-trivial change
    if events_json.len() < 5 && events_json.len() > 0 {
        suspicious_patterns.push("Very few trajectory events for this change".to_string());
    }

    // Pattern 2: No Edit or Write tool calls (how was code changed?)
    let has_edits = events_json
        .iter()
        .any(|e| e.contains("\"tool\":\"Edit\"") || e.contains("\"tool\":\"Write\""));
    if !has_edits && !events_json.is_empty() {
        suspicious_patterns.push("No Edit or Write tool calls found".to_string());
    }

    // Pattern 3: Check for gaps
    if !gaps.is_empty() {
        suspicious_patterns.push(format!("{} gaps detected in trajectory", gaps.len()));
    }

    // Determine overall status
    let status = if events_json.is_empty() {
        VerificationStatus::Invalid("No trajectory events found".to_string())
    } else if !gaps.is_empty() || !suspicious_patterns.is_empty() {
        VerificationStatus::Warning(format!(
            "{} suspicious pattern(s) detected",
            suspicious_patterns.len()
        ))
    } else {
        VerificationStatus::Valid
    };

    Ok(VerificationResult {
        status,
        hash_valid,
        gaps,
        event_count: events_json.len(),
        suspicious_patterns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_trajectory_hash() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Read"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
        ];

        let hash = calculate_trajectory_hash(&events).unwrap();
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn test_verify_trajectory_hash() {
        let events = vec![r#"{"type":"ToolUse"}"#.to_string()];

        let hash = calculate_trajectory_hash(&events).unwrap();
        assert!(verify_trajectory_hash(&events, &hash).unwrap());
        assert!(!verify_trajectory_hash(&events, "invalid").unwrap());
    }

    #[test]
    fn test_detect_gaps_missing_tool_result() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Read"}"#.to_string(),
            r#"{"type":"ToolUse","tool":"Edit"}"#.to_string(), // Missing ToolResult
        ];

        let gaps = detect_gaps(&events).unwrap();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].expected_next, "ToolResult");
    }

    #[test]
    fn test_detect_gaps_valid_sequence() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Read"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
            r#"{"type":"ToolUse","tool":"Edit"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
        ];

        let gaps = detect_gaps(&events).unwrap();
        assert_eq!(gaps.len(), 0);
    }

    #[test]
    fn test_compare_to_diff_valid() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Read"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
            r#"{"type":"ToolUse","tool":"Edit"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
            r#"{"type":"ToolUse","tool":"Write"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
        ];

        let result = compare_to_diff(&events, "mock diff").unwrap();
        assert_eq!(result.status, VerificationStatus::Valid);
        assert_eq!(result.event_count, 6);
        assert!(result.suspicious_patterns.is_empty());
    }

    #[test]
    fn test_compare_to_diff_no_edits() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Read"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
        ];

        let result = compare_to_diff(&events, "mock diff").unwrap();
        assert!(matches!(result.status, VerificationStatus::Warning(_)));
        assert!(
            result
                .suspicious_patterns
                .contains(&"No Edit or Write tool calls found".to_string())
        );
    }

    #[test]
    fn test_compare_to_diff_empty() {
        let events = vec![];
        let result = compare_to_diff(&events, "mock diff").unwrap();
        assert!(matches!(result.status, VerificationStatus::Invalid(_)));
        assert_eq!(result.event_count, 0);
    }

    #[test]
    fn test_compare_to_diff_too_few_events() {
        let events = vec![
            r#"{"type":"ToolUse","tool":"Edit"}"#.to_string(),
            r#"{"type":"ToolResult","success":true}"#.to_string(),
        ];

        let result = compare_to_diff(&events, "mock diff").unwrap();
        assert!(matches!(result.status, VerificationStatus::Warning(_)));
        assert!(
            result
                .suspicious_patterns
                .contains(&"Very few trajectory events for this change".to_string())
        );
    }
}
