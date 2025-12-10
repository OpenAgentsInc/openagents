//! HillClimber Task Decomposer Module
//!
//! Breaks complex tasks into subtasks with verification checkpoints.
//! Uses ONLY general-purpose decomposition rules - no task-specific hardcoding.
//!
//! Part of the MAP (Modular Agentic Planner) architecture.

use crate::types::{Subtask, TaskDecomposition, TerminalBenchTask};
use regex::Regex;

// ============================================================================
// GUARDRAIL: NO TASK-SPECIFIC HARDCODING
//
// This file must NEVER contain:
// - Task IDs (e.g., "regex-log", "path-tracing")
// - Task-specific patterns (e.g., IPv4 format, date format)
// - Task-specific hints (e.g., "use lookahead for IPv4")
// - Task-specific file paths (e.g., "/app/regex.txt")
//
// All knowledge must come from:
// 1. The task description (passed as parameter)
// 2. General process knowledge (TDD, iteration)
//
// If you're tempted to add task-specific code, you're defeating the thesis:
// "Architecture beats model size"
// ============================================================================

// ============================================================================
// Helper Functions: Extract Information from Task Description
// ============================================================================

/// Extract files mentioned in the task description.
/// Looks for patterns like /app/filename or /app/path/to/file
fn extract_files_to_read(description: &str) -> Vec<String> {
    let re = Regex::new(r"/app/[\w\-\./]+").unwrap();
    let matches: Vec<String> = re
        .find_iter(description)
        .map(|m| m.as_str().to_string())
        .collect();

    // Deduplicate
    let mut unique: Vec<String> = Vec::new();
    for m in matches {
        if !unique.contains(&m) {
            unique.push(m);
        }
    }
    unique
}

/// Extract required output files from task description.
/// Looks for patterns like "write to /app/X" or "output file: /app/X"
fn extract_required_outputs(description: &str) -> Vec<String> {
    let re = Regex::new(r"(?i)(?:write|output|create|save).*?(/app/[\w\-\.]+)").unwrap();
    re.captures_iter(description)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

// ============================================================================
// Main Decomposer
// ============================================================================

/// Decompose a Terminal-Bench task into subtasks.
///
/// This is a GENERAL-PURPOSE decomposer that works for ANY task.
/// It uses ONLY the task description to generate subtasks.
///
/// # Arguments
///
/// * `task` - Terminal-Bench task
///
/// # Returns
///
/// Task decomposition with generic subtasks
pub fn decompose_task(task: &TerminalBenchTask) -> TaskDecomposition {
    // Extract information from task description
    let files_to_read = extract_files_to_read(&task.description);
    let required_outputs = extract_required_outputs(&task.description);

    // Generate GENERIC subtasks that work for any task
    TaskDecomposition {
        task_id: task.id.clone(),
        subtask_count: 4,
        subtasks: vec![
            Subtask {
                id: 0,
                name: "understand-requirements".to_string(),
                goal: "Read the task description carefully. Identify: (1) required output files, (2) success criteria, (3) any constraints mentioned.".to_string(),
                checkpoint: "Task requirements are understood".to_string(),
                expected_artifacts: vec![],
                depends_on: vec![],
                hints: vec![
                    "Use read_file to examine any example files mentioned".to_string(),
                    "Note the exact output format required".to_string(),
                ],
                max_turns: 3,
            },
            Subtask {
                id: 1,
                name: "write-initial-solution".to_string(),
                goal: "Write an initial solution based on your understanding of the requirements.".to_string(),
                checkpoint: "Initial solution file exists".to_string(),
                expected_artifacts: required_outputs.clone(),
                depends_on: vec![0],
                hints: vec![
                    "Start simple - get something working first".to_string(),
                    "Use write_file to create the required output".to_string(),
                ],
                max_turns: 5,
            },
            Subtask {
                id: 2,
                name: "test-and-iterate".to_string(),
                goal: "Run verify_progress to see test results. Analyze failures and fix issues.".to_string(),
                checkpoint: "At least 50% of test cases passing".to_string(),
                expected_artifacts: required_outputs.clone(),
                depends_on: vec![1],
                hints: vec![
                    "Read the failure messages carefully".to_string(),
                    "Make ONE targeted change per iteration".to_string(),
                    "False positives: tighten constraints".to_string(),
                    "False negatives: loosen constraints".to_string(),
                ],
                max_turns: 10,
            },
            Subtask {
                id: 3,
                name: "final-validation".to_string(),
                goal: "Ensure all tests pass. Fix any remaining edge cases.".to_string(),
                checkpoint: "100% test cases passing".to_string(),
                expected_artifacts: required_outputs.clone(),
                depends_on: vec![2],
                hints: vec![
                    "Check boundary conditions".to_string(),
                    "Test edge cases mentioned in failures".to_string(),
                ],
                max_turns: 5,
            },
        ],
        global_hints: vec![
            // ONLY general process knowledge
            "Use verify_progress after each change to get feedback".to_string(),
            "Read failure messages to understand what's wrong".to_string(),
            "Iterate until all tests pass".to_string(),
        ],
        files_to_read,
        required_outputs,
    }
}

/// Get the current subtask based on execution state.
///
/// # Arguments
///
/// * `decomposition` - Task decomposition
/// * `completed_subtasks` - IDs of completed subtasks
///
/// # Returns
///
/// Current subtask to work on, or None if all complete
pub fn get_current_subtask<'a>(
    decomposition: &'a TaskDecomposition,
    completed_subtasks: &[u32],
) -> Option<&'a Subtask> {
    for subtask in &decomposition.subtasks {
        // Check if already completed
        if completed_subtasks.contains(&subtask.id) {
            continue;
        }

        // Check if dependencies are met
        let deps_complete = subtask
            .depends_on
            .iter()
            .all(|dep| completed_subtasks.contains(dep));

        if deps_complete {
            return Some(subtask);
        }
    }

    None
}

/// Build a prompt section for the current subtask.
///
/// # Arguments
///
/// * `decomposition` - Task decomposition
/// * `subtask` - Current subtask
/// * `previous_feedback` - Optional feedback from previous attempt
///
/// # Returns
///
/// Formatted prompt string
pub fn build_subtask_prompt(
    decomposition: &TaskDecomposition,
    subtask: &Subtask,
    previous_feedback: Option<&str>,
) -> String {
    let mut lines = Vec::new();

    lines.push(format!(
        "## Current Subtask: {} ({}/{})",
        subtask.name,
        subtask.id + 1,
        decomposition.subtask_count
    ));
    lines.push(String::new());
    lines.push(format!("**Goal:** {}", subtask.goal));
    lines.push(String::new());
    lines.push(format!("**Checkpoint:** {}", subtask.checkpoint));
    lines.push(String::new());

    if !subtask.hints.is_empty() {
        lines.push("**Hints:**".to_string());
        for hint in &subtask.hints {
            lines.push(format!("- {}", hint));
        }
        lines.push(String::new());
    }

    if !subtask.expected_artifacts.is_empty() {
        lines.push(format!(
            "**Expected outputs:** {}",
            subtask.expected_artifacts.join(", ")
        ));
        lines.push(String::new());
    }

    if let Some(feedback) = previous_feedback {
        lines.push("**Previous attempt feedback:**".to_string());
        lines.push(feedback.to_string());
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Check if a subtask is complete based on evaluation results.
///
/// # Arguments
///
/// * `subtask` - The subtask to check
/// * `progress` - Current progress (0.0 - 1.0)
/// * `artifacts` - List of existing artifact paths
///
/// # Returns
///
/// `true` if the subtask is complete
pub fn is_subtask_complete(subtask: &Subtask, progress: f64, artifacts: &[String]) -> bool {
    // Check if all expected artifacts exist
    let has_all_artifacts = subtask.expected_artifacts.iter().all(|expected| {
        artifacts
            .iter()
            .any(|artifact| artifact.ends_with(expected) || artifact == expected)
    });

    if !has_all_artifacts && !subtask.expected_artifacts.is_empty() {
        return false;
    }

    // For final subtask (final-validation), require 100% progress
    if subtask.name == "final-validation" && progress < 1.0 {
        return false;
    }

    // For test-and-iterate subtask, need at least 50% progress
    if subtask.name == "test-and-iterate" && progress < 0.5 {
        return false;
    }

    true
}

/// Create a fallback decomposition for unknown tasks.
pub fn create_fallback_decomposition(task: &TerminalBenchTask) -> TaskDecomposition {
    let description_preview: String = task.description.chars().take(200).collect();

    TaskDecomposition {
        task_id: task.id.clone(),
        subtask_count: 3,
        subtasks: vec![
            Subtask {
                id: 0,
                name: "understand".to_string(),
                goal: "Read and understand the task requirements".to_string(),
                checkpoint: "Task requirements are clear".to_string(),
                expected_artifacts: vec![],
                depends_on: vec![],
                hints: vec![
                    "Read the task description carefully".to_string(),
                    "Identify input and output files".to_string(),
                ],
                max_turns: 3,
            },
            Subtask {
                id: 1,
                name: "implement".to_string(),
                goal: "Implement the solution".to_string(),
                checkpoint: "Solution file exists".to_string(),
                expected_artifacts: vec![],
                depends_on: vec![0],
                hints: vec![
                    "Write the solution code".to_string(),
                    "Use verify_progress to check progress".to_string(),
                ],
                max_turns: 15,
            },
            Subtask {
                id: 2,
                name: "verify".to_string(),
                goal: "Verify the solution passes all tests".to_string(),
                checkpoint: "All tests pass".to_string(),
                expected_artifacts: vec![],
                depends_on: vec![1],
                hints: vec![
                    "Run verification".to_string(),
                    "Fix any remaining issues".to_string(),
                ],
                max_turns: 10,
            },
        ],
        global_hints: vec![description_preview],
        files_to_read: vec![],
        required_outputs: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VerificationConfig;

    fn create_test_task(description: &str) -> TerminalBenchTask {
        TerminalBenchTask {
            id: "test-task".to_string(),
            description: description.to_string(),
            source_path: None,
            verification: VerificationConfig::default(),
        }
    }

    #[test]
    fn test_extract_files_to_read() {
        let desc = "Read the log file at /app/server.log and write output to /app/result.txt";
        let files = extract_files_to_read(desc);
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"/app/server.log".to_string()));
        assert!(files.contains(&"/app/result.txt".to_string()));
    }

    #[test]
    fn test_extract_required_outputs() {
        let desc = "Write the regex pattern to /app/regex.txt that matches valid entries";
        let outputs = extract_required_outputs(desc);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0], "/app/regex.txt");
    }

    #[test]
    fn test_decompose_task() {
        let task = create_test_task("Write a regex to /app/regex.txt that matches dates");
        let decomp = decompose_task(&task);

        assert_eq!(decomp.task_id, "test-task");
        assert_eq!(decomp.subtask_count, 4);
        assert_eq!(decomp.subtasks.len(), 4);
        assert!(decomp.required_outputs.contains(&"/app/regex.txt".to_string()));
    }

    #[test]
    fn test_get_current_subtask() {
        let task = create_test_task("Test task");
        let decomp = decompose_task(&task);

        // Nothing completed - should get first subtask
        let current = get_current_subtask(&decomp, &[]);
        assert!(current.is_some());
        assert_eq!(current.unwrap().id, 0);

        // First completed - should get second
        let current = get_current_subtask(&decomp, &[0]);
        assert!(current.is_some());
        assert_eq!(current.unwrap().id, 1);

        // All completed - should get None
        let current = get_current_subtask(&decomp, &[0, 1, 2, 3]);
        assert!(current.is_none());
    }

    #[test]
    fn test_is_subtask_complete() {
        // Test-and-iterate subtask needs 50% progress
        let subtask = Subtask {
            id: 2,
            name: "test-and-iterate".to_string(),
            goal: "Test".to_string(),
            checkpoint: "At least 50% of test cases passing".to_string(),
            expected_artifacts: vec!["/app/solution.txt".to_string()],
            depends_on: vec![1],
            hints: vec![],
            max_turns: 10,
        };

        // No artifacts, no progress - not complete
        assert!(!is_subtask_complete(&subtask, 0.0, &[]));

        // Has artifact but low progress - not complete (needs 50%)
        assert!(!is_subtask_complete(
            &subtask,
            0.3,
            &["/app/solution.txt".to_string()]
        ));

        // Has artifact and enough progress - complete
        assert!(is_subtask_complete(
            &subtask,
            0.6,
            &["/app/solution.txt".to_string()]
        ));

        // Final validation subtask needs 100% progress
        let final_subtask = Subtask {
            id: 3,
            name: "final-validation".to_string(),
            goal: "Ensure all tests pass".to_string(),
            checkpoint: "100% test cases passing".to_string(),
            expected_artifacts: vec![],
            depends_on: vec![2],
            hints: vec![],
            max_turns: 5,
        };

        // 60% is not enough for final-validation
        assert!(!is_subtask_complete(&final_subtask, 0.6, &[]));

        // 100% is good
        assert!(is_subtask_complete(&final_subtask, 1.0, &[]));
    }
}
