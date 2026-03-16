use std::cmp::Ordering;

use arc_core::{ArcGrid, CanonicalTask, TraceLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, BudgetCounterDelta, BudgetCounterSummary, BudgetLedger,
    BudgetLedgerError, CandidateDeduplicationStatus, CandidateDeduplicator, CandidateIdentity,
    CandidateIdentityError, Hypothesis, HypothesisError, HypothesisKind, LaneBatchStatus,
    LaneProposalBatch, ProposalPhase, RefusalEnvelope, SolverLaneId, SolverPhase,
    SolverRefusalCode, TaskBudget, TracedLaneProposal,
};

/// Ownership summary for the recursive tiny-model lane.
pub const RECURSIVE_TINY_MODEL_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns the recursive tiny-model lane interface, bounded step tracing, and ARC-specific answer-state policy";

pub const RECURSIVE_TINY_MODEL_LANE_ID: &str = "recursive_tiny_model";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcRecursiveTinyModelBootstrapMode {
    TinyCheckpoint,
    Scratch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelConfig {
    pub bootstrap_mode: ArcRecursiveTinyModelBootstrapMode,
    pub max_recursive_steps: usize,
    pub max_test_time_updates: usize,
    pub max_returned_proposals: usize,
    pub halt_threshold: f32,
}

impl Default for ArcRecursiveTinyModelConfig {
    fn default() -> Self {
        Self {
            bootstrap_mode: ArcRecursiveTinyModelBootstrapMode::TinyCheckpoint,
            max_recursive_steps: 4,
            max_test_time_updates: 0,
            max_returned_proposals: 4,
            halt_threshold: 0.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelState<S> {
    pub latent_state: S,
    pub answer_grid: ArcGrid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelBootstrap<S> {
    pub state: ArcRecursiveTinyModelState<S>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelStepOutput<S> {
    pub state: ArcRecursiveTinyModelState<S>,
    pub halt_score: f32,
    pub continue_score: f32,
    pub note: String,
}

pub trait ArcTinyModel {
    type Error: std::fmt::Display;
    type LatentState: Clone + Serialize;

    fn initialize(
        &mut self,
        task: &CanonicalTask,
        mode: ArcRecursiveTinyModelBootstrapMode,
    ) -> Result<ArcRecursiveTinyModelBootstrap<Self::LatentState>, Self::Error>;

    fn test_time_update(
        &mut self,
        _task: &CanonicalTask,
        _state: &mut ArcRecursiveTinyModelState<Self::LatentState>,
    ) -> Result<Option<String>, Self::Error> {
        Ok(None)
    }

    fn step(
        &mut self,
        task: &CanonicalTask,
        state: &ArcRecursiveTinyModelState<Self::LatentState>,
        step_index: u32,
    ) -> Result<ArcRecursiveTinyModelStepOutput<Self::LatentState>, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcRecursiveTinyModelTracePhase {
    Bootstrap,
    TestTimeUpdate,
    RecursiveStep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcRecursiveTinyModelStepDecision {
    Continue,
    Halt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelStepTrace {
    pub phase: ArcRecursiveTinyModelTracePhase,
    pub step_index: u32,
    pub latent_state_digest: ArcDigest,
    pub answer_state_digest: ArcDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halt_score: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continue_score: Option<f32>,
    pub answer_snapshot: ArcGrid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<ArcRecursiveTinyModelStepDecision>,
    pub note: String,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRecursiveTinyModelLaneRun {
    pub bootstrap_mode: ArcRecursiveTinyModelBootstrapMode,
    pub ttt_updates_applied: u32,
    pub proposal_batch: LaneProposalBatch,
    pub refinement_batch: LaneProposalBatch,
    pub step_traces: Vec<ArcRecursiveTinyModelStepTrace>,
}

impl ArcRecursiveTinyModelLaneRun {
    #[must_use]
    pub fn best_hypothesis(&self) -> Option<&Hypothesis> {
        self.refinement_batch
            .proposals
            .first()
            .or_else(|| self.proposal_batch.proposals.first())
            .map(|proposal| &proposal.hypothesis)
    }
}

pub struct ArcRecursiveTinyModelLane<M> {
    model: M,
    config: ArcRecursiveTinyModelConfig,
}

impl<M> ArcRecursiveTinyModelLane<M> {
    #[must_use]
    pub fn new(model: M, config: ArcRecursiveTinyModelConfig) -> Self {
        Self { model, config }
    }
}

impl<M> ArcRecursiveTinyModelLane<M>
where
    M: ArcTinyModel,
{
    pub fn run(
        &mut self,
        task: &CanonicalTask,
        budget: TaskBudget,
    ) -> Result<ArcRecursiveTinyModelLaneRun, ArcRecursiveTinyModelLaneError> {
        let lane_id = SolverLaneId::new(RECURSIVE_TINY_MODEL_LANE_ID)?;
        let proposal_trace =
            phase_trace_locator(task, ArcRecursiveTinyModelTracePhase::Bootstrap, 0)?;
        let refinement_trace = TraceLocator::new(format!(
            "trace://arc-solvers/recursive-tiny-model/{}/refinement",
            task.raw.id.as_str()
        ))?;

        if budget.max_candidates == 0
            || budget.max_model_forward_calls == 0
            || budget.max_refinement_steps == 0
            || self.config.max_recursive_steps == 0
            || self.config.max_returned_proposals == 0
        {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::MinimumBudgetNotMet,
                SolverPhase::Propose,
                "recursive tiny-model lane requires at least one candidate slot, one model forward call, one refinement step, and one returned proposal slot",
            )?;
            return Ok(ArcRecursiveTinyModelLaneRun {
                bootstrap_mode: self.config.bootstrap_mode,
                ttt_updates_applied: 0,
                proposal_batch: LaneProposalBatch::new(
                    lane_id.clone(),
                    ProposalPhase::Propose,
                    LaneBatchStatus::Refused,
                    Vec::new(),
                    Some(refusal),
                    proposal_trace,
                    BudgetCounterDelta::default(),
                )?,
                refinement_batch: LaneProposalBatch::new(
                    lane_id,
                    ProposalPhase::Refine,
                    LaneBatchStatus::Empty,
                    Vec::new(),
                    None,
                    refinement_trace,
                    BudgetCounterDelta::default(),
                )?,
                step_traces: Vec::new(),
            });
        }

        validate_probability(self.config.halt_threshold, "halt threshold")?;

        let mut ledger = BudgetLedger::new(budget);
        let bootstrap = self
            .model
            .initialize(task, self.config.bootstrap_mode)
            .map_err(|error| ArcRecursiveTinyModelLaneError::Runner(error.to_string()))?;
        let mut state = bootstrap.state;
        let mut step_traces = vec![trace_snapshot(
            &state,
            ArcRecursiveTinyModelTracePhase::Bootstrap,
            0,
            None,
            None,
            None,
            bootstrap.note,
            phase_trace_locator(task, ArcRecursiveTinyModelTracePhase::Bootstrap, 0)?,
            BudgetCounterDelta::default(),
        )?];
        ledger.apply(BudgetCounterDelta {
            candidates_generated: 1,
            ..BudgetCounterDelta::default()
        })?;

        let bootstrap_hypothesis = hypothesis_from_state(
            task,
            &lane_id,
            &state,
            0.45 * shape_score(task, &state.answer_grid),
            "bootstrap",
            0,
            ledger.used().into(),
        )?;
        let bootstrap_proposal = TracedLaneProposal {
            rationale_digest: ArcDigest::from_serializable(&step_traces[0])?,
            local_rank: 0,
            hypothesis: bootstrap_hypothesis,
        };
        let proposal_batch = LaneProposalBatch::new(
            lane_id.clone(),
            ProposalPhase::Propose,
            LaneBatchStatus::Proposed,
            vec![bootstrap_proposal.clone()],
            None,
            proposal_trace,
            ledger.used().into(),
        )?;

        let mut deduplicator = CandidateDeduplicator::new();
        deduplicator.record(&bootstrap_proposal.hypothesis);

        let mut ttt_updates_applied = 0_u32;
        let allowed_ttt_updates = self
            .config
            .max_test_time_updates
            .min(budget.max_ttt_updates as usize);
        for update_index in 0..allowed_ttt_updates {
            let update_note = self
                .model
                .test_time_update(task, &mut state)
                .map_err(|error| ArcRecursiveTinyModelLaneError::Runner(error.to_string()))?;
            let Some(note) = update_note else {
                break;
            };
            let delta = BudgetCounterDelta {
                ttt_updates: 1,
                ..BudgetCounterDelta::default()
            };
            if ledger.apply(delta).is_err() {
                break;
            }
            ttt_updates_applied += 1;
            step_traces.push(trace_snapshot(
                &state,
                ArcRecursiveTinyModelTracePhase::TestTimeUpdate,
                update_index as u32,
                None,
                None,
                None,
                note,
                phase_trace_locator(
                    task,
                    ArcRecursiveTinyModelTracePhase::TestTimeUpdate,
                    update_index as u32,
                )?,
                delta,
            )?);
        }

        let steps_allowed = self
            .config
            .max_recursive_steps
            .min(budget.max_refinement_steps as usize)
            .min(budget.max_model_forward_calls as usize);
        let mut refined_candidates = Vec::new();
        let mut budget_exhausted = false;

        for step_index in 0..steps_allowed {
            let step_delta = BudgetCounterDelta {
                refinement_steps: 1,
                model_forward_calls: 1,
                ..BudgetCounterDelta::default()
            };
            if ledger.apply(step_delta).is_err() {
                budget_exhausted = true;
                break;
            }

            let step = self
                .model
                .step(task, &state, step_index as u32)
                .map_err(|error| ArcRecursiveTinyModelLaneError::Runner(error.to_string()))?;
            validate_probability(step.halt_score, "halt score")?;
            validate_probability(step.continue_score, "continue score")?;
            let decision = if step.halt_score >= self.config.halt_threshold
                || step.halt_score >= step.continue_score
            {
                ArcRecursiveTinyModelStepDecision::Halt
            } else {
                ArcRecursiveTinyModelStepDecision::Continue
            };
            state = step.state;
            step_traces.push(trace_snapshot(
                &state,
                ArcRecursiveTinyModelTracePhase::RecursiveStep,
                step_index as u32,
                Some(step.halt_score),
                Some(step.continue_score),
                Some(decision),
                step.note,
                phase_trace_locator(
                    task,
                    ArcRecursiveTinyModelTracePhase::RecursiveStep,
                    step_index as u32,
                )?,
                step_delta,
            )?);

            let local_score =
                0.65 * step.halt_score + 0.35 * shape_score(task, &state.answer_grid);
            let candidate_delta = BudgetCounterDelta {
                candidates_generated: 1,
                ..BudgetCounterDelta::default()
            };
            let mut candidate = hypothesis_from_state(
                task,
                &lane_id,
                &state,
                local_score,
                "step",
                step_index as u32,
                delta_between(ledger.used(), proposal_batch.budget_delta),
            )?;

            let deduplication = deduplicator.record(&candidate);
            match deduplication.status {
                CandidateDeduplicationStatus::Accepted => {
                    if ledger.apply(candidate_delta).is_err() {
                        budget_exhausted = true;
                        break;
                    }
                    candidate.budget_delta = delta_between(ledger.used(), proposal_batch.budget_delta);
                    refined_candidates.push(TracedLaneProposal {
                        rationale_digest: ArcDigest::from_serializable(
                            step_traces.last().expect("step trace exists"),
                        )?,
                        local_rank: 0,
                        hypothesis: candidate,
                    });
                }
                CandidateDeduplicationStatus::Duplicate { .. } => {}
            }

            if decision == ArcRecursiveTinyModelStepDecision::Halt {
                break;
            }
        }

        sort_candidates(&mut refined_candidates);
        for (index, proposal) in refined_candidates.iter_mut().enumerate() {
            proposal.local_rank = index as u32;
        }
        refined_candidates.truncate(self.config.max_returned_proposals);

        let refinement_status = if !refined_candidates.is_empty() {
            LaneBatchStatus::Proposed
        } else if budget_exhausted {
            LaneBatchStatus::BudgetExhausted
        } else {
            LaneBatchStatus::Empty
        };
        let refinement_batch = LaneProposalBatch::new(
            lane_id,
            ProposalPhase::Refine,
            refinement_status,
            refined_candidates,
            None,
            refinement_trace,
            delta_between(ledger.used(), proposal_batch.budget_delta),
        )?;

        Ok(ArcRecursiveTinyModelLaneRun {
            bootstrap_mode: self.config.bootstrap_mode,
            ttt_updates_applied,
            proposal_batch,
            refinement_batch,
            step_traces,
        })
    }
}

#[derive(Debug, Error)]
pub enum ArcRecursiveTinyModelLaneError {
    #[error("failed to compute recursive tiny-model digests: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to create recursive tiny-model ids or refusals: {0}")]
    SolverId(#[from] crate::SolverIdError),
    #[error("failed to create recursive tiny-model refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
    #[error("failed to create recursive tiny-model hypothesis: {0}")]
    Hypothesis(#[from] HypothesisError),
    #[error("failed to create recursive tiny-model candidate identity: {0}")]
    CandidateIdentity(#[from] CandidateIdentityError),
    #[error("recursive tiny-model lane budget accounting failed: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("failed to create recursive tiny-model trace locator: {0}")]
    TraceLocator(#[from] arc_core::TraceLocatorError),
    #[error("failed to build recursive tiny-model proposal batches: {0}")]
    Trace(#[from] crate::TraceBundleError),
    #[error("recursive tiny-model runner failed: {0}")]
    Runner(String),
    #[error("recursive tiny-model score must be finite and between 0 and 1, got {score} for {label}")]
    InvalidScore { label: &'static str, score: f32 },
}

fn validate_probability(
    score: f32,
    label: &'static str,
) -> Result<(), ArcRecursiveTinyModelLaneError> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(ArcRecursiveTinyModelLaneError::InvalidScore { label, score });
    }
    Ok(())
}

fn hypothesis_from_state<S>(
    task: &CanonicalTask,
    lane_id: &SolverLaneId,
    state: &ArcRecursiveTinyModelState<S>,
    local_score: f32,
    label: &str,
    index: u32,
    budget_delta: BudgetCounterDelta,
) -> Result<Hypothesis, ArcRecursiveTinyModelLaneError>
where
    S: Clone + Serialize,
{
    Ok(Hypothesis::new(
        HypothesisKind::StaticAnswer,
        lane_id.clone(),
        0,
        CandidateIdentity::new(
            HypothesisKind::StaticAnswer,
            None,
            Some(&state.answer_grid),
            None,
            None,
        )?,
        None,
        Some(state.answer_grid.clone()),
        None,
        local_score,
        TraceLocator::new(format!(
            "trace://arc-solvers/recursive-tiny-model/{}/{}-candidate-{}",
            task.raw.id.as_str(),
            label,
            index
        ))?,
        budget_delta,
    )?)
}

fn trace_snapshot<S>(
    state: &ArcRecursiveTinyModelState<S>,
    phase: ArcRecursiveTinyModelTracePhase,
    step_index: u32,
    halt_score: Option<f32>,
    continue_score: Option<f32>,
    decision: Option<ArcRecursiveTinyModelStepDecision>,
    note: String,
    trace_locator: TraceLocator,
    budget_delta: BudgetCounterDelta,
) -> Result<ArcRecursiveTinyModelStepTrace, ArcRecursiveTinyModelLaneError>
where
    S: Clone + Serialize,
{
    if note.trim().is_empty() {
        return Err(ArcRecursiveTinyModelLaneError::Runner(String::from(
            "recursive tiny-model trace notes must not be empty",
        )));
    }

    Ok(ArcRecursiveTinyModelStepTrace {
        phase,
        step_index,
        latent_state_digest: ArcDigest::from_serializable(&state.latent_state)?,
        answer_state_digest: ArcDigest::from_serializable(&state.answer_grid)?,
        halt_score,
        continue_score,
        answer_snapshot: state.answer_grid.clone(),
        decision,
        note,
        trace_locator,
        budget_delta,
    })
}

fn phase_trace_locator(
    task: &CanonicalTask,
    phase: ArcRecursiveTinyModelTracePhase,
    step_index: u32,
) -> Result<TraceLocator, arc_core::TraceLocatorError> {
    let phase_name = match phase {
        ArcRecursiveTinyModelTracePhase::Bootstrap => "bootstrap",
        ArcRecursiveTinyModelTracePhase::TestTimeUpdate => "ttt-update",
        ArcRecursiveTinyModelTracePhase::RecursiveStep => "recursive-step",
    };
    TraceLocator::new(format!(
        "trace://arc-solvers/recursive-tiny-model/{}/{}-{}",
        task.raw.id.as_str(),
        phase_name,
        step_index
    ))
}

fn shape_score(task: &CanonicalTask, candidate: &ArcGrid) -> f32 {
    let train_outputs = task
        .normalized_train
        .iter()
        .map(|pair| &pair.output.grid)
        .collect::<Vec<_>>();
    let matching_shapes = train_outputs
        .iter()
        .filter(|grid| grid.width() == candidate.width() && grid.height() == candidate.height())
        .count() as f32;
    0.2 + (matching_shapes / train_outputs.len().max(1) as f32) * 0.8
}

fn delta_between(
    newer: BudgetCounterSummary,
    older: BudgetCounterDelta,
) -> BudgetCounterDelta {
    BudgetCounterDelta {
        wall_ms: newer.wall_ms.saturating_sub(older.wall_ms),
        candidates_generated: newer
            .candidates_generated
            .saturating_sub(older.candidates_generated),
        verifier_evals: newer.verifier_evals.saturating_sub(older.verifier_evals),
        train_pair_execs: newer
            .train_pair_execs
            .saturating_sub(older.train_pair_execs),
        refinement_steps: newer
            .refinement_steps
            .saturating_sub(older.refinement_steps),
        model_forward_calls: newer
            .model_forward_calls
            .saturating_sub(older.model_forward_calls),
        ttt_updates: newer.ttt_updates.saturating_sub(older.ttt_updates),
        peak_memory_mb: newer.peak_memory_mb.saturating_sub(older.peak_memory_mb),
    }
}

fn sort_candidates(candidates: &mut [TracedLaneProposal]) {
    candidates.sort_by(|left, right| {
        right
            .hypothesis
            .local_score
            .partial_cmp(&left.hypothesis.local_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                left.hypothesis
                    .candidate_identity
                    .canonical_signature
                    .as_str()
                    .cmp(right.hypothesis.candidate_identity.canonical_signature.as_str())
            })
    });
}
