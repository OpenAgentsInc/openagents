use arc_core::{ArcGrid, ArcTaskId, TraceLocator};
use arc_solvers::{
    ArcDigest, BudgetCounterDelta, CandidateIdentity, Hypothesis, HypothesisKind,
    LaneBatchStatus, LaneProposalBatch, PortfolioArbiter, PortfolioArbiterConfig,
    ProposalPhase, SolverLaneId, TracedLaneProposal, VerificationReport,
};

fn grid(width: u8, height: u8, rows: &[&[u8]]) -> ArcGrid {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    ArcGrid::new(width, height, cells).expect("grid should validate")
}

fn hypothesis(lane: &str, label: &str, answer: ArcGrid, local_score: f32) -> Hypothesis {
    let lane_id = SolverLaneId::new(lane).expect("lane id");
    Hypothesis::new(
        HypothesisKind::StaticAnswer,
        lane_id,
        0,
        CandidateIdentity::new(HypothesisKind::StaticAnswer, None, Some(&answer), None, None)
            .expect("candidate identity"),
        None,
        Some(answer),
        None,
        local_score,
        TraceLocator::new(format!("trace://tests/arbiter/{}/{}", lane, label))
            .expect("trace locator"),
        BudgetCounterDelta::default(),
    )
    .expect("hypothesis should validate")
}

fn batch(task_id: &ArcTaskId, lane: &str, hypotheses: Vec<Hypothesis>) -> LaneProposalBatch {
    LaneProposalBatch::new(
        SolverLaneId::new(lane).expect("lane id"),
        ProposalPhase::Propose,
        LaneBatchStatus::Proposed,
        hypotheses
            .into_iter()
            .enumerate()
            .map(|(index, hypothesis)| TracedLaneProposal {
                local_rank: index as u32,
                rationale_digest: ArcDigest::from_serializable(&hypothesis.id.as_str())
                    .expect("digest"),
                hypothesis,
            })
            .collect(),
        None,
        TraceLocator::new(format!(
            "trace://tests/arbiter/{}/{}",
            task_id.as_str(),
            lane
        ))
        .expect("trace locator"),
        BudgetCounterDelta::default(),
    )
    .expect("batch should validate")
}

fn report(
    hypothesis: &Hypothesis,
    exact_fit: bool,
    verifier_pass: bool,
    simplicity_score: f32,
    stability_score: f32,
    spuriousness_risk: f32,
) -> VerificationReport {
    VerificationReport {
        hypothesis_id: hypothesis.id.clone(),
        verifier_config_digest: ArcDigest::from_serializable(&hypothesis.id.as_str())
            .expect("digest"),
        exact_fit,
        pair_results: Vec::new(),
        falsification_checks: Vec::new(),
        simplicity_score,
        stability_score,
        spuriousness_risk,
        verifier_pass,
        trace_locator: TraceLocator::new(format!(
            "trace://tests/arbiter/verify/{}",
            hypothesis.id.as_str()
        ))
        .expect("trace locator"),
        budget_delta: BudgetCounterDelta::default(),
        refusal: None,
    }
}

#[test]
fn portfolio_arbiter_prefers_verified_cross_lane_consensus() {
    let task_id = ArcTaskId::new("arbiter-consensus").expect("task id");
    let answer_a = grid(2, 2, &[&[1, 1], &[1, 1]]);
    let answer_b = grid(2, 2, &[&[1, 0], &[0, 1]]);
    let symbolic = hypothesis("symbolic", "a", answer_a.clone(), 0.9);
    let recursive = hypothesis("recursive_tiny_model", "a", answer_a, 0.7);
    let mdl = hypothesis("mdl_compression", "b", answer_b, 0.85);

    let batches = vec![
        batch(&task_id, "symbolic", vec![symbolic.clone()]),
        batch(&task_id, "recursive_tiny_model", vec![recursive.clone()]),
        batch(&task_id, "mdl_compression", vec![mdl.clone()]),
    ];
    let reports = vec![
        report(&symbolic, true, true, 0.8, 0.9, 0.05),
        report(&recursive, true, true, 0.75, 0.8, 0.1),
        report(&mdl, true, true, 0.95, 0.7, 0.1),
    ];

    let run = PortfolioArbiter::new(PortfolioArbiterConfig::default())
        .decide(task_id, 0, &batches, &reports)
        .expect("arbiter should run");
    assert_eq!(
        run.decision
            .selected_hypothesis
            .as_ref()
            .expect("selected hypothesis"),
        &symbolic.id
    );
    assert_eq!(run.candidate_scores[0].cross_lane_agreement_count, 2);
    assert!(!run.second_attempt_policy.allowed);
}

#[test]
fn portfolio_arbiter_allows_second_attempt_only_for_materially_distinct_candidates() {
    let task_id = ArcTaskId::new("arbiter-second-attempt").expect("task id");
    let answer_a = grid(2, 2, &[&[1, 1], &[1, 1]]);
    let answer_b = grid(2, 2, &[&[0, 0], &[0, 0]]);
    let symbolic = hypothesis("symbolic", "a", answer_a.clone(), 0.72);
    let transductive = hypothesis("transductive", "same-answer", answer_a, 0.55);
    let recursive = hypothesis("recursive_tiny_model", "b", answer_b, 0.68);

    let batches = vec![
        batch(&task_id, "symbolic", vec![symbolic.clone()]),
        batch(&task_id, "transductive", vec![transductive.clone()]),
        batch(&task_id, "recursive_tiny_model", vec![recursive.clone()]),
    ];
    let reports = vec![
        report(&symbolic, false, false, 0.5, 0.45, 0.35),
        report(&transductive, false, false, 0.45, 0.4, 0.4),
        report(&recursive, false, false, 0.48, 0.52, 0.25),
    ];

    let run = PortfolioArbiter::new(PortfolioArbiterConfig::default())
        .decide(task_id, 0, &batches, &reports)
        .expect("arbiter should run");
    assert!(run.second_attempt_policy.allowed);
    assert_eq!(run.second_attempt_policy.materially_distinct_candidates.len(), 1);
    assert_eq!(
        run.second_attempt_policy.materially_distinct_candidates[0].hypothesis_id,
        recursive.id
    );
}

#[test]
fn portfolio_arbiter_refuses_without_verified_hypotheses() {
    let task_id = ArcTaskId::new("arbiter-no-reports").expect("task id");
    let symbolic = hypothesis("symbolic", "a", grid(1, 1, &[&[1]]), 0.9);
    let batches = vec![batch(&task_id, "symbolic", vec![symbolic])];

    let run = PortfolioArbiter::new(PortfolioArbiterConfig::default())
        .decide(task_id, 0, &batches, &[])
        .expect("arbiter should run");
    assert!(run.decision.selected_hypothesis.is_none());
    assert!(
        run.decision
            .refusal
            .as_ref()
            .expect("refusal exists")
            .detail
            .contains("verified")
    );
}
