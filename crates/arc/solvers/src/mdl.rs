use std::cmp::Ordering;
use std::collections::BTreeSet;

use arc_core::{ArcGrid, ArcGridError, CanonicalTask, TraceLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, BudgetCounterDelta, BudgetLedger, BudgetLedgerError,
    CandidateDeduplicationStatus, CandidateDeduplicator, CandidateIdentity,
    CandidateIdentityError, Hypothesis, HypothesisError, HypothesisId, HypothesisKind,
    LaneBatchStatus, LaneProposalBatch, ProposalPhase, RefusalEnvelope, SolverLaneId, SolverPhase,
    SolverRefusalCode, TaskBudget, TracedLaneProposal,
};

/// Ownership summary for the MDL/compression lane.
pub const MDL_LANE_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns the task-local MDL/compression lane, representation scoring, and simplicity-aware ranking signal";

pub const MDL_LANE_ID: &str = "mdl_compression";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcMdlInitializationMode {
    NoPretraining,
    WarmStartedPriors,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcMdlLaneConfig {
    pub initialization_mode: ArcMdlInitializationMode,
    pub max_candidates: usize,
}

impl Default for ArcMdlLaneConfig {
    fn default() -> Self {
        Self {
            initialization_mode: ArcMdlInitializationMode::NoPretraining,
            max_candidates: 6,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcMdlRepresentation {
    FillFromInputShape { color: u8 },
    FillFixedShape { width: u8, height: u8, color: u8 },
    ReuseTrainOutput { pair_index: u16 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcMdlCandidateReport {
    pub hypothesis_id: HypothesisId,
    pub representation: ArcMdlRepresentation,
    pub model_bits: u32,
    pub residual_bits: u32,
    pub solution_bits: u32,
    pub total_description_length_bits: u32,
    pub train_fit_ratio: f32,
    pub simplicity_score: f32,
    pub exact_train_fit: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcMdlLaneRun {
    pub initialization_mode: ArcMdlInitializationMode,
    pub proposal_batch: LaneProposalBatch,
    pub refinement_batch: LaneProposalBatch,
    pub candidate_reports: Vec<ArcMdlCandidateReport>,
}

impl ArcMdlLaneRun {
    #[must_use]
    pub fn best_hypothesis(&self) -> Option<&Hypothesis> {
        self.proposal_batch
            .proposals
            .first()
            .map(|proposal| &proposal.hypothesis)
    }
}

#[derive(Clone, Debug)]
pub struct ArcMdlLane {
    config: ArcMdlLaneConfig,
}

impl ArcMdlLane {
    #[must_use]
    pub fn new(config: ArcMdlLaneConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &ArcMdlLaneConfig {
        &self.config
    }

    pub fn run(
        &self,
        task: &CanonicalTask,
        budget: TaskBudget,
    ) -> Result<ArcMdlLaneRun, ArcMdlLaneError> {
        let lane_id = SolverLaneId::new(MDL_LANE_ID)?;
        let proposal_trace = TraceLocator::new(format!(
            "trace://arc-solvers/mdl/{}/proposal",
            task.raw.id.as_str()
        ))?;
        let refinement_trace = TraceLocator::new(format!(
            "trace://arc-solvers/mdl/{}/refinement",
            task.raw.id.as_str()
        ))?;

        if task.normalized_test_inputs.len() != 1 {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::UnsupportedTask,
                SolverPhase::Propose,
                "bounded MDL lane currently supports exactly one static test input",
            )?;
            return Ok(ArcMdlLaneRun {
                initialization_mode: self.config.initialization_mode,
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
                candidate_reports: Vec::new(),
            });
        }

        let minimum_train_pair_execs = task.normalized_train.len() as u32;
        if budget.max_candidates == 0
            || self.config.max_candidates == 0
            || budget.max_train_pair_execs < minimum_train_pair_execs
        {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::MinimumBudgetNotMet,
                SolverPhase::Propose,
                format!(
                    "MDL lane requires at least one candidate slot and {} train-pair executions",
                    minimum_train_pair_execs
                ),
            )?;
            return Ok(ArcMdlLaneRun {
                initialization_mode: self.config.initialization_mode,
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
                candidate_reports: Vec::new(),
            });
        }

        let mut ledger = BudgetLedger::new(budget);
        let mut deduplicator = CandidateDeduplicator::new();
        let mut proposals = Vec::new();
        let mut reports = Vec::new();
        let mut budget_exhausted = false;
        let max_candidates = self.config.max_candidates.min(budget.max_candidates as usize);
        let representations = generate_representations(task);

        for (index, representation) in representations.into_iter().enumerate() {
            if proposals.len() >= max_candidates {
                break;
            }
            let delta = BudgetCounterDelta {
                candidates_generated: 1,
                train_pair_execs: task.normalized_train.len() as u32,
                ..BudgetCounterDelta::default()
            };
            if ledger.apply(delta).is_err() {
                budget_exhausted = true;
                break;
            }

            let candidate = evaluate_representation(
                task,
                self.config.initialization_mode,
                &representation,
            )?;
            let local_score =
                0.6 * candidate.scoring.simplicity_score + 0.4 * candidate.scoring.train_fit_ratio;
            let hypothesis = Hypothesis::new(
                HypothesisKind::StaticAnswer,
                lane_id.clone(),
                0,
                CandidateIdentity::new(
                    HypothesisKind::StaticAnswer,
                    None,
                    Some(&candidate.predicted_test_output),
                    None,
                    None,
                )?,
                None,
                Some(candidate.predicted_test_output.clone()),
                None,
                local_score,
                TraceLocator::new(format!(
                    "trace://arc-solvers/mdl/{}/candidate-{}",
                    task.raw.id.as_str(),
                    index
                ))?,
                ledger.used().into(),
            )?;

            match deduplicator.record(&hypothesis).status {
                CandidateDeduplicationStatus::Accepted => {
                    proposals.push(TracedLaneProposal {
                        hypothesis: hypothesis.clone(),
                        local_rank: 0,
                        rationale_digest: ArcDigest::from_serializable(&candidate.scoring)?,
                    });
                    reports.push(ArcMdlCandidateReport {
                        hypothesis_id: hypothesis.id.clone(),
                        representation: candidate.scoring.representation,
                        model_bits: candidate.scoring.model_bits,
                        residual_bits: candidate.scoring.residual_bits,
                        solution_bits: candidate.scoring.solution_bits,
                        total_description_length_bits: candidate
                            .scoring
                            .total_description_length_bits,
                        train_fit_ratio: candidate.scoring.train_fit_ratio,
                        simplicity_score: candidate.scoring.simplicity_score,
                        exact_train_fit: candidate.scoring.exact_train_fit,
                    });
                }
                CandidateDeduplicationStatus::Duplicate { .. } => {}
            }
        }

        sort_candidates(&mut proposals, &mut reports);
        for (index, proposal) in proposals.iter_mut().enumerate() {
            proposal.local_rank = index as u32;
        }

        let proposal_status = if !proposals.is_empty() {
            LaneBatchStatus::Proposed
        } else if budget_exhausted {
            LaneBatchStatus::BudgetExhausted
        } else {
            LaneBatchStatus::Empty
        };
        let proposal_batch = LaneProposalBatch::new(
            lane_id.clone(),
            ProposalPhase::Propose,
            proposal_status,
            proposals,
            None,
            proposal_trace,
            ledger.used().into(),
        )?;
        let refinement_batch = LaneProposalBatch::new(
            lane_id,
            ProposalPhase::Refine,
            LaneBatchStatus::Empty,
            Vec::new(),
            None,
            refinement_trace,
            BudgetCounterDelta::default(),
        )?;

        Ok(ArcMdlLaneRun {
            initialization_mode: self.config.initialization_mode,
            proposal_batch,
            refinement_batch,
            candidate_reports: reports,
        })
    }
}

#[derive(Debug, Error)]
pub enum ArcMdlLaneError {
    #[error("failed to compute MDL digests: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to create MDL lane ids or refusals: {0}")]
    SolverId(#[from] crate::SolverIdError),
    #[error("failed to create MDL lane refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
    #[error("failed to create MDL lane hypothesis: {0}")]
    Hypothesis(#[from] HypothesisError),
    #[error("failed to create MDL lane candidate identity: {0}")]
    CandidateIdentity(#[from] CandidateIdentityError),
    #[error("MDL lane budget accounting failed: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("failed to build an MDL candidate grid: {0}")]
    Grid(#[from] ArcGridError),
    #[error("failed to create MDL lane trace locator: {0}")]
    TraceLocator(#[from] arc_core::TraceLocatorError),
    #[error("failed to build MDL lane proposal batches: {0}")]
    Trace(#[from] crate::TraceBundleError),
}

struct EvaluatedCandidate {
    scoring: CandidateScoring,
    predicted_test_output: ArcGrid,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct CandidateScoring {
    representation: ArcMdlRepresentation,
    model_bits: u32,
    residual_bits: u32,
    solution_bits: u32,
    total_description_length_bits: u32,
    train_fit_ratio: f32,
    simplicity_score: f32,
    exact_train_fit: bool,
}

fn generate_representations(task: &CanonicalTask) -> Vec<ArcMdlRepresentation> {
    let mut colors = BTreeSet::new();
    let mut output_shapes = BTreeSet::new();
    for pair in &task.normalized_train {
        output_shapes.insert((pair.output.grid.width(), pair.output.grid.height()));
        for color in pair.output.grid.cells() {
            colors.insert(*color);
        }
    }

    let mut representations = Vec::new();
    for color in &colors {
        representations.push(ArcMdlRepresentation::FillFromInputShape { color: *color });
    }
    for &(width, height) in &output_shapes {
        for color in &colors {
            representations.push(ArcMdlRepresentation::FillFixedShape {
                width,
                height,
                color: *color,
            });
        }
    }
    for index in 0..task.normalized_train.len() {
        representations.push(ArcMdlRepresentation::ReuseTrainOutput {
            pair_index: index as u16,
        });
    }

    representations
}

fn evaluate_representation(
    task: &CanonicalTask,
    initialization_mode: ArcMdlInitializationMode,
    representation: &ArcMdlRepresentation,
) -> Result<EvaluatedCandidate, ArcMdlLaneError> {
    let mut residual_bits = 0_u32;
    let mut matched_cells = 0_u32;
    let mut total_cells = 0_u32;
    let mut exact_train_fit = true;

    for pair in &task.normalized_train {
        let predicted = render_representation(task, representation, &pair.input.grid)?;
        let comparison = compare_grids(&predicted, &pair.output.grid);
        residual_bits = residual_bits.saturating_add(comparison.residual_bits);
        matched_cells = matched_cells.saturating_add(comparison.matched_cells);
        total_cells = total_cells.saturating_add(comparison.total_cells);
        exact_train_fit &= comparison.exact_match;
    }

    let predicted_test_output = render_representation(
        task,
        representation,
        &task.normalized_test_inputs[0].grid,
    )?;
    let model_bits = model_bits(initialization_mode, representation, task);
    let solution_bits = predicted_test_output.cells().len() as u32 * 2;
    let total_description_length_bits = model_bits
        .saturating_add(residual_bits)
        .saturating_add(solution_bits);
    let simplicity_score = 1.0 / (1.0 + total_description_length_bits as f32);
    let train_fit_ratio = if total_cells == 0 {
        0.0
    } else {
        matched_cells as f32 / total_cells as f32
    };

    Ok(EvaluatedCandidate {
        scoring: CandidateScoring {
            representation: representation.clone(),
            model_bits,
            residual_bits,
            solution_bits,
            total_description_length_bits,
            train_fit_ratio,
            simplicity_score,
            exact_train_fit,
        },
        predicted_test_output,
    })
}

fn render_representation(
    task: &CanonicalTask,
    representation: &ArcMdlRepresentation,
    input: &ArcGrid,
) -> Result<ArcGrid, ArcMdlLaneError> {
    match representation {
        ArcMdlRepresentation::FillFromInputShape { color } => {
            Ok(filled_grid(input.width(), input.height(), *color)?)
        }
        ArcMdlRepresentation::FillFixedShape {
            width,
            height,
            color,
        } => Ok(filled_grid(*width, *height, *color)?),
        ArcMdlRepresentation::ReuseTrainOutput { pair_index } => Ok(task.normalized_train
            [usize::from(*pair_index)]
        .output
        .grid
        .clone()),
    }
}

fn filled_grid(width: u8, height: u8, color: u8) -> Result<ArcGrid, ArcMdlLaneError> {
    let cells = vec![color; usize::from(width) * usize::from(height)];
    Ok(ArcGrid::new(width, height, cells)?)
}

fn model_bits(
    initialization_mode: ArcMdlInitializationMode,
    representation: &ArcMdlRepresentation,
    task: &CanonicalTask,
) -> u32 {
    let base_bits = match representation {
        ArcMdlRepresentation::FillFromInputShape { .. } => 8,
        ArcMdlRepresentation::FillFixedShape { .. } => 16,
        ArcMdlRepresentation::ReuseTrainOutput { pair_index } => {
            let grid = &task.normalized_train[usize::from(*pair_index)].output.grid;
            12 + grid.cells().len() as u32 * 4
        }
    };

    match (initialization_mode, representation) {
        (ArcMdlInitializationMode::WarmStartedPriors, ArcMdlRepresentation::FillFromInputShape { .. })
        | (ArcMdlInitializationMode::WarmStartedPriors, ArcMdlRepresentation::FillFixedShape { .. }) => {
            base_bits.saturating_sub(2)
        }
        _ => base_bits,
    }
}

struct GridComparison {
    residual_bits: u32,
    matched_cells: u32,
    total_cells: u32,
    exact_match: bool,
}

fn compare_grids(predicted: &ArcGrid, expected: &ArcGrid) -> GridComparison {
    let total_cells = expected.cells().len() as u32;
    if predicted.width() != expected.width() || predicted.height() != expected.height() {
        let area_difference =
            predicted.cells().len().abs_diff(expected.cells().len()) as u32;
        return GridComparison {
            residual_bits: 32 + area_difference * 4,
            matched_cells: 0,
            total_cells,
            exact_match: false,
        };
    }

    let mut mismatches = 0_u32;
    let mut matched = 0_u32;
    for (left, right) in predicted.cells().iter().zip(expected.cells().iter()) {
        if left == right {
            matched += 1;
        } else {
            mismatches += 1;
        }
    }

    GridComparison {
        residual_bits: mismatches * 4,
        matched_cells: matched,
        total_cells,
        exact_match: mismatches == 0,
    }
}

fn sort_candidates(
    proposals: &mut Vec<TracedLaneProposal>,
    reports: &mut Vec<ArcMdlCandidateReport>,
) {
    let mut combined = proposals
        .drain(..)
        .zip(reports.drain(..))
        .collect::<Vec<_>>();
    combined.sort_by(|(left_proposal, left_report), (right_proposal, right_report)| {
        left_report
            .total_description_length_bits
            .cmp(&right_report.total_description_length_bits)
            .then_with(|| {
                right_report
                    .train_fit_ratio
                    .partial_cmp(&left_report.train_fit_ratio)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| {
                right_proposal
                    .hypothesis
                    .local_score
                    .partial_cmp(&left_proposal.hypothesis.local_score)
                    .unwrap_or(Ordering::Equal)
            })
    });

    for (proposal, report) in combined {
        proposals.push(proposal);
        reports.push(report);
    }
}
