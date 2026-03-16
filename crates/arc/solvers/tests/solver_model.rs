use arc_core::{ArcAction, ArcGrid, ArcTaskId, TraceLocator};
use arc_solvers::{
    ArcDigest, BudgetCounterDelta, BudgetCounterKind, BudgetCounterSummary, BudgetLedger,
    BudgetLedgerError, CandidateDeduplicationStatus, CandidateDeduplicator, CandidateIdentity,
    Hypothesis, HypothesisKind, RefusalEnvelope, SecondAttemptDistinctnessField,
    SolveAttemptEnvelope, SolveAttemptStatus, SolveAttemptVerificationSummary, SolverLaneId,
    SolverPhase, SolverRefusalCode, TaskBudget,
};

use arc_solvers::{ArcGridExpr, ArcProgram, ArcProgramMetadata, ArcSymbol};

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
        max_wall_ms: 1_000,
        max_candidates: 8,
        max_verifier_evals: 8,
        max_train_pair_execs: 8,
        max_refinement_steps: 8,
        max_model_forward_calls: 4,
        max_ttt_updates: 2,
        max_memory_mb: 64,
    }
}

fn static_program(label: &str) -> ArcProgram {
    ArcProgram {
        input_symbol: symbol("input"),
        bindings: Vec::new(),
        body: ArcGridExpr::Input,
        metadata: ArcProgramMetadata {
            label: Some(label.to_owned()),
            ..ArcProgramMetadata::default()
        },
    }
}

#[test]
fn semantic_digest_can_collapse_program_variants_into_one_candidate_signature() {
    let answer = grid(2, 2, &[&[1, 0], &[0, 1]]);
    let program_a = static_program("variant-a");
    let program_b = static_program("variant-b");
    let semantic_digest = digest('a');

    let identity_a = CandidateIdentity::new(
        HypothesisKind::StaticProgram,
        Some(&program_a),
        Some(&answer),
        None,
        Some(semantic_digest.clone()),
    )
    .expect("identity should build");
    let identity_b = CandidateIdentity::new(
        HypothesisKind::StaticProgram,
        Some(&program_b),
        Some(&answer),
        None,
        Some(semantic_digest),
    )
    .expect("identity should build");

    assert_ne!(identity_a.program_digest, identity_b.program_digest);
    assert_eq!(
        identity_a.canonical_signature, identity_b.canonical_signature,
        "semantic digest should define the dedup signature",
    );
}

#[test]
fn deduplicator_returns_the_first_hypothesis_for_duplicate_signatures() {
    let answer = grid(2, 2, &[&[1, 0], &[0, 1]]);
    let lane = SolverLaneId::new("symbolic").expect("lane id");
    let trace = TraceLocator::new("trace://arc-solvers/tests/hypothesis").expect("trace locator");
    let semantic_digest = digest('b');

    let first = Hypothesis::new(
        HypothesisKind::StaticProgram,
        lane.clone(),
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticProgram,
            Some(&static_program("first")),
            Some(&answer),
            None,
            Some(semantic_digest.clone()),
        )
        .expect("identity should build"),
        Some(static_program("first")),
        Some(answer.clone()),
        None,
        0.9,
        trace.clone(),
        BudgetCounterDelta {
            candidates_generated: 1,
            ..BudgetCounterDelta::default()
        },
    )
    .expect("hypothesis should build");

    let second = Hypothesis::new(
        HypothesisKind::StaticProgram,
        lane,
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticProgram,
            Some(&static_program("second")),
            Some(&answer),
            None,
            Some(semantic_digest),
        )
        .expect("identity should build"),
        Some(static_program("second")),
        Some(answer),
        None,
        0.8,
        trace,
        BudgetCounterDelta {
            candidates_generated: 1,
            ..BudgetCounterDelta::default()
        },
    )
    .expect("hypothesis should build");

    let mut deduplicator = CandidateDeduplicator::new();
    let first_decision = deduplicator.record(&first);
    assert_eq!(
        first_decision.status,
        CandidateDeduplicationStatus::Accepted
    );

    let duplicate = deduplicator.record(&second);
    assert_eq!(
        duplicate.status,
        CandidateDeduplicationStatus::Duplicate {
            existing_hypothesis_id: first.id.clone(),
        }
    );
}

#[test]
fn budget_ledger_tracks_usage_and_refuses_overdraw() {
    let mut ledger = BudgetLedger::new(budget());
    let first = ledger
        .apply(BudgetCounterDelta {
            wall_ms: 200,
            candidates_generated: 3,
            verifier_evals: 2,
            peak_memory_mb: 12,
            ..BudgetCounterDelta::default()
        })
        .expect("initial budget delta should fit");
    assert_eq!(
        first,
        BudgetCounterSummary {
            wall_ms: 200,
            candidates_generated: 3,
            verifier_evals: 2,
            peak_memory_mb: 12,
            ..BudgetCounterSummary::default()
        }
    );

    let error = ledger
        .apply(BudgetCounterDelta {
            candidates_generated: 6,
            ..BudgetCounterDelta::default()
        })
        .expect_err("delta should exceed candidate limit");
    assert_eq!(
        error,
        BudgetLedgerError::Overdraw {
            counter: BudgetCounterKind::CandidatesGenerated,
            attempted: 9,
            limit: 8,
        }
    );
}

#[test]
fn second_attempt_distinctness_requires_material_candidate_changes() {
    let answer_a = grid(2, 2, &[&[1, 0], &[0, 1]]);
    let answer_b = grid(2, 2, &[&[0, 1], &[1, 0]]);
    let lane = SolverLaneId::new("symbolic").expect("lane id");
    let trace = TraceLocator::new("trace://arc-solvers/tests/distinct").expect("trace locator");

    let first = Hypothesis::new(
        HypothesisKind::StaticAnswer,
        lane.clone(),
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticAnswer,
            None,
            Some(&answer_a),
            None,
            None,
        )
        .expect("identity should build"),
        None,
        Some(answer_a.clone()),
        None,
        0.7,
        trace.clone(),
        BudgetCounterDelta::default(),
    )
    .expect("hypothesis should build");

    let same = Hypothesis::new(
        HypothesisKind::StaticAnswer,
        lane.clone(),
        1,
        CandidateIdentity::new(
            HypothesisKind::StaticAnswer,
            None,
            Some(&answer_a),
            None,
            None,
        )
        .expect("identity should build"),
        None,
        Some(answer_a),
        None,
        0.6,
        trace.clone(),
        BudgetCounterDelta::default(),
    )
    .expect("hypothesis should build");
    let same_distinctness = same.materially_distinct_from(&first);
    assert!(!same_distinctness.materially_distinct);
    assert!(same_distinctness.changed_fields.is_empty());

    let changed = Hypothesis::new(
        HypothesisKind::StaticAnswer,
        lane,
        1,
        CandidateIdentity::new(
            HypothesisKind::StaticAnswer,
            None,
            Some(&answer_b),
            None,
            None,
        )
        .expect("identity should build"),
        None,
        Some(answer_b),
        None,
        0.6,
        trace,
        BudgetCounterDelta::default(),
    )
    .expect("hypothesis should build");
    let distinctness = changed.materially_distinct_from(&first);
    assert!(distinctness.materially_distinct);
    assert_eq!(
        distinctness.changed_fields,
        vec![SecondAttemptDistinctnessField::AnswerDigest]
    );
}

#[test]
fn solve_attempt_envelope_enforces_status_rules_and_round_trips() {
    let task_id = ArcTaskId::new("arc-demo").expect("task id");
    let trace_locator =
        TraceLocator::new("trace://arc-solvers/tests/attempt-envelope").expect("trace locator");
    let answer = grid(2, 2, &[&[1, 1], &[0, 0]]);
    let verification = SolveAttemptVerificationSummary {
        exact_fit: true,
        verifier_pass: true,
        simplicity_score: 0.8,
        stability_score: 0.9,
        spuriousness_risk: 0.1,
    };

    let envelope = SolveAttemptEnvelope::new(
        task_id,
        0,
        budget(),
        SolveAttemptStatus::Solved,
        Some(answer),
        Some(SolverLaneId::new("symbolic").expect("lane id")),
        0.93,
        Some(verification),
        BudgetCounterSummary {
            wall_ms: 320,
            candidates_generated: 4,
            verifier_evals: 2,
            train_pair_execs: 2,
            ..BudgetCounterSummary::default()
        },
        digest('c'),
        trace_locator,
        digest('d'),
        digest('e'),
        None,
    )
    .expect("envelope should validate");

    let encoded = serde_json::to_string_pretty(&envelope).expect("envelope should serialize");
    let decoded: SolveAttemptEnvelope =
        serde_json::from_str(&encoded).expect("envelope should deserialize");
    assert_eq!(decoded, envelope);

    let refusal = RefusalEnvelope::new(
        SolverRefusalCode::IndistinctSecondAttempt,
        SolverPhase::Arbitrate,
        "attempt 2 matched the first candidate signature",
    )
    .expect("refusal should validate");
    let error = SolveAttemptEnvelope::new(
        ArcTaskId::new("arc-demo-2").expect("task id"),
        1,
        budget(),
        SolveAttemptStatus::Refused,
        Some(grid(1, 1, &[&[1]])),
        Some(SolverLaneId::new("symbolic").expect("lane id")),
        0.1,
        None,
        BudgetCounterSummary::default(),
        digest('f'),
        TraceLocator::new("trace://arc-solvers/tests/refused-attempt").expect("trace locator"),
        digest('a'),
        digest('b'),
        Some(refusal),
    )
    .expect_err("refused attempts must not include an answer");
    assert_eq!(
        error.to_string(),
        "ARC refused attempts must not include a selected answer or lane"
    );
}

#[test]
fn planned_action_steps_digest_stably_for_interactive_candidates() {
    let action = ArcAction::action6(3, 4).expect("action should validate");
    let steps = vec![
        arc_solvers::PlannedActionStep {
            action,
            expected_state: None,
            expected_level_index: Some(0),
            reset_marker: false,
        },
        arc_solvers::PlannedActionStep {
            action: ArcAction::Reset,
            expected_state: None,
            expected_level_index: None,
            reset_marker: true,
        },
    ];

    let identity = CandidateIdentity::new(
        HypothesisKind::InteractivePlan,
        None,
        None,
        Some(&steps),
        None,
    )
    .expect("identity should build");

    assert!(identity.action_plan_digest.is_some());
    assert!(identity.program_digest.is_none());
    assert!(identity.answer_digest.is_none());
}
