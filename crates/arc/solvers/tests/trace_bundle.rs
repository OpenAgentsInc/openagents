use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use arc_core::{ArcExample, ArcGrid, ArcTask, ArcTaskId, TraceLocator, canonicalize_task};
use arc_solvers::{
    ArbiterDecision, ArcCommonVerifier, ArcDigest, ArcGridExpr, ArcProgram, ArcSymbol,
    BudgetCounterDelta, BudgetCounterSummary, CandidateIdentity, CandidateVerifier, Hypothesis,
    HypothesisKind, LaneBatchStatus, LaneProposalBatch, ProposalPhase, RefusalEnvelope,
    SolveAttemptEnvelope, SolveAttemptStatus, SolveAttemptVerificationSummary, SolverLaneId,
    SolverPhase, SolverRefusalCode, SolverTraceBundle, TaskBudget, TracedLaneProposal,
    read_trace_bundle_json_file, replay_trace_bundle, write_trace_bundle_json_file,
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

fn digest(ch: char) -> ArcDigest {
    ArcDigest::new(ch.to_string().repeat(64)).expect("digest should validate")
}

fn budget() -> TaskBudget {
    TaskBudget {
        max_wall_ms: 10_000,
        max_candidates: 16,
        max_verifier_evals: 2,
        max_train_pair_execs: 64,
        max_refinement_steps: 8,
        max_model_forward_calls: 0,
        max_ttt_updates: 0,
        max_memory_mb: 128,
    }
}

fn task() -> ArcTask {
    ArcTask::new(
        ArcTaskId::new("trace-bundle-task").expect("task id"),
        vec![
            ArcExample {
                input: grid(2, 2, &[&[1, 0], &[0, 1]]),
                output: grid(2, 2, &[&[1, 0], &[0, 1]]),
            },
            ArcExample {
                input: grid(2, 2, &[&[0, 2], &[2, 0]]),
                output: grid(2, 2, &[&[0, 2], &[2, 0]]),
            },
            ArcExample {
                input: grid(2, 2, &[&[3, 3], &[0, 0]]),
                output: grid(2, 2, &[&[3, 3], &[0, 0]]),
            },
        ],
        vec![grid(1, 1, &[&[0]])],
    )
    .expect("task should validate")
}

fn identity_hypothesis(trace_locator: &str) -> Hypothesis {
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
        TraceLocator::new(trace_locator).expect("trace locator"),
        BudgetCounterDelta::default(),
    )
    .expect("hypothesis should validate")
}

fn build_bundle() -> SolverTraceBundle {
    let canonical = canonicalize_task(&task()).expect("task should canonicalize");
    let hypothesis = identity_hypothesis("trace://arc-solvers/tests/trace-bundle/hypothesis");
    let verifier = ArcCommonVerifier::default();
    let verification = verifier
        .evaluate(&canonical, &hypothesis, budget())
        .expect("verification should succeed");

    let proposal_batch = LaneProposalBatch::new(
        SolverLaneId::new("symbolic").expect("lane id"),
        ProposalPhase::Propose,
        LaneBatchStatus::Proposed,
        vec![TracedLaneProposal {
            hypothesis: hypothesis.clone(),
            local_rank: 0,
            rationale_digest: digest('c'),
        }],
        None,
        TraceLocator::new("trace://arc-solvers/tests/trace-bundle/proposal")
            .expect("trace locator"),
        BudgetCounterDelta {
            candidates_generated: 1,
            ..BudgetCounterDelta::default()
        },
    )
    .expect("proposal batch should validate");

    let refinement_batch = LaneProposalBatch::new(
        SolverLaneId::new("symbolic").expect("lane id"),
        ProposalPhase::Refine,
        LaneBatchStatus::Refused,
        Vec::new(),
        Some(
            RefusalEnvelope::new(
                SolverRefusalCode::IndistinctSecondAttempt,
                SolverPhase::Refine,
                "refinement refused because the candidate was not materially distinct",
            )
            .expect("refusal should validate"),
        ),
        TraceLocator::new("trace://arc-solvers/tests/trace-bundle/refinement")
            .expect("trace locator"),
        BudgetCounterDelta {
            refinement_steps: 1,
            ..BudgetCounterDelta::default()
        },
    )
    .expect("refinement batch should validate");

    let arbiter = ArbiterDecision::new(
        canonical.raw.id.clone(),
        0,
        Some(hypothesis.id.clone()),
        vec![hypothesis.id.clone()],
        false,
        None,
        "selected the only verified hypothesis",
        TraceLocator::new("trace://arc-solvers/tests/trace-bundle/arbiter").expect("trace locator"),
        BudgetCounterDelta {
            wall_ms: 5,
            ..BudgetCounterDelta::default()
        },
    )
    .expect("arbiter decision should validate");

    let budget_summary = BudgetCounterSummary {
        wall_ms: verification.budget_delta.wall_ms + 5,
        candidates_generated: 1 + verification.budget_delta.candidates_generated,
        verifier_evals: verification.budget_delta.verifier_evals,
        train_pair_execs: verification.budget_delta.train_pair_execs,
        refinement_steps: 1 + verification.budget_delta.refinement_steps,
        model_forward_calls: verification.budget_delta.model_forward_calls,
        ttt_updates: verification.budget_delta.ttt_updates,
        peak_memory_mb: verification.budget_delta.peak_memory_mb,
    };
    let final_result = SolveAttemptEnvelope::new(
        canonical.raw.id.clone(),
        0,
        budget(),
        SolveAttemptStatus::Solved,
        Some(grid(1, 1, &[&[0]])),
        Some(SolverLaneId::new("symbolic").expect("lane id")),
        0.93,
        Some(SolveAttemptVerificationSummary {
            exact_fit: verification.exact_fit,
            verifier_pass: verification.verifier_pass,
            simplicity_score: verification.simplicity_score,
            stability_score: verification.stability_score,
            spuriousness_risk: verification.spuriousness_risk,
        }),
        budget_summary,
        digest('d'),
        TraceLocator::new("trace://arc-solvers/tests/trace-bundle/final-result")
            .expect("trace locator"),
        digest('a'),
        digest('b'),
        None,
    )
    .expect("final solve envelope should validate");

    SolverTraceBundle::build(
        digest('a'),
        digest('b'),
        vec![proposal_batch],
        vec![verification],
        vec![refinement_batch],
        arbiter,
        final_result,
    )
    .expect("trace bundle should build")
}

#[test]
fn trace_bundle_round_trips_and_replays_deterministically() {
    let first = build_bundle();
    let second = build_bundle();
    assert_eq!(
        first, second,
        "repeated bundle builds should be deterministic"
    );

    let report = replay_trace_bundle(&first).expect("bundle replay should succeed");
    assert_eq!(report.proposal_batch_count, 1);
    assert_eq!(report.verification_report_count, 1);
    assert_eq!(report.refinement_batch_count, 1);
    assert_eq!(
        report.selected_hypothesis,
        first.arbiter_decision.selected_hypothesis
    );

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tests")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("arc-solvers-trace-bundle-{unique}.json"));
    write_trace_bundle_json_file(&path, &first).expect("bundle should write");
    let decoded = read_trace_bundle_json_file(&path).expect("bundle should read");
    assert_eq!(decoded, first);
    fs::remove_file(path).expect("temporary bundle file should remove");
}

#[test]
fn trace_bundle_replay_refuses_unverified_selected_hypotheses() {
    let mut bundle = build_bundle();
    bundle.verification_reports.clear();

    let error = replay_trace_bundle(&bundle).expect_err("unverified selection should fail");
    assert_eq!(
        error.to_string(),
        "trace bundle selected a hypothesis that was not verified"
    );
}
