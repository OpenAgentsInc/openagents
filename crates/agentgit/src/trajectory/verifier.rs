//! Trajectory hash verification
//!
//! Verifies that trajectory hash in PR matches actual trajectory events.

use anyhow::Result;
use sha2::{Digest, Sha256};

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
        let events = vec![
            r#"{"type":"ToolUse"}"#.to_string(),
        ];

        let hash = calculate_trajectory_hash(&events).unwrap();
        assert!(verify_trajectory_hash(&events, &hash).unwrap());
        assert!(!verify_trajectory_hash(&events, "invalid").unwrap());
    }
}
