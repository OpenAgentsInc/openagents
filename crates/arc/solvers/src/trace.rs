use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use arc_core::{ARC_CORE_SCHEMA_VERSION, ArcTaskId, TraceLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, BudgetCounterDelta, BudgetCounterSummary, BudgetLedgerError,
    Hypothesis, HypothesisId, RefusalEnvelope, SolveAttemptEnvelope, SolveAttemptEnvelopeError,
    SolverLaneId, VerificationReport,
};

/// Ownership summary for solver trace bundles and replay validation.
pub const TRACE_BUNDLE_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns proposal/verification/refinement/arbiter trace bundles and replay validation";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalPhase {
    Propose,
    Refine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneBatchStatus {
    Proposed,
    Empty,
    Refused,
    BudgetExhausted,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TracedLaneProposal {
    pub hypothesis: Hypothesis,
    pub local_rank: u32,
    pub rationale_digest: ArcDigest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaneProposalBatch {
    pub lane_id: SolverLaneId,
    pub phase: ProposalPhase,
    pub status: LaneBatchStatus,
    pub proposals: Vec<TracedLaneProposal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalEnvelope>,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

impl LaneProposalBatch {
    pub fn new(
        lane_id: SolverLaneId,
        phase: ProposalPhase,
        status: LaneBatchStatus,
        proposals: Vec<TracedLaneProposal>,
        refusal: Option<RefusalEnvelope>,
        trace_locator: TraceLocator,
        budget_delta: BudgetCounterDelta,
    ) -> Result<Self, TraceBundleError> {
        match status {
            LaneBatchStatus::Proposed if proposals.is_empty() => {
                return Err(TraceBundleError::InvalidLaneBatch(
                    "proposed batches require at least one proposal",
                ));
            }
            LaneBatchStatus::Refused if refusal.is_none() || !proposals.is_empty() => {
                return Err(TraceBundleError::InvalidLaneBatch(
                    "refused batches require a refusal envelope and must not carry proposals",
                ));
            }
            LaneBatchStatus::Empty | LaneBatchStatus::BudgetExhausted if !proposals.is_empty() => {
                return Err(TraceBundleError::InvalidLaneBatch(
                    "empty or budget-exhausted batches must not carry proposals",
                ));
            }
            _ => {}
        }

        Ok(Self {
            lane_id,
            phase,
            status,
            proposals,
            refusal,
            trace_locator,
            budget_delta,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArbiterDecision {
    pub task_id: ArcTaskId,
    pub attempt_index: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_hypothesis: Option<HypothesisId>,
    pub ranked_hypotheses: Vec<HypothesisId>,
    pub second_attempt_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalEnvelope>,
    pub decision_reason: String,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

impl ArbiterDecision {
    pub fn new(
        task_id: ArcTaskId,
        attempt_index: u8,
        selected_hypothesis: Option<HypothesisId>,
        ranked_hypotheses: Vec<HypothesisId>,
        second_attempt_allowed: bool,
        refusal: Option<RefusalEnvelope>,
        decision_reason: impl Into<String>,
        trace_locator: TraceLocator,
        budget_delta: BudgetCounterDelta,
    ) -> Result<Self, TraceBundleError> {
        let decision_reason = decision_reason.into();
        let trimmed = decision_reason.trim();
        if trimmed.is_empty() {
            return Err(TraceBundleError::InvalidArbiterDecision(
                "decision reason must not be empty",
            ));
        }
        if let Some(selected) = &selected_hypothesis {
            if !ranked_hypotheses
                .iter()
                .any(|candidate| candidate == selected)
            {
                return Err(TraceBundleError::InvalidArbiterDecision(
                    "selected hypothesis must be present in the ranked list",
                ));
            }
        }

        Ok(Self {
            task_id,
            attempt_index,
            selected_hypothesis,
            ranked_hypotheses,
            second_attempt_allowed,
            refusal,
            decision_reason: trimmed.to_owned(),
            trace_locator,
            budget_delta,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceBundleManifest {
    pub schema_version: u32,
    pub bundle_id: String,
    pub task_id: ArcTaskId,
    pub seed_bundle_digest: ArcDigest,
    pub solver_manifest_digest: ArcDigest,
    pub proposal_batches: Vec<TraceLocator>,
    pub verification_reports: Vec<TraceLocator>,
    pub refinement_batches: Vec<TraceLocator>,
    pub arbiter_decision: TraceLocator,
    pub budget_ledger_digest: ArcDigest,
    pub final_result_digest: ArcDigest,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolverTraceBundle {
    pub manifest: TraceBundleManifest,
    pub proposal_batches: Vec<LaneProposalBatch>,
    pub verification_reports: Vec<VerificationReport>,
    pub refinement_batches: Vec<LaneProposalBatch>,
    pub arbiter_decision: ArbiterDecision,
    pub final_result: SolveAttemptEnvelope,
}

impl SolverTraceBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        seed_bundle_digest: ArcDigest,
        solver_manifest_digest: ArcDigest,
        proposal_batches: Vec<LaneProposalBatch>,
        verification_reports: Vec<VerificationReport>,
        refinement_batches: Vec<LaneProposalBatch>,
        arbiter_decision: ArbiterDecision,
        final_result: SolveAttemptEnvelope,
    ) -> Result<Self, TraceBundleError> {
        validate_bundle_components(
            &proposal_batches,
            &verification_reports,
            &refinement_batches,
            &arbiter_decision,
            &final_result,
        )?;

        let budget_summary = summarize_bundle_budget(
            &proposal_batches,
            &verification_reports,
            &refinement_batches,
            &arbiter_decision,
        )?;
        if budget_summary != final_result.budget_summary {
            return Err(TraceBundleError::BudgetSummaryMismatch {
                expected: final_result.budget_summary,
                actual: budget_summary,
            });
        }

        let proposal_locators = proposal_batches
            .iter()
            .map(|batch| batch.trace_locator.clone())
            .collect::<Vec<_>>();
        let verification_locators = verification_reports
            .iter()
            .map(|report| report.trace_locator.clone())
            .collect::<Vec<_>>();
        let refinement_locators = refinement_batches
            .iter()
            .map(|batch| batch.trace_locator.clone())
            .collect::<Vec<_>>();
        let budget_ledger_digest = ArcDigest::from_serializable(&budget_summary)?;
        let final_result_digest = ArcDigest::from_serializable(&final_result)?;
        let bundle_digest = ArcDigest::from_serializable(&TraceBundleIdInput {
            task_id: &final_result.task_id,
            seed_bundle_digest: &seed_bundle_digest,
            solver_manifest_digest: &solver_manifest_digest,
            proposal_locators: &proposal_locators,
            verification_locators: &verification_locators,
            refinement_locators: &refinement_locators,
            arbiter_trace_locator: &arbiter_decision.trace_locator,
            budget_ledger_digest: &budget_ledger_digest,
            final_result_digest: &final_result_digest,
        })?;
        let manifest = TraceBundleManifest {
            schema_version: ARC_CORE_SCHEMA_VERSION,
            bundle_id: format!("trace-bundle-{}", bundle_digest.as_str()),
            task_id: final_result.task_id.clone(),
            seed_bundle_digest,
            solver_manifest_digest,
            proposal_batches: proposal_locators,
            verification_reports: verification_locators,
            refinement_batches: refinement_locators,
            arbiter_decision: arbiter_decision.trace_locator.clone(),
            budget_ledger_digest,
            final_result_digest,
        };

        Ok(Self {
            manifest,
            proposal_batches,
            verification_reports,
            refinement_batches,
            arbiter_decision,
            final_result,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceBundleReplayReport {
    pub bundle_id: String,
    pub proposal_batch_count: usize,
    pub verification_report_count: usize,
    pub refinement_batch_count: usize,
    pub selected_hypothesis: Option<HypothesisId>,
}

pub fn replay_trace_bundle(
    bundle: &SolverTraceBundle,
) -> Result<TraceBundleReplayReport, TraceBundleError> {
    validate_bundle_components(
        &bundle.proposal_batches,
        &bundle.verification_reports,
        &bundle.refinement_batches,
        &bundle.arbiter_decision,
        &bundle.final_result,
    )?;

    let recomputed = SolverTraceBundle::build(
        bundle.manifest.seed_bundle_digest.clone(),
        bundle.manifest.solver_manifest_digest.clone(),
        bundle.proposal_batches.clone(),
        bundle.verification_reports.clone(),
        bundle.refinement_batches.clone(),
        bundle.arbiter_decision.clone(),
        bundle.final_result.clone(),
    )?;
    if bundle.manifest != recomputed.manifest {
        return Err(TraceBundleError::ManifestDrift);
    }

    Ok(TraceBundleReplayReport {
        bundle_id: bundle.manifest.bundle_id.clone(),
        proposal_batch_count: bundle.proposal_batches.len(),
        verification_report_count: bundle.verification_reports.len(),
        refinement_batch_count: bundle.refinement_batches.len(),
        selected_hypothesis: bundle.arbiter_decision.selected_hypothesis.clone(),
    })
}

pub fn write_trace_bundle_json_file(
    path: impl AsRef<Path>,
    bundle: &SolverTraceBundle,
) -> Result<(), TraceBundleError> {
    let json = serde_json::to_string_pretty(bundle)?;
    fs::write(path, json).map_err(TraceBundleError::Io)
}

pub fn read_trace_bundle_json_file(
    path: impl AsRef<Path>,
) -> Result<SolverTraceBundle, TraceBundleError> {
    let json = fs::read_to_string(path).map_err(TraceBundleError::Io)?;
    serde_json::from_str(&json).map_err(TraceBundleError::Serde)
}

#[derive(Debug, Error)]
pub enum TraceBundleError {
    #[error("failed to compute ARC trace-bundle digest: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to serialize or deserialize ARC trace bundle: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("failed to read or write ARC trace bundle file: {0}")]
    Io(#[source] std::io::Error),
    #[error("invalid lane batch: {0}")]
    InvalidLaneBatch(&'static str),
    #[error("invalid arbiter decision: {0}")]
    InvalidArbiterDecision(&'static str),
    #[error("trace bundle components do not agree on the task id")]
    TaskIdMismatch,
    #[error("trace bundle contains duplicate trace locators")]
    DuplicateTraceLocator,
    #[error("trace bundle selected a hypothesis that was not verified")]
    SelectedHypothesisNotVerified,
    #[error("trace bundle ranked a hypothesis that was never proposed or refined")]
    RankedHypothesisMissing,
    #[error("trace bundle budget summary drifted: expected {expected:?}, got {actual:?}")]
    BudgetSummaryMismatch {
        expected: BudgetCounterSummary,
        actual: BudgetCounterSummary,
    },
    #[error("trace bundle manifest drifted from the replayed records")]
    ManifestDrift,
    #[error("trace bundle budget accounting overflowed: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("trace bundle solve envelope was invalid: {0}")]
    SolveEnvelope(#[from] SolveAttemptEnvelopeError),
}

fn validate_bundle_components(
    proposal_batches: &[LaneProposalBatch],
    verification_reports: &[VerificationReport],
    refinement_batches: &[LaneProposalBatch],
    arbiter_decision: &ArbiterDecision,
    final_result: &SolveAttemptEnvelope,
) -> Result<(), TraceBundleError> {
    for batch in proposal_batches.iter().chain(refinement_batches.iter()) {
        LaneProposalBatch::new(
            batch.lane_id.clone(),
            batch.phase,
            batch.status,
            batch.proposals.clone(),
            batch.refusal.clone(),
            batch.trace_locator.clone(),
            batch.budget_delta,
        )?;
    }
    ArbiterDecision::new(
        arbiter_decision.task_id.clone(),
        arbiter_decision.attempt_index,
        arbiter_decision.selected_hypothesis.clone(),
        arbiter_decision.ranked_hypotheses.clone(),
        arbiter_decision.second_attempt_allowed,
        arbiter_decision.refusal.clone(),
        arbiter_decision.decision_reason.clone(),
        arbiter_decision.trace_locator.clone(),
        arbiter_decision.budget_delta,
    )?;
    SolveAttemptEnvelope::new(
        final_result.task_id.clone(),
        final_result.attempt_index,
        final_result.task_budget,
        final_result.status,
        final_result.selected_answer.clone(),
        final_result.selected_lane.clone(),
        final_result.confidence,
        final_result.verification_summary,
        final_result.budget_summary,
        final_result.trace_digest.clone(),
        final_result.trace_locator.clone(),
        final_result.seed_bundle_digest.clone(),
        final_result.solver_manifest_digest.clone(),
        final_result.refusal.clone(),
    )?;

    let mut trace_locators = BTreeSet::new();
    for locator in proposal_batches
        .iter()
        .map(|batch| &batch.trace_locator)
        .chain(
            verification_reports
                .iter()
                .map(|report| &report.trace_locator),
        )
        .chain(refinement_batches.iter().map(|batch| &batch.trace_locator))
        .chain(std::iter::once(&arbiter_decision.trace_locator))
    {
        if !trace_locators.insert(locator.as_str().to_owned()) {
            return Err(TraceBundleError::DuplicateTraceLocator);
        }
    }

    if arbiter_decision.task_id != final_result.task_id {
        return Err(TraceBundleError::TaskIdMismatch);
    }

    let mut hypothesis_index = BTreeMap::new();
    for proposal in proposal_batches
        .iter()
        .flat_map(|batch| batch.proposals.iter())
        .chain(
            refinement_batches
                .iter()
                .flat_map(|batch| batch.proposals.iter()),
        )
    {
        hypothesis_index.insert(
            proposal.hypothesis.id.clone(),
            proposal.hypothesis.candidate_identity.clone(),
        );
    }

    let verified = verification_reports
        .iter()
        .map(|report| report.hypothesis_id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(selected) = &arbiter_decision.selected_hypothesis {
        if !verified.contains(selected) {
            return Err(TraceBundleError::SelectedHypothesisNotVerified);
        }
    }
    for ranked in &arbiter_decision.ranked_hypotheses {
        if !hypothesis_index.contains_key(ranked) {
            return Err(TraceBundleError::RankedHypothesisMissing);
        }
    }

    Ok(())
}

fn summarize_bundle_budget(
    proposal_batches: &[LaneProposalBatch],
    verification_reports: &[VerificationReport],
    refinement_batches: &[LaneProposalBatch],
    arbiter_decision: &ArbiterDecision,
) -> Result<BudgetCounterSummary, TraceBundleError> {
    let mut summary = BudgetCounterSummary::default();
    for delta in proposal_batches
        .iter()
        .map(|batch| batch.budget_delta)
        .chain(
            verification_reports
                .iter()
                .map(|report| report.budget_delta),
        )
        .chain(refinement_batches.iter().map(|batch| batch.budget_delta))
        .chain(std::iter::once(arbiter_decision.budget_delta))
    {
        summary = summary.checked_add(delta)?;
    }
    Ok(summary)
}

#[derive(Serialize)]
struct TraceBundleIdInput<'a> {
    task_id: &'a ArcTaskId,
    seed_bundle_digest: &'a ArcDigest,
    solver_manifest_digest: &'a ArcDigest,
    proposal_locators: &'a [TraceLocator],
    verification_locators: &'a [TraceLocator],
    refinement_locators: &'a [TraceLocator],
    arbiter_trace_locator: &'a TraceLocator,
    budget_ledger_digest: &'a ArcDigest,
    final_result_digest: &'a ArcDigest,
}
