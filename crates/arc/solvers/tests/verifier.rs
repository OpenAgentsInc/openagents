use arc_core::{
    ArcAction, ArcExample, ArcGrid, ArcTask, ArcTaskId, TraceLocator, canonicalize_task,
};
use arc_solvers::{
    ArcCommonVerifier, ArcGridExpr, ArcProgram, ArcSymbol, CandidateIdentity, CandidateVerifier,
    FalsificationCheckKind, FalsificationCheckStatus, Hypothesis, HypothesisKind,
    PlannedActionStep, SolverLaneId, SolverRefusalCode, TaskBudget,
};

fn symbol(name: &str) -> ArcSymbol {
    ArcSymbol::new(name).expect("symbol should validate")
}

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

fn large_budget() -> TaskBudget {
    TaskBudget {
        max_wall_ms: 10_000,
        max_candidates: 8,
        max_verifier_evals: 1,
        max_train_pair_execs: 32,
        max_refinement_steps: 0,
        max_model_forward_calls: 0,
        max_ttt_updates: 0,
        max_memory_mb: 64,
    }
}

fn identity_hypothesis() -> Hypothesis {
    let program = ArcProgram::new(symbol("input"), ArcGridExpr::Input);
    Hypothesis::new(
        HypothesisKind::StaticProgram,
        SolverLaneId::new("symbolic").expect("lane id"),
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticProgram,
            Some(&program),
            None,
            None,
            None,
        )
        .expect("candidate identity"),
        Some(program),
        None,
        None,
        0.95,
        TraceLocator::new("trace://arc-solvers/tests/identity-hypothesis").expect("trace locator"),
        Default::default(),
    )
    .expect("hypothesis should validate")
}

#[test]
fn verifier_passes_identity_program_and_runs_both_falsifiers() {
    let raw_task = task(
        "verifier-identity",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                grid(2, 2, &[&[1, 0], &[0, 1]]),
            ),
            (
                grid(3, 2, &[&[0, 2, 0], &[2, 2, 0]]),
                grid(3, 2, &[&[0, 2, 0], &[2, 2, 0]]),
            ),
            (
                grid(2, 3, &[&[3, 0], &[3, 3], &[0, 3]]),
                grid(2, 3, &[&[3, 0], &[3, 3], &[0, 3]]),
            ),
        ],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let report = ArcCommonVerifier::default()
        .evaluate(&task, &identity_hypothesis(), large_budget())
        .expect("verification should succeed");

    assert!(report.verifier_pass);
    assert!(report.exact_fit);
    assert_eq!(report.pair_results.len(), 3);
    assert!(report.refusal.is_none());
    assert_eq!(report.budget_delta.verifier_evals, 1);

    let augmentation = report
        .falsification_checks
        .iter()
        .find(|check| check.kind == FalsificationCheckKind::AugmentationStability)
        .expect("augmentation check should exist");
    assert_eq!(augmentation.status, FalsificationCheckStatus::Passed);
    assert_eq!(augmentation.score, Some(1.0));

    let holdout = report
        .falsification_checks
        .iter()
        .find(|check| check.kind == FalsificationCheckKind::HoldoutOnTrain)
        .expect("holdout check should exist");
    assert_eq!(holdout.status, FalsificationCheckStatus::Passed);
    assert_eq!(holdout.score, Some(1.0));
}

#[test]
fn verifier_flags_spurious_direct_grid_candidates() {
    let raw_task = task(
        "verifier-static-answer",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                grid(2, 2, &[&[1, 1], &[0, 0]]),
            ),
            (
                grid(2, 2, &[&[0, 2], &[2, 0]]),
                grid(2, 2, &[&[0, 2], &[2, 2]]),
            ),
            (
                grid(2, 2, &[&[3, 3], &[0, 0]]),
                grid(2, 2, &[&[3, 0], &[0, 3]]),
            ),
        ],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let answer = task.normalized_train[0].output.grid.clone();
    let hypothesis = Hypothesis::new(
        HypothesisKind::StaticAnswer,
        SolverLaneId::new("direct-grid").expect("lane id"),
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticAnswer,
            None,
            Some(&answer),
            None,
            None,
        )
        .expect("candidate identity"),
        None,
        Some(answer),
        None,
        0.4,
        TraceLocator::new("trace://arc-solvers/tests/static-answer-hypothesis")
            .expect("trace locator"),
        Default::default(),
    )
    .expect("hypothesis should validate");

    let report = ArcCommonVerifier::default()
        .evaluate(&task, &hypothesis, large_budget())
        .expect("verification should succeed");

    assert!(!report.verifier_pass);
    assert!(!report.exact_fit);
    assert!(report.spuriousness_risk > 0.0);
    assert!(report.pair_results.iter().any(|pair| !pair.exact_match));

    let holdout = report
        .falsification_checks
        .iter()
        .find(|check| check.kind == FalsificationCheckKind::HoldoutOnTrain)
        .expect("holdout check should exist");
    assert_eq!(holdout.status, FalsificationCheckStatus::Failed);
    assert!(holdout.score.expect("holdout score") < 1.0);
}

#[test]
fn verifier_skips_optional_falsifiers_when_budget_is_too_small() {
    let raw_task = task(
        "verifier-budget",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                grid(2, 2, &[&[1, 0], &[0, 1]]),
            ),
            (
                grid(2, 2, &[&[0, 2], &[2, 0]]),
                grid(2, 2, &[&[0, 2], &[2, 0]]),
            ),
            (
                grid(2, 2, &[&[3, 3], &[0, 0]]),
                grid(2, 2, &[&[3, 3], &[0, 0]]),
            ),
        ],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let budget = TaskBudget {
        max_train_pair_execs: 3,
        ..large_budget()
    };
    let report = ArcCommonVerifier::default()
        .evaluate(&task, &identity_hypothesis(), budget)
        .expect("verification should succeed");

    assert!(report.verifier_pass);
    for check in &report.falsification_checks {
        assert_eq!(check.status, FalsificationCheckStatus::Skipped);
        assert!(check.detail.contains("skipped"));
    }
}

#[test]
fn verifier_refuses_interactive_plan_hypotheses_in_static_mode() {
    let raw_task = task(
        "verifier-interactive",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 0], &[0, 1]]),
        )],
        grid(1, 1, &[&[0]]),
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let steps = vec![PlannedActionStep {
        action: ArcAction::Action1,
        expected_state: None,
        expected_level_index: Some(0),
        reset_marker: false,
    }];
    let hypothesis = Hypothesis::new(
        HypothesisKind::InteractivePlan,
        SolverLaneId::new("interactive").expect("lane id"),
        0,
        CandidateIdentity::new(
            HypothesisKind::InteractivePlan,
            None,
            None,
            Some(&steps),
            None,
        )
        .expect("candidate identity"),
        None,
        None,
        Some(steps),
        0.2,
        TraceLocator::new("trace://arc-solvers/tests/interactive-hypothesis")
            .expect("trace locator"),
        Default::default(),
    )
    .expect("hypothesis should validate");

    let report = ArcCommonVerifier::default()
        .evaluate(&task, &hypothesis, large_budget())
        .expect("verification should succeed");

    let refusal = report
        .refusal
        .expect("interactive verification should refuse");
    assert_eq!(refusal.code, SolverRefusalCode::UnsupportedTask);
    assert!(!report.verifier_pass);
}
