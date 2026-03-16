use arc_core::{ArcExample, ArcGrid, ArcTask, ArcTaskId, canonicalize_task};
use arc_solvers::{
    ArcCommonVerifier, ArcGridExpr, CandidateVerifier, FalsificationCheckKind, LaneBatchStatus,
    SYMBOLIC_LANE_ID, SolverRefusalCode, SymbolicLane, SymbolicRepairOperator, TaskBudget,
};

fn grid(width: u8, height: u8, rows: &[&[u8]]) -> ArcGrid {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    ArcGrid::new(width, height, cells).expect("grid should validate")
}

fn task(id: &str, pairs: &[(ArcGrid, ArcGrid)], test: ArcGrid) -> ArcTask {
    ArcTask::new(
        ArcTaskId::new(id).expect("task id"),
        pairs
            .iter()
            .cloned()
            .map(|(input, output)| ArcExample { input, output })
            .collect(),
        vec![test],
    )
    .expect("task should validate")
}

fn budget() -> TaskBudget {
    TaskBudget {
        max_wall_ms: 10_000,
        max_candidates: 16,
        max_verifier_evals: 4,
        max_train_pair_execs: 128,
        max_refinement_steps: 8,
        max_model_forward_calls: 0,
        max_ttt_updates: 0,
        max_memory_mb: 128,
    }
}

#[test]
fn symbolic_lane_solves_rotation_tasks_via_seed_search() {
    let raw_task = task(
        "symbolic-rotate",
        &[
            (
                grid(2, 3, &[&[1, 0], &[1, 1], &[0, 1]]),
                grid(3, 2, &[&[0, 1, 1], &[1, 1, 0]]),
            ),
            (
                grid(2, 2, &[&[2, 0], &[2, 2]]),
                grid(2, 2, &[&[2, 2], &[2, 0]]),
            ),
        ],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let run = SymbolicLane::default()
        .run(&task, budget())
        .expect("symbolic lane should run");

    assert_eq!(run.proposal_batch.lane_id.as_str(), SYMBOLIC_LANE_ID);
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Proposed);
    let best = run
        .best_hypothesis()
        .expect("symbolic lane should find a program");
    assert!(matches!(
        best.program.as_ref().expect("program").body,
        ArcGridExpr::RotateQuarterTurns {
            quarter_turns: 1,
            ..
        }
    ));

    let report = ArcCommonVerifier::default()
        .evaluate(&task, best, budget())
        .expect("verification should succeed");
    assert!(report.exact_fit);
    let augmentation = report
        .falsification_checks
        .iter()
        .find(|check| check.kind == FalsificationCheckKind::AugmentationStability)
        .expect("augmentation check should exist");
    assert!(augmentation.score.expect("score") < 1.0);
}

#[test]
fn symbolic_lane_uses_typed_recolor_repairs_for_near_miss_programs() {
    let raw_task = task(
        "symbolic-recolor",
        &[
            (
                grid(2, 2, &[&[2, 0], &[0, 2]]),
                grid(2, 2, &[&[3, 0], &[0, 3]]),
            ),
            (
                grid(3, 2, &[&[2, 2, 0], &[0, 2, 0]]),
                grid(3, 2, &[&[3, 3, 0], &[0, 3, 0]]),
            ),
        ],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let run = SymbolicLane::default()
        .run(&task, budget())
        .expect("symbolic lane should run");

    assert!(!run.repair_attempts.is_empty());
    assert!(run.repair_attempts.iter().any(|attempt| {
        attempt.operator == SymbolicRepairOperator::Recolor { from: 1, to: 2 }
            || attempt.operator == SymbolicRepairOperator::Recolor { from: 2, to: 3 }
            || attempt.operator == SymbolicRepairOperator::Recolor { from: 1, to: 3 }
    }));

    let best = run
        .best_hypothesis()
        .expect("repair should produce a candidate");
    assert!(matches!(
        best.program.as_ref().expect("program").body,
        ArcGridExpr::Recolor { .. }
    ));

    let report = ArcCommonVerifier::default()
        .evaluate(&task, best, budget())
        .expect("verification should succeed");
    assert!(report.verifier_pass);
}

#[test]
fn symbolic_lane_refuses_when_minimum_budget_is_missing() {
    let raw_task = task(
        "symbolic-budget",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 0], &[0, 1]]),
        )],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let budget = TaskBudget {
        max_candidates: 0,
        ..budget()
    };

    let run = SymbolicLane::default()
        .run(&task, budget)
        .expect("symbolic lane should return a refusal batch");

    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Refused);
    let refusal = run
        .proposal_batch
        .refusal
        .as_ref()
        .expect("refusal should exist");
    assert_eq!(refusal.code, SolverRefusalCode::MinimumBudgetNotMet);
    assert_eq!(run.refinement_batch.status, LaneBatchStatus::Empty);
}
