//! Pre-built message sequences for common test scenarios
//!
//! These sequences mirror the message patterns from the TypeScript E2E tests,
//! providing complete workflows that can be injected into components.

use crate::messages::factories::*;
use crate::protocol::*;

/// Create a complete Golden Loop message sequence
///
/// This mirrors the E1-E5 test scenarios from golden-loop.spec.ts:
/// - Session start
/// - Task selection
/// - Task decomposition into subtasks
/// - Subtask execution (2 subtasks)
/// - Verification
/// - Commit and push
/// - Session complete
pub fn golden_loop_sequence(task_id: Option<&str>) -> Vec<HudMessage> {
    let task_id_str = task_id
        .map(String::from)
        .unwrap_or_else(|| format!("oa-golden-{}", uuid::Uuid::new_v4().simple()));

    let task = task_info(Some(&task_id_str), &format!("Golden Loop Task {}", task_id_str));

    let subtask1 = subtask_info(
        Some(&format!("{}-sub-001", task_id_str)),
        "Implement feature",
    );
    let subtask2 = subtask_info(Some(&format!("{}-sub-002", task_id_str)), "Add tests");

    vec![
        // Session start
        session_start(Some(&format!("session-{}", task_id_str))),
        // Task selection
        task_selected(task.clone()),
        // Task decomposition
        task_decomposed(vec![subtask1.clone(), subtask2.clone()]),
        // First subtask
        subtask_start(subtask1.clone()),
        subtask_complete(
            subtask1.clone(),
            Some(subagent_result_success(vec!["src/feature.rs"], 3)),
        ),
        // Second subtask
        subtask_start(subtask2.clone()),
        subtask_complete(
            subtask2.clone(),
            Some(subagent_result_success(vec!["src/feature_test.rs"], 2)),
        ),
        // Verification
        verification_start("cargo test"),
        verification_complete("cargo test", true, Some("42 tests passed")),
        // Commit and push
        commit_created("abc123def456", &format!("{}: Implement feature", task_id_str)),
        push_complete("main"),
        // Session complete
        session_complete(true, "Task completed successfully"),
    ]
}

/// Create an APM progress sequence (simulates increasing activity)
///
/// Useful for testing APM widget updates and rendering.
pub fn apm_progress_sequence(session_id: &str) -> Vec<HudMessage> {
    vec![
        apm_update_for_session(session_id, 5.0, 5),
        apm_update_for_session(session_id, 10.0, 15),
        apm_update_for_session(session_id, 15.0, 30),
        apm_update_for_session(session_id, 20.0, 50),
        apm_update_for_session(session_id, 25.0, 75),
        apm_update_for_session(session_id, 30.0, 100),
    ]
}

/// Create an error recovery sequence
///
/// Simulates a session that encounters an error but recovers.
pub fn error_recovery_sequence() -> Vec<HudMessage> {
    let task = task_info(None, "Error Recovery Task");
    let subtask = subtask_info(None, "Subtask that fails initially");

    vec![
        session_start(None),
        task_selected(task),
        task_decomposed(vec![subtask.clone()]),
        subtask_start(subtask.clone()),
        // First attempt fails
        error(OrchestratorPhase::Executing, "First attempt failed"),
        // Retry and succeed
        subtask_start(subtask.clone()),
        subtask_complete(subtask, None),
        verification_start("cargo test"),
        verification_complete("cargo test", true, None),
        session_complete(true, "Recovered and completed"),
    ]
}

/// Create a verification failure sequence
///
/// Simulates a session where verification fails.
pub fn verification_failure_sequence() -> Vec<HudMessage> {
    let task = task_info(None, "Verification Failure Task");
    let subtask = subtask_info(None, "Implementation subtask");

    vec![
        session_start(None),
        task_selected(task),
        task_decomposed(vec![subtask.clone()]),
        subtask_start(subtask.clone()),
        subtask_complete(subtask, None),
        verification_start("cargo test"),
        verification_complete(
            "cargo test",
            false,
            Some("3 tests failed:\n- test_feature_1\n- test_feature_2\n- test_edge_case"),
        ),
        error(OrchestratorPhase::Verifying, "Verification failed: 3 tests failing"),
        session_complete(false, "Session failed due to verification errors"),
    ]
}

/// Create a rapid message burst sequence
///
/// Useful for stress testing message handling.
pub fn rapid_burst_sequence(count: usize) -> Vec<HudMessage> {
    (0..count)
        .map(|i| apm_update((i + 1) as f64, (i + 1) as u64))
        .collect()
}

/// Create a sequence with malformed-like messages
///
/// These are valid Rust types but represent edge cases.
pub fn edge_case_sequence() -> Vec<HudMessage> {
    vec![
        // Empty strings
        session_start(Some("")),
        task_selected(task_info(Some(""), "")),
        // Very long strings
        text_output(&"x".repeat(10000), None),
        // Edge case APM values
        apm_update(0.0, 0),
        apm_update(f64::MAX, u64::MAX),
        // Special characters
        error(OrchestratorPhase::Failed, "Error with \"quotes\" and\nnewlines\tand\ttabs"),
    ]
}

/// Create a complete session with multiple subtasks
pub fn multi_subtask_sequence(subtask_count: usize) -> Vec<HudMessage> {
    let task = task_info(None, "Multi-subtask Task");

    let subtasks: Vec<HudSubtaskInfo> = (0..subtask_count)
        .map(|i| subtask_info(None, &format!("Subtask {}", i + 1)))
        .collect();

    let mut messages = vec![
        session_start(None),
        task_selected(task),
        task_decomposed(subtasks.clone()),
    ];

    // Add start/complete for each subtask
    for subtask in subtasks {
        messages.push(subtask_start(subtask.clone()));
        messages.push(subtask_complete(subtask, None));
    }

    messages.push(verification_start("cargo test"));
    messages.push(verification_complete("cargo test", true, None));
    messages.push(session_complete(true, "All subtasks completed"));

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_loop_sequence_length() {
        let seq = golden_loop_sequence(None);
        assert_eq!(seq.len(), 12);
    }

    #[test]
    fn test_golden_loop_sequence_starts_with_session() {
        let seq = golden_loop_sequence(None);
        assert!(matches!(seq[0], HudMessage::SessionStart { .. }));
    }

    #[test]
    fn test_golden_loop_sequence_ends_with_complete() {
        let seq = golden_loop_sequence(None);
        assert!(matches!(
            seq.last().unwrap(),
            HudMessage::SessionComplete { success: true, .. }
        ));
    }

    #[test]
    fn test_apm_progress_sequence_increases() {
        let seq = apm_progress_sequence("test");
        for (i, msg) in seq.iter().enumerate() {
            if let HudMessage::ApmUpdate { session_apm, .. } = msg {
                assert_eq!(*session_apm, (i + 1) as f64 * 5.0);
            }
        }
    }

    #[test]
    fn test_multi_subtask_sequence() {
        let seq = multi_subtask_sequence(3);
        // session_start + task_selected + decomposed + (start+complete)*3 + verification_start + verification_complete + session_complete
        // = 1 + 1 + 1 + 6 + 1 + 1 + 1 = 12
        assert_eq!(seq.len(), 12);
    }

    #[test]
    fn test_rapid_burst_sequence() {
        let seq = rapid_burst_sequence(100);
        assert_eq!(seq.len(), 100);
    }
}
