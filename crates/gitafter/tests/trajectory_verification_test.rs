//! Comprehensive tests for trajectory hash verification
//!
//! These tests ensure agents cannot falsify their work history by verifying:
//! - Hash calculation is deterministic
//! - Valid trajectories pass verification
//! - Tampered trajectories are detected
//! - Missing events are detected
//! - Full PR→trajectory→verification flow works end-to-end

use anyhow::Result;
use gitafter::trajectory::verifier::{
    VerificationStatus, calculate_trajectory_hash, compare_to_diff, detect_gaps,
    verify_trajectory_hash,
};

/// Helper to create sample trajectory events
fn sample_trajectory_events() -> Vec<String> {
    vec![
        r#"{"type":"ToolUse","tool":"Read","args":{"file":"src/main.rs"}}"#.to_string(),
        r#"{"type":"ToolResult","tool":"Read","output":"fn main() {}"}"#.to_string(),
        r#"{"type":"ToolUse","tool":"Edit","args":{"file":"src/main.rs","content":"fn main() { println!(\"Hello\"); }"}}"#.to_string(),
        r#"{"type":"ToolResult","tool":"Edit","output":"Success"}"#.to_string(),
    ]
}

#[test]
fn test_calculate_trajectory_hash_deterministic() -> Result<()> {
    let events = sample_trajectory_events();

    // Calculate hash multiple times
    let hash1 = calculate_trajectory_hash(&events)?;
    let hash2 = calculate_trajectory_hash(&events)?;
    let hash3 = calculate_trajectory_hash(&events)?;

    // All should be identical
    assert_eq!(hash1, hash2);
    assert_eq!(hash2, hash3);

    // Hash should be 64 hex characters (SHA-256)
    assert_eq!(hash1.len(), 64);
    assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));

    Ok(())
}

#[test]
fn test_calculate_hash_empty_trajectory() -> Result<()> {
    let empty_events: Vec<String> = vec![];
    let hash = calculate_trajectory_hash(&empty_events)?;

    // Empty trajectory should have a specific hash (hash of nothing)
    assert_eq!(hash.len(), 64);

    // Should differ from non-empty
    let non_empty_hash = calculate_trajectory_hash(&sample_trajectory_events())?;
    assert_ne!(hash, non_empty_hash);

    Ok(())
}

#[test]
fn test_calculate_hash_order_matters() -> Result<()> {
    let mut events = sample_trajectory_events();
    let hash1 = calculate_trajectory_hash(&events)?;

    // Reverse order
    events.reverse();
    let hash2 = calculate_trajectory_hash(&events)?;

    // Hashes should differ (order matters)
    assert_ne!(hash1, hash2);

    Ok(())
}

#[test]
fn test_verify_trajectory_hash_valid() -> Result<()> {
    let events = sample_trajectory_events();
    let hash = calculate_trajectory_hash(&events)?;

    // Verification should pass
    assert!(verify_trajectory_hash(&events, &hash)?);

    Ok(())
}

#[test]
fn test_verify_trajectory_hash_tampered() -> Result<()> {
    let mut events = sample_trajectory_events();
    let original_hash = calculate_trajectory_hash(&events)?;

    // Tamper with an event
    events[2] = r#"{"type":"ToolUse","tool":"Edit","args":{"file":"src/main.rs","content":"fn main() { println!(\"Hacked\"); }"}}"#.to_string();

    // Verification should fail
    assert!(!verify_trajectory_hash(&events, &original_hash)?);

    Ok(())
}

#[test]
fn test_verify_trajectory_hash_missing_event() -> Result<()> {
    let mut events = sample_trajectory_events();
    let original_hash = calculate_trajectory_hash(&events)?;

    // Remove an event
    events.remove(1);

    // Verification should fail
    assert!(!verify_trajectory_hash(&events, &original_hash)?);

    Ok(())
}

#[test]
fn test_verify_trajectory_hash_extra_event() -> Result<()> {
    let mut events = sample_trajectory_events();
    let original_hash = calculate_trajectory_hash(&events)?;

    // Add an extra event
    events.push(r#"{"type":"ToolUse","tool":"Bash","args":{"cmd":"echo hacked"}}"#.to_string());

    // Verification should fail
    assert!(!verify_trajectory_hash(&events, &original_hash)?);

    Ok(())
}

#[test]
fn test_verify_trajectory_hash_wrong_hash() -> Result<()> {
    let events = sample_trajectory_events();
    let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    // Verification should fail with obviously wrong hash
    assert!(!verify_trajectory_hash(&events, wrong_hash)?);

    Ok(())
}

#[test]
fn test_detect_gaps_complete_trajectory() -> Result<()> {
    let events = sample_trajectory_events();
    let gaps = detect_gaps(&events)?;

    // Should have no gaps
    assert_eq!(gaps.len(), 0);

    Ok(())
}

#[test]
fn test_detect_gaps_missing_tool_result() -> Result<()> {
    let events = vec![
        r#"{"type":"ToolUse","tool":"Read","args":{"file":"src/main.rs"}}"#.to_string(),
        // Missing ToolResult here!
        r#"{"type":"ToolUse","tool":"Edit","args":{"file":"src/main.rs"}}"#.to_string(),
    ];

    let gaps = detect_gaps(&events)?;

    // Should detect the missing ToolResult
    assert!(gaps.len() > 0);
    assert!(gaps[0].description.contains("Missing ToolResult"));

    Ok(())
}

#[test]
fn test_compare_to_diff_suspicious_too_few_events() -> Result<()> {
    let events = vec![r#"{"type":"ToolUse","tool":"Edit","args":{}}"#.to_string()];
    let diff = "- old line\n+ new line\n+ another line\n+ third line\n";

    let result = compare_to_diff(&events, diff)?;

    // Should flag as suspicious (too few events for non-trivial change)
    assert!(result.suspicious_patterns.len() > 0);

    Ok(())
}

#[test]
fn test_compare_to_diff_with_gaps() -> Result<()> {
    let events = vec![
        r#"{"type":"ToolUse","tool":"Read","args":{}}"#.to_string(),
        // Missing result
        r#"{"type":"ToolUse","tool":"Edit","args":{}}"#.to_string(),
    ];
    let diff = "- old\n+ new\n";

    let result = compare_to_diff(&events, diff)?;

    // Should have detected gaps
    assert!(result.gaps.len() > 0);

    Ok(())
}

#[test]
fn test_verification_status_serialization() {
    use serde_json;

    // Test all verification status variants can be serialized
    let valid = VerificationStatus::Valid;
    assert!(serde_json::to_string(&valid).is_ok());

    let warning = VerificationStatus::Warning("Test warning".to_string());
    assert!(serde_json::to_string(&warning).is_ok());

    let invalid = VerificationStatus::Invalid("Test error".to_string());
    assert!(serde_json::to_string(&invalid).is_ok());

    let na = VerificationStatus::NotApplicable;
    assert!(serde_json::to_string(&na).is_ok());
}

#[test]
fn test_hash_calculation_with_special_characters() -> Result<()> {
    let events = vec![
        r#"{"content":"Special chars: \"quoted\" and 'single' and \n newline"}"#.to_string(),
        r#"{"content":"Unicode: 你好 🌟"}"#.to_string(),
    ];

    let hash = calculate_trajectory_hash(&events)?;

    // Should handle special characters without errors
    assert_eq!(hash.len(), 64);

    // Should be reproducible
    let hash2 = calculate_trajectory_hash(&events)?;
    assert_eq!(hash, hash2);

    Ok(())
}

#[test]
fn test_hash_calculation_large_trajectory() -> Result<()> {
    // Create a large trajectory (1000 events)
    let mut events = Vec::new();
    for i in 0..1000 {
        events.push(format!(r#"{{"event":{}}}"#, i));
    }

    let hash = calculate_trajectory_hash(&events)?;

    // Should handle large trajectories
    assert_eq!(hash.len(), 64);

    Ok(())
}

#[test]
fn test_verification_with_real_world_trajectory_pattern() -> Result<()> {
    // Simulate a real agent workflow
    let events = vec![
        // 1. Read the file
        r#"{"type":"ToolUse","tool":"Read","file":"src/lib.rs","line":1}"#.to_string(),
        r#"{"type":"ToolResult","content":"pub fn add(a: i32, b: i32) -> i32 { a + b }"}"#.to_string(),

        // 2. Think about the change
        r#"{"type":"Message","content":"I need to add multiplication function"}"#.to_string(),

        // 3. Make the edit
        r#"{"type":"ToolUse","tool":"Edit","file":"src/lib.rs","old":"pub fn add","new":"pub fn multiply(a: i32, b: i32) -> i32 { a * b }\n\npub fn add"}"#.to_string(),
        r#"{"type":"ToolResult","status":"success"}"#.to_string(),

        // 4. Test it
        r#"{"type":"ToolUse","tool":"Bash","command":"cargo test"}"#.to_string(),
        r#"{"type":"ToolResult","stdout":"test result: ok. 2 passed"}"#.to_string(),
    ];

    let hash = calculate_trajectory_hash(&events)?;

    // Verification should pass
    assert!(verify_trajectory_hash(&events, &hash)?);

    // Should have no gaps
    let gaps = detect_gaps(&events)?;
    assert_eq!(gaps.len(), 0);

    Ok(())
}

#[test]
fn test_tampered_trajectory_detection_subtly_modified() -> Result<()> {
    let events = sample_trajectory_events();
    let hash = calculate_trajectory_hash(&events)?;

    // Make a very subtle change (add one space)
    let mut tampered = events.clone();
    tampered[0] = tampered[0].clone() + " ";

    // Should still detect the tampering
    assert!(!verify_trajectory_hash(&tampered, &hash)?);

    Ok(())
}

#[test]
fn test_reordered_events_detected() -> Result<()> {
    let mut events = sample_trajectory_events();
    let hash = calculate_trajectory_hash(&events)?;

    // Swap two events (maintaining structure but changing order)
    events.swap(0, 2);

    // Should detect reordering
    assert!(!verify_trajectory_hash(&events, &hash)?);

    Ok(())
}
