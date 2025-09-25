use openagents_lib::headless::{plan_with_fallback, run_once_headless};
use openagents_lib::{task_create, AutonomyBudget, TaskStatus, SubtaskStatus, next_pending_index};

use tempfile::TempDir;

#[test]
fn full_flow_headless_readonly() {
    // Isolate in a temp CODEX_HOME
    let td = TempDir::new().unwrap();
    std::env::set_var("CODEX_HOME", td.path());

    // Create a task with read-only sandbox and small budgets
    let t = task_create(
        "Readonly – Flow Test",
        AutonomyBudget { approvals: "never".into(), sandbox: "read-only".into(), max_turns: Some(2), max_tokens: Some(10_000), max_minutes: Some(1) }
    ).unwrap();

    // Plan with fallback
    let t = plan_with_fallback(&t.id, "List top-level files; Summarize crates").unwrap();
    assert!(matches!(t.status, TaskStatus::Planned));
    assert!(!t.queue.is_empty());

    // Run once; should mark first subtask done
    let t = run_once_headless(&t.id).unwrap();
    assert!(matches!(t.status, TaskStatus::Planned | TaskStatus::Running | TaskStatus::Completed));
    assert!(matches!(t.queue[0].status, SubtaskStatus::Done));
    // Metrics should have advanced
    assert!(t.metrics.turns >= 1);

    // Run until complete or until 4 steps to keep the test short
    let mut tcur = t;
    for _ in 0..4 {
        if next_pending_index(&tcur).is_none() { break; }
        tcur = run_once_headless(&tcur.id).unwrap();
    }
    // At least first two subtasks should be completed after up to 5 total runs (1 + 4)
    assert!(tcur.queue.iter().take(2).all(|s| matches!(s.status, SubtaskStatus::Done)));
}
