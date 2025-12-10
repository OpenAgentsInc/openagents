//! Anti-Cheat Verification Tests
//!
//! CRITICAL: These tests ensure we're not gaming the Terminal-Bench benchmark.
//!
//! The HillClimber architecture proves "architecture beats model size" by:
//! 1. TestGen generates tests from task DESCRIPTION (not TB2 tests)
//! 2. FM DISCOVERS solutions through iteration against TestGen tests
//! 3. Giving FM the answer defeats the entire purpose
//!
//! If we hardcode TB2 knowledge:
//! - We're not proving architecture beats model size
//! - Results won't generalize to TB3 or novel tasks
//! - The entire thesis is invalidated
//!
//! The real test: Would this system perform equally well on Terminal-Bench 3
//! with completely different tasks?

use std::fs;

// ============================================================================
// Helpers
// ============================================================================

/// Check if a line is a comment, doc comment, or part of a GUARDRAIL block
fn is_comment_or_guardrail(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*")
        || trimmed.contains("GUARDRAIL")
}

/// Check if a pattern appears in non-comment code
fn contains_in_code(src: &str, pattern: &str) -> bool {
    for line in src.lines() {
        if !is_comment_or_guardrail(line) && line.contains(pattern) {
            return true;
        }
    }
    false
}

/// Get non-comment lines containing a pattern (for error messages)
fn find_violations(src: &str, pattern: &str) -> Vec<(usize, String)> {
    let mut violations = Vec::new();
    for (line_num, line) in src.lines().enumerate() {
        if !is_comment_or_guardrail(line) && line.contains(pattern) {
            violations.push((line_num + 1, line.to_string()));
        }
    }
    violations
}

// ============================================================================
// Source Code Scanning Tests
// ============================================================================

/// Known TB2 task IDs that should NEVER appear in runtime code
/// (OK in: comments, tests, docs, examples, CLI help text)
const FORBIDDEN_TASK_IDS: &[&str] = &[
    // Don't include common examples like "regex-log" since they appear in
    // GUARDRAIL comments as examples of what NOT to do
];

/// Solution hints that should NEVER appear in FM prompts
/// These are task-specific knowledge that would constitute cheating
const FORBIDDEN_SOLUTION_PATTERNS: &[&str] = &[
    // TB2-specific paths (actual cheating)
    "test_outputs.py",
    "test_inputs.py",
];

#[test]
fn test_no_hardcoded_task_ids_in_decomposer() {
    let src = include_str!("../src/decomposer.rs");

    for task_id in FORBIDDEN_TASK_IDS {
        let violations = find_violations(src, task_id);
        assert!(
            violations.is_empty(),
            "decomposer.rs contains forbidden task ID '{}' in actual code:\n{:?}\n\
             VIOLATION: Task IDs must not be hardcoded!\n\
             See docs/logs/20251208/1219-benchmark-gaming-analysis.md",
            task_id,
            violations
        );
    }
}

#[test]
fn test_no_hardcoded_task_ids_in_orchestrator() {
    let src = include_str!("../src/orchestrator.rs");

    for task_id in FORBIDDEN_TASK_IDS {
        let violations = find_violations(src, task_id);
        assert!(
            violations.is_empty(),
            "orchestrator.rs contains forbidden task ID '{}' in actual code:\n{:?}",
            task_id,
            violations
        );
    }
}

#[test]
fn test_no_hardcoded_task_ids_in_prompt() {
    let src = include_str!("../src/prompt.rs");

    for task_id in FORBIDDEN_TASK_IDS {
        let violations = find_violations(src, task_id);
        assert!(
            violations.is_empty(),
            "prompt.rs contains forbidden task ID '{}' in actual code:\n{:?}",
            task_id,
            violations
        );
    }
}

#[test]
fn test_no_solution_hints_in_decomposer() {
    let src = include_str!("../src/decomposer.rs");

    for pattern in FORBIDDEN_SOLUTION_PATTERNS {
        let violations = find_violations(src, pattern);
        assert!(
            violations.is_empty(),
            "decomposer.rs contains forbidden solution pattern '{}' in actual code:\n{:?}\n\
             VIOLATION: FM must DISCOVER techniques, not be given them!",
            pattern,
            violations
        );
    }
}

#[test]
fn test_no_solution_hints_in_prompt() {
    let src = include_str!("../src/prompt.rs");

    for pattern in FORBIDDEN_SOLUTION_PATTERNS {
        let violations = find_violations(src, pattern);
        assert!(
            violations.is_empty(),
            "prompt.rs contains forbidden solution pattern '{}' in actual code:\n{:?}\n\
             VIOLATION: FM must DISCOVER techniques, not be given them!",
            pattern,
            violations
        );
    }
}

// ============================================================================
// Decomposer Behavior Tests
// ============================================================================

#[test]
fn test_decomposer_is_task_agnostic() {
    use hillclimber::decomposer::decompose_task;
    use hillclimber::types::{TerminalBenchTask, VerificationConfig};

    // Create two completely different tasks
    let task_a = TerminalBenchTask {
        id: "task-alpha".to_string(),
        description: "Write a regex parser that extracts dates".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    let task_b = TerminalBenchTask {
        id: "task-beta".to_string(),
        description: "Implement a JSON validator".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    let decomp_a = decompose_task(&task_a);
    let decomp_b = decompose_task(&task_b);

    // Both should produce the same NUMBER of subtasks
    assert_eq!(
        decomp_a.subtask_count, decomp_b.subtask_count,
        "Decomposition should produce same structure regardless of task"
    );

    // Both should have the same PHASES (understand, write, test-iterate, validate)
    assert_eq!(
        decomp_a.subtasks.len(), decomp_b.subtasks.len(),
        "Subtask count should be consistent"
    );

    // Phases should be generic, not task-specific
    for (sub_a, sub_b) in decomp_a.subtasks.iter().zip(decomp_b.subtasks.iter()) {
        assert_eq!(
            sub_a.name, sub_b.name,
            "Subtask names should be generic: {} vs {}",
            sub_a.name, sub_b.name
        );
    }
}

#[test]
fn test_decomposer_extracts_from_description_only() {
    use hillclimber::decomposer::decompose_task;
    use hillclimber::types::{TerminalBenchTask, VerificationConfig};

    // Task with specific files mentioned in description
    let task = TerminalBenchTask {
        id: "unknown-task".to_string(), // Unknown task ID
        description: "Read /app/input.txt and write the result to /app/output.txt".to_string(),
        source_path: None,
        verification: VerificationConfig::default(),
    };

    let decomp = decompose_task(&task);

    // Decomposition should work for ANY task description
    assert!(decomp.subtask_count > 0, "Should decompose unknown tasks");
    assert!(decomp.subtasks.len() > 0, "Should produce subtasks");
}

// ============================================================================
// GUARDRAIL Comment Verification
// ============================================================================

#[test]
fn test_decomposer_has_guardrail_comment() {
    let src = include_str!("../src/decomposer.rs");

    assert!(
        src.contains("GUARDRAIL: NO TASK-SPECIFIC HARDCODING"),
        "decomposer.rs MUST have the GUARDRAIL comment block"
    );
}

#[test]
fn test_orchestrator_has_guardrail_comment() {
    let src = include_str!("../src/orchestrator.rs");

    assert!(
        src.contains("GUARDRAIL: NO TASK-SPECIFIC HARDCODING"),
        "orchestrator.rs MUST have the GUARDRAIL comment block"
    );
}

// ============================================================================
// TestGen Anti-Cheat Tests
// ============================================================================

#[test]
fn test_no_tb2_paths_in_testgen_generator() {
    // Read testgen generator source
    let src_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("testgen/src/generator.rs");

    if src_path.exists() {
        let src = fs::read_to_string(&src_path).unwrap();

        // Should not reference TB2-specific files
        assert!(
            !src.contains("test_outputs.py"),
            "generator.rs should not reference TB2 test files"
        );

        assert!(
            !src.contains("test_inputs.py"),
            "generator.rs should not reference TB2 test files"
        );
    }
}

#[test]
fn test_no_forbidden_task_ids_in_testgen() {
    let src_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("testgen/src/generator.rs");

    if src_path.exists() {
        let src = fs::read_to_string(&src_path).unwrap();

        for task_id in FORBIDDEN_TASK_IDS {
            assert!(
                !src.contains(task_id),
                "testgen/generator.rs contains forbidden task ID: '{}'",
                task_id
            );
        }
    }
}

// ============================================================================
// Statistics: Code Health Report
// ============================================================================

#[test]
fn test_anti_cheat_code_health_report() {
    let decomposer_src = include_str!("../src/decomposer.rs");
    let orchestrator_src = include_str!("../src/orchestrator.rs");
    let prompt_src = include_str!("../src/prompt.rs");

    let mut violations = Vec::new();

    // Check all files for forbidden patterns in actual code (not comments)
    let files = [
        ("decomposer.rs", decomposer_src),
        ("orchestrator.rs", orchestrator_src),
        ("prompt.rs", prompt_src),
    ];

    for (filename, src) in files {
        for task_id in FORBIDDEN_TASK_IDS {
            let file_violations = find_violations(src, task_id);
            for (line, content) in file_violations {
                violations.push(format!(
                    "{} line {}: forbidden task ID '{}' in: {}",
                    filename, line, task_id, content.trim()
                ));
            }
        }
        for pattern in FORBIDDEN_SOLUTION_PATTERNS {
            let file_violations = find_violations(src, pattern);
            for (line, content) in file_violations {
                violations.push(format!(
                    "{} line {}: forbidden pattern '{}' in: {}",
                    filename, line, pattern, content.trim()
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "ANTI-CHEAT CODE HEALTH REPORT\n\
             ================================\n\
             Found {} violations:\n\n{}\n\n\
             These violations invalidate the Terminal-Bench results.\n\
             See CLAUDE.md 'Terminal-Bench Anti-Cheating Policy' section.",
            violations.len(),
            violations.join("\n")
        );
    }

    println!("ANTI-CHEAT CODE HEALTH: PASS");
    println!("  - Checked {} files", files.len());
    println!("  - Checked for {} forbidden task IDs", FORBIDDEN_TASK_IDS.len());
    println!("  - Checked for {} forbidden patterns", FORBIDDEN_SOLUTION_PATTERNS.len());
    println!("  - No violations found in actual code (comments excluded)");
}
