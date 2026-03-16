use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use arc_core::{ArcTaskId, TraceLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArbiterDecision, ArcDigest, ArcDigestError, Hypothesis, HypothesisId, LaneProposalBatch,
    RefusalEnvelope, SecondAttemptDistinctness, SolverLaneId, SolverPhase, SolverRefusalCode,
    VerificationReport,
};

/// Ownership summary for portfolio arbitration and attempt policy.
pub const PORTFOLIO_ARBITER_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns verifier-first portfolio ranking, cross-lane agreement scoring, and second-attempt gating";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortfolioArbiterConfig {
    pub default_lane_reliability: f32,
    pub lane_reliability_priors: BTreeMap<String, f32>,
    pub verifier_pass_weight: f32,
    pub exact_fit_weight: f32,
    pub simplicity_weight: f32,
    pub stability_weight: f32,
    pub agreement_weight: f32,
    pub local_score_weight: f32,
    pub lane_prior_weight: f32,
    pub spuriousness_penalty_weight: f32,
    pub compute_penalty_weight: f32,
}

impl Default for PortfolioArbiterConfig {
    fn default() -> Self {
        let mut lane_reliability_priors = BTreeMap::new();
        lane_reliability_priors.insert(String::from("symbolic"), 0.85);
        lane_reliability_priors.insert(String::from("recursive_tiny_model"), 0.65);
        lane_reliability_priors.insert(String::from("mdl_compression"), 0.6);
        lane_reliability_priors.insert(String::from("transductive"), 0.5);

        Self {
            default_lane_reliability: 0.55,
            lane_reliability_priors,
            verifier_pass_weight: 0.35,
            exact_fit_weight: 0.2,
            simplicity_weight: 0.12,
            stability_weight: 0.12,
            agreement_weight: 0.08,
            local_score_weight: 0.08,
            lane_prior_weight: 0.1,
            spuriousness_penalty_weight: 0.12,
            compute_penalty_weight: 0.07,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortfolioArbiterCandidateScore {
    pub hypothesis_id: HypothesisId,
    pub lane_id: SolverLaneId,
    pub final_score: f32,
    pub verifier_pass: bool,
    pub exact_fit: bool,
    pub simplicity_score: f32,
    pub stability_score: f32,
    pub spuriousness_risk: f32,
    pub local_score: f32,
    pub lane_reliability_prior: f32,
    pub cross_lane_agreement_count: u32,
    pub cross_lane_agreement_score: f32,
    pub compute_penalty: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecondAttemptCandidateGate {
    pub hypothesis_id: HypothesisId,
    pub distinctness: SecondAttemptDistinctness,
    pub score_gap: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecondAttemptPolicyOutcome {
    pub allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_hypothesis_id: Option<HypothesisId>,
    pub materially_distinct_candidates: Vec<SecondAttemptCandidateGate>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortfolioArbiterRun {
    pub candidate_scores: Vec<PortfolioArbiterCandidateScore>,
    pub second_attempt_policy: SecondAttemptPolicyOutcome,
    pub decision: ArbiterDecision,
}

#[derive(Clone, Debug, Default)]
pub struct PortfolioArbiter {
    config: PortfolioArbiterConfig,
}

impl PortfolioArbiter {
    #[must_use]
    pub fn new(config: PortfolioArbiterConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &PortfolioArbiterConfig {
        &self.config
    }

    pub fn decide(
        &self,
        task_id: ArcTaskId,
        attempt_index: u8,
        lane_batches: &[LaneProposalBatch],
        verification_reports: &[VerificationReport],
    ) -> Result<PortfolioArbiterRun, PortfolioArbiterError> {
        let trace_locator = TraceLocator::new(format!(
            "trace://arc-solvers/arbiter/{}/attempt-{}",
            task_id.as_str(),
            attempt_index
        ))?;
        let mut hypotheses = BTreeMap::new();
        for batch in lane_batches {
            for proposal in &batch.proposals {
                hypotheses.insert(
                    proposal.hypothesis.id.clone(),
                    proposal.hypothesis.clone(),
                );
            }
        }

        let verification_by_hypothesis = verification_reports
            .iter()
            .map(|report| (report.hypothesis_id.clone(), report))
            .collect::<BTreeMap<_, _>>();
        let total_lane_count = hypotheses
            .values()
            .map(|hypothesis| hypothesis.lane_id.as_str().to_owned())
            .collect::<BTreeSet<_>>()
            .len()
            .max(1) as u32;
        let agreement_index = build_agreement_index(hypotheses.values());

        let mut candidate_scores = hypotheses
            .values()
            .filter_map(|hypothesis| {
                let report = verification_by_hypothesis.get(&hypothesis.id)?;
                Some(score_candidate(
                    &self.config,
                    total_lane_count,
                    &agreement_index,
                    hypothesis,
                    report,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        candidate_scores.sort_by(compare_candidate_scores);
        if candidate_scores.is_empty() {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::InvalidCandidate,
                SolverPhase::Arbitrate,
                "portfolio arbiter requires at least one verified hypothesis",
            )?;
            let second_attempt_policy = SecondAttemptPolicyOutcome {
                allowed: false,
                selected_hypothesis_id: None,
                materially_distinct_candidates: Vec::new(),
                detail: String::from("no verified hypotheses were available for arbitration"),
            };
            let decision = ArbiterDecision::new(
                task_id,
                attempt_index,
                None,
                Vec::new(),
                false,
                Some(refusal),
                "no verified hypotheses were available for arbitration",
                trace_locator,
                crate::BudgetCounterDelta::default(),
            )?;
            return Ok(PortfolioArbiterRun {
                candidate_scores,
                second_attempt_policy,
                decision,
            });
        }

        let ranked_hypotheses = candidate_scores
            .iter()
            .map(|candidate| candidate.hypothesis_id.clone())
            .collect::<Vec<_>>();
        let selected = candidate_scores
            .first()
            .and_then(|candidate| hypotheses.get(&candidate.hypothesis_id))
            .expect("candidate score should refer to a hypothesis");
        let second_attempt_policy =
            build_second_attempt_policy(selected, &candidate_scores, &hypotheses);
        let decision_reason = if candidate_scores[0].verifier_pass && candidate_scores[0].exact_fit {
            String::from("selected the highest-ranked verifier-passing exact-fit hypothesis")
        } else if second_attempt_policy.allowed {
            String::from(
                "selected the current best hypothesis while preserving materially distinct candidates for a second attempt",
            )
        } else {
            String::from("selected the highest-ranked hypothesis after verifier-first portfolio scoring")
        };
        let decision = ArbiterDecision::new(
            task_id,
            attempt_index,
            Some(selected.id.clone()),
            ranked_hypotheses,
            second_attempt_policy.allowed,
            None,
            decision_reason,
            trace_locator,
            crate::BudgetCounterDelta::default(),
        )?;

        Ok(PortfolioArbiterRun {
            candidate_scores,
            second_attempt_policy,
            decision,
        })
    }
}

#[derive(Debug, Error)]
pub enum PortfolioArbiterError {
    #[error("failed to compute portfolio-arbiter digests: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to create portfolio-arbiter trace locator: {0}")]
    TraceLocator(#[from] arc_core::TraceLocatorError),
    #[error("failed to create portfolio-arbiter refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
    #[error("failed to create portfolio-arbiter decision: {0}")]
    Trace(#[from] crate::TraceBundleError),
}

fn build_agreement_index<'a>(
    hypotheses: impl Iterator<Item = &'a Hypothesis>,
) -> BTreeMap<ArcDigest, BTreeSet<String>> {
    let mut agreement_index = BTreeMap::new();
    for hypothesis in hypotheses {
        if let Some(answer_digest) = &hypothesis.candidate_identity.answer_digest {
            agreement_index
                .entry(answer_digest.clone())
                .or_insert_with(BTreeSet::new)
                .insert(hypothesis.lane_id.as_str().to_owned());
        }
    }
    agreement_index
}

fn score_candidate(
    config: &PortfolioArbiterConfig,
    total_lane_count: u32,
    agreement_index: &BTreeMap<ArcDigest, BTreeSet<String>>,
    hypothesis: &Hypothesis,
    report: &VerificationReport,
) -> Result<PortfolioArbiterCandidateScore, PortfolioArbiterError> {
    let lane_reliability_prior = config
        .lane_reliability_priors
        .get(hypothesis.lane_id.as_str())
        .copied()
        .unwrap_or(config.default_lane_reliability);
    let cross_lane_agreement_count = hypothesis
        .candidate_identity
        .answer_digest
        .as_ref()
        .and_then(|digest| agreement_index.get(digest))
        .map(|lanes| lanes.len() as u32)
        .unwrap_or(1);
    let cross_lane_agreement_score = if total_lane_count <= 1 || cross_lane_agreement_count <= 1 {
        0.0
    } else {
        (cross_lane_agreement_count - 1) as f32 / (total_lane_count - 1) as f32
    };
    let compute_penalty = compute_penalty(hypothesis);
    let final_score = config.verifier_pass_weight * if report.verifier_pass { 1.0 } else { 0.0 }
        + config.exact_fit_weight * if report.exact_fit { 1.0 } else { 0.0 }
        + config.simplicity_weight * report.simplicity_score
        + config.stability_weight * report.stability_score
        + config.agreement_weight * cross_lane_agreement_score
        + config.local_score_weight * hypothesis.local_score
        + config.lane_prior_weight * lane_reliability_prior
        - config.spuriousness_penalty_weight * report.spuriousness_risk
        - config.compute_penalty_weight * compute_penalty;

    Ok(PortfolioArbiterCandidateScore {
        hypothesis_id: hypothesis.id.clone(),
        lane_id: hypothesis.lane_id.clone(),
        final_score,
        verifier_pass: report.verifier_pass,
        exact_fit: report.exact_fit,
        simplicity_score: report.simplicity_score,
        stability_score: report.stability_score,
        spuriousness_risk: report.spuriousness_risk,
        local_score: hypothesis.local_score,
        lane_reliability_prior,
        cross_lane_agreement_count,
        cross_lane_agreement_score,
        compute_penalty,
    })
}

fn compute_penalty(hypothesis: &Hypothesis) -> f32 {
    let budget = hypothesis.budget_delta;
    let raw = budget.train_pair_execs as f32 / 64.0
        + budget.model_forward_calls as f32 / 8.0
        + budget.refinement_steps as f32 / 8.0
        + budget.ttt_updates as f32 / 4.0;
    raw.min(1.0)
}

fn build_second_attempt_policy(
    selected: &Hypothesis,
    candidate_scores: &[PortfolioArbiterCandidateScore],
    hypotheses: &BTreeMap<HypothesisId, Hypothesis>,
) -> SecondAttemptPolicyOutcome {
    let selected_score = candidate_scores
        .first()
        .map(|candidate| candidate.final_score)
        .unwrap_or_default();
    let materially_distinct_candidates = candidate_scores
        .iter()
        .skip(1)
        .filter_map(|candidate| {
            let hypothesis = hypotheses.get(&candidate.hypothesis_id)?;
            let distinctness = hypothesis.materially_distinct_from(selected);
            if !distinctness.materially_distinct {
                return None;
            }
            Some(SecondAttemptCandidateGate {
                hypothesis_id: hypothesis.id.clone(),
                distinctness,
                score_gap: (selected_score - candidate.final_score).max(0.0),
            })
        })
        .collect::<Vec<_>>();
    let selected_candidate = &candidate_scores[0];
    let allowed = !selected_candidate.verifier_pass
        && !selected_candidate.exact_fit
        && !materially_distinct_candidates.is_empty();
    let detail = if allowed {
        String::from("a materially distinct verified alternative is available for a second attempt")
    } else if selected_candidate.verifier_pass && selected_candidate.exact_fit {
        String::from("second attempt not allowed because the top-ranked hypothesis already passes verification exactly")
    } else {
        String::from("second attempt not allowed because no materially distinct verified alternative remained")
    };

    SecondAttemptPolicyOutcome {
        allowed,
        selected_hypothesis_id: Some(selected.id.clone()),
        materially_distinct_candidates,
        detail,
    }
}

fn compare_candidate_scores(
    left: &PortfolioArbiterCandidateScore,
    right: &PortfolioArbiterCandidateScore,
) -> Ordering {
    right
        .final_score
        .partial_cmp(&left.final_score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| right.verifier_pass.cmp(&left.verifier_pass))
        .then_with(|| right.exact_fit.cmp(&left.exact_fit))
        .then_with(|| {
            right
                .cross_lane_agreement_count
                .cmp(&left.cross_lane_agreement_count)
        })
        .then_with(|| {
            right
                .local_score
                .partial_cmp(&left.local_score)
                .unwrap_or(Ordering::Equal)
        })
}
