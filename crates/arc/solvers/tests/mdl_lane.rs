use arc_core::{ArcExample, ArcGrid, ArcTask, ArcTaskId, canonicalize_task};
use arc_solvers::{
    ArcCommonVerifier, ArcMdlInitializationMode, ArcMdlLane, ArcMdlLaneConfig,
    ArcMdlRepresentation, CandidateVerifier, LaneBatchStatus, SolverRefusalCode, TaskBudget,
};

fn grid(width: u8, height: u8, rows: &[&[u8]]) -> ArcGrid {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    ArcGrid::new(width, height, cells).expect("grid should validate")
}

fn task(id: &str, pairs: &[(ArcGrid, ArcGrid)], test: Vec<ArcGrid>) -> ArcTask {
    ArcTask::new(
        ArcTaskId::new(id).expect("task id"),
        pairs
            .iter()
            .cloned()
            .map(|(input, output)| ArcExample { input, output })
            .collect(),
        test,
    )
    .expect("task should validate")
}

fn budget() -> TaskBudget {
    TaskBudget {
        max_wall_ms: 10_000,
        max_candidates: 6,
        max_verifier_evals: 4,
        max_train_pair_execs: 64,
        max_refinement_steps: 0,
        max_model_forward_calls: 0,
        max_ttt_updates: 0,
        max_memory_mb: 128,
    }
}

#[test]
fn mdl_lane_prefers_fill_model_as_the_simpler_ranking_signal() {
    let raw_task = task(
        "mdl-fill-ranking",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                grid(2, 2, &[&[7, 7], &[7, 7]]),
            ),
            (
                grid(2, 2, &[&[0, 1], &[1, 0]]),
                grid(2, 2, &[&[7, 7], &[7, 7]]),
            ),
        ],
        vec![grid(1, 4, &[&[1], &[1], &[0], &[0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let lane = ArcMdlLane::new(ArcMdlLaneConfig::default());

    let run = lane.run(&task, budget()).expect("lane should run");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Proposed);
    assert!(matches!(
        run.candidate_reports[0].representation,
        ArcMdlRepresentation::FillFromInputShape { .. }
    ));
    assert!(run.candidate_reports[0].exact_train_fit);
    assert!(
        run.candidate_reports[0].total_description_length_bits
            <= run.candidate_reports[1].total_description_length_bits
    );
}

#[test]
fn mdl_lane_solves_fixed_shape_fill_tasks_without_pretraining() {
    let solved = grid(2, 2, &[&[4, 4], &[4, 4]]);
    let raw_task = task(
        "mdl-fixed-shape",
        &[
            (grid(1, 3, &[&[1], &[0], &[1]]), solved.clone()),
            (grid(3, 1, &[&[0, 1, 0]]), solved.clone()),
        ],
        vec![grid(1, 1, &[&[9]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let lane = ArcMdlLane::new(ArcMdlLaneConfig {
        initialization_mode: ArcMdlInitializationMode::NoPretraining,
        max_candidates: 6,
    });

    let run = lane.run(&task, budget()).expect("lane should run");
    let best = run.best_hypothesis().expect("best hypothesis should exist");
    let report = ArcCommonVerifier::default()
        .evaluate(&task, best, budget())
        .expect("verification should succeed");
    assert!(report.verifier_pass);
}

#[test]
fn mdl_lane_refuses_when_budget_is_missing() {
    let raw_task = task(
        "mdl-budget",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[7, 7], &[7, 7]]),
        )],
        vec![grid(2, 2, &[&[0, 0], &[0, 0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let lane = ArcMdlLane::new(ArcMdlLaneConfig::default());

    let mut too_small = budget();
    too_small.max_candidates = 0;
    let run = lane.run(&task, too_small).expect("lane should refuse");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Refused);
    assert_eq!(
        run.proposal_batch
            .refusal
            .as_ref()
            .expect("refusal exists")
            .code,
        SolverRefusalCode::MinimumBudgetNotMet
    );
}
