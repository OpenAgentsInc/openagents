use std::collections::BTreeSet;

use arc_core::{ArcGrid, CanonicalTask, TraceLocator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, ArcGridExpr, ArcInterpreter, ArcObjectSelector, ArcProgram,
    ArcProgramMetadata, ArcSymbol, BudgetCounterDelta, BudgetCounterSummary, BudgetLedger,
    BudgetLedgerError, CandidateDeduplicationStatus, CandidateDeduplicator, CandidateIdentity,
    CandidateIdentityError, Hypothesis, HypothesisError, HypothesisId, HypothesisKind,
    LaneBatchStatus, LaneProposalBatch, ProposalPhase, RefusalEnvelope, SolverLaneId, SolverPhase,
    SolverRefusalCode, TaskBudget, TracedLaneProposal,
};

/// Ownership summary for the symbolic induction lane.
pub const SYMBOLIC_LANE_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns the typed symbolic program-search lane and deterministic repair operators";

pub const SYMBOLIC_LANE_ID: &str = "symbolic";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolicLaneConfig {
    pub max_seed_programs: usize,
    pub max_returned_proposals: usize,
    pub max_repair_attempts: usize,
}

impl Default for SymbolicLaneConfig {
    fn default() -> Self {
        Self {
            max_seed_programs: 32,
            max_returned_proposals: 4,
            max_repair_attempts: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolicSeedTemplate {
    Identity,
    RotateQuarterTurns { quarter_turns: u8 },
    ReflectHorizontal,
    ReflectVertical,
    CropByColor { color: u8 },
    CropLargest,
    CropSmallest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolicOutputTransformRepair {
    RotateQuarterTurns { quarter_turns: u8 },
    ReflectHorizontal,
    ReflectVertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SymbolicRepairOperator {
    Recolor { from: u8, to: u8 },
    OutputTransform(SymbolicOutputTransformRepair),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolicRepairAttempt {
    pub source_hypothesis_id: HypothesisId,
    pub operator: SymbolicRepairOperator,
    pub refined_hypothesis: Hypothesis,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymbolicLaneRun {
    pub proposal_batch: LaneProposalBatch,
    pub refinement_batch: LaneProposalBatch,
    pub repair_attempts: Vec<SymbolicRepairAttempt>,
}

impl SymbolicLaneRun {
    #[must_use]
    pub fn best_hypothesis(&self) -> Option<&Hypothesis> {
        self.refinement_batch
            .proposals
            .first()
            .or_else(|| self.proposal_batch.proposals.first())
            .map(|proposal| &proposal.hypothesis)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SymbolicLane {
    config: SymbolicLaneConfig,
}

impl SymbolicLane {
    #[must_use]
    pub fn new(config: SymbolicLaneConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &SymbolicLaneConfig {
        &self.config
    }

    pub fn run(
        &self,
        task: &CanonicalTask,
        budget: TaskBudget,
    ) -> Result<SymbolicLaneRun, SymbolicLaneError> {
        let lane_id = SolverLaneId::new(SYMBOLIC_LANE_ID)?;
        let proposal_trace = proposal_trace_locator(task, "proposal")?;
        let refinement_trace = proposal_trace_locator(task, "refinement")?;
        let minimum_train_pair_execs = task.normalized_train.len() as u32;
        if budget.max_candidates == 0 || budget.max_train_pair_execs < minimum_train_pair_execs {
            let proposal_batch = LaneProposalBatch::new(
                lane_id.clone(),
                ProposalPhase::Propose,
                LaneBatchStatus::Refused,
                Vec::new(),
                Some(RefusalEnvelope::new(
                    SolverRefusalCode::MinimumBudgetNotMet,
                    SolverPhase::Propose,
                    format!(
                        "symbolic search requires at least 1 candidate slot and {} train-pair executions",
                        minimum_train_pair_execs
                    ),
                )?),
                proposal_trace,
                BudgetCounterDelta::default(),
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
            return Ok(SymbolicLaneRun {
                proposal_batch,
                refinement_batch,
                repair_attempts: Vec::new(),
            });
        }

        let mut ledger = BudgetLedger::new(budget);
        let mut deduplicator = CandidateDeduplicator::new();
        let mut evaluated = Vec::new();
        let mut budget_exhausted = false;
        for (index, seed) in generate_seed_programs(task)
            .into_iter()
            .take(self.config.max_seed_programs)
            .enumerate()
        {
            if evaluated.len() >= usize::try_from(budget.max_candidates).unwrap_or(usize::MAX) {
                break;
            }
            match evaluate_program_candidate(
                task,
                &lane_id,
                &mut deduplicator,
                &mut ledger,
                seed.program,
                index,
                false,
            ) {
                Ok(Some(candidate)) => evaluated.push(candidate),
                Ok(None) => {}
                Err(SymbolicLaneError::Budget(_)) => {
                    budget_exhausted = true;
                    break;
                }
                Err(other) => return Err(other),
            }
        }

        sort_candidates(&mut evaluated);
        let proposal_candidates = evaluated
            .iter()
            .take(self.config.max_returned_proposals)
            .cloned()
            .collect::<Vec<_>>();
        let proposal_batch = build_lane_batch(
            lane_id.clone(),
            ProposalPhase::Propose,
            proposal_trace,
            &proposal_candidates,
            None,
            budget_exhausted,
            ledger.used().into(),
        )?;
        let proposal_budget_summary = ledger.used();

        let mut repair_attempts = Vec::new();
        let mut refined = Vec::new();
        let mut repair_budget_exhausted = false;
        let mut attempted_repairs = 0_usize;
        for candidate in proposal_candidates.iter().cloned() {
            if attempted_repairs >= self.config.max_repair_attempts {
                break;
            }
            for operator in infer_repairs(&candidate) {
                if attempted_repairs >= self.config.max_repair_attempts {
                    break;
                }
                attempted_repairs += 1;
                let repaired_program = apply_repair(candidate.program.clone(), operator);
                match evaluate_program_candidate(
                    task,
                    &lane_id,
                    &mut deduplicator,
                    &mut ledger,
                    repaired_program,
                    attempted_repairs,
                    true,
                ) {
                    Ok(Some(refined_candidate)) => {
                        repair_attempts.push(SymbolicRepairAttempt {
                            source_hypothesis_id: candidate.hypothesis.id.clone(),
                            operator,
                            refined_hypothesis: refined_candidate.hypothesis.clone(),
                        });
                        refined.push(refined_candidate);
                    }
                    Ok(None) => {}
                    Err(SymbolicLaneError::Budget(_)) => {
                        repair_budget_exhausted = true;
                        break;
                    }
                    Err(other) => return Err(other),
                }
            }
        }

        sort_candidates(&mut refined);
        let refinement_candidates = refined
            .iter()
            .take(self.config.max_returned_proposals)
            .cloned()
            .collect::<Vec<_>>();
        let refinement_batch = build_lane_batch(
            lane_id,
            ProposalPhase::Refine,
            refinement_trace,
            &refinement_candidates,
            None,
            repair_budget_exhausted,
            delta_between(ledger.used(), proposal_budget_summary),
        )?;

        Ok(SymbolicLaneRun {
            proposal_batch,
            refinement_batch,
            repair_attempts,
        })
    }
}

#[derive(Debug, Error)]
pub enum SymbolicLaneError {
    #[error("failed to build symbolic-lane digest: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to build symbolic-lane ids or refusals: {0}")]
    SolverId(#[from] crate::SolverIdError),
    #[error("failed to build symbolic-lane refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
    #[error("failed to create symbolic-lane hypothesis: {0}")]
    Hypothesis(#[from] HypothesisError),
    #[error("failed to create symbolic-lane candidate identity: {0}")]
    CandidateIdentity(#[from] CandidateIdentityError),
    #[error("symbolic lane budget exhausted: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("failed to build symbolic-lane trace locator: {0}")]
    TraceLocator(#[from] arc_core::TraceLocatorError),
    #[error("failed to build symbolic-lane proposal batches: {0}")]
    Trace(#[from] crate::TraceBundleError),
}

#[derive(Clone)]
struct SeedProgram {
    program: ArcProgram,
}

#[derive(Clone)]
struct EvaluatedCandidate {
    hypothesis: Hypothesis,
    program: ArcProgram,
    local_score: f32,
    fit_score: f32,
    simplicity_score: f32,
    stability_score: f32,
    exact_fit: bool,
    predictions: Vec<ArcGrid>,
    expected_outputs: Vec<ArcGrid>,
}

fn generate_seed_programs(task: &CanonicalTask) -> Vec<SeedProgram> {
    let mut programs = vec![
        seed_program(SymbolicSeedTemplate::Identity),
        seed_program(SymbolicSeedTemplate::RotateQuarterTurns { quarter_turns: 1 }),
        seed_program(SymbolicSeedTemplate::RotateQuarterTurns { quarter_turns: 2 }),
        seed_program(SymbolicSeedTemplate::RotateQuarterTurns { quarter_turns: 3 }),
        seed_program(SymbolicSeedTemplate::ReflectHorizontal),
        seed_program(SymbolicSeedTemplate::ReflectVertical),
        seed_program(SymbolicSeedTemplate::CropLargest),
        seed_program(SymbolicSeedTemplate::CropSmallest),
    ];

    let mut colors = BTreeSet::new();
    for pair in &task.normalized_train {
        for color in pair
            .input
            .grid
            .cells()
            .iter()
            .chain(pair.output.grid.cells())
        {
            if *color != 0 {
                colors.insert(*color);
            }
        }
    }
    for color in colors {
        programs.push(seed_program(SymbolicSeedTemplate::CropByColor { color }));
    }
    programs
}

fn seed_program(template: SymbolicSeedTemplate) -> SeedProgram {
    let body = match template {
        SymbolicSeedTemplate::Identity => ArcGridExpr::Input,
        SymbolicSeedTemplate::RotateQuarterTurns { quarter_turns } => {
            ArcGridExpr::RotateQuarterTurns {
                source: Box::new(ArcGridExpr::Input),
                quarter_turns,
            }
        }
        SymbolicSeedTemplate::ReflectHorizontal => ArcGridExpr::ReflectHorizontal {
            source: Box::new(ArcGridExpr::Input),
        },
        SymbolicSeedTemplate::ReflectVertical => ArcGridExpr::ReflectVertical {
            source: Box::new(ArcGridExpr::Input),
        },
        SymbolicSeedTemplate::CropByColor { color } => ArcGridExpr::CropToSelector {
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::ByColor { color },
        },
        SymbolicSeedTemplate::CropLargest => ArcGridExpr::CropToSelector {
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::Largest,
        },
        SymbolicSeedTemplate::CropSmallest => ArcGridExpr::CropToSelector {
            source: Box::new(ArcGridExpr::Input),
            selector: ArcObjectSelector::Smallest,
        },
    };

    SeedProgram {
        program: ArcProgram {
            input_symbol: ArcSymbol::new("input").expect("symbol should validate"),
            bindings: Vec::new(),
            body,
            metadata: ArcProgramMetadata {
                label: Some(format!("{template:?}")),
                ..ArcProgramMetadata::default()
            },
        },
    }
}

fn evaluate_program_candidate(
    task: &CanonicalTask,
    lane_id: &SolverLaneId,
    deduplicator: &mut CandidateDeduplicator,
    ledger: &mut BudgetLedger,
    program: ArcProgram,
    index: usize,
    refinement: bool,
) -> Result<Option<EvaluatedCandidate>, SymbolicLaneError> {
    let train_pair_count = task.normalized_train.len() as u32;
    ledger.apply(BudgetCounterDelta {
        candidates_generated: 1,
        train_pair_execs: train_pair_count,
        refinement_steps: if refinement { 1 } else { 0 },
        ..BudgetCounterDelta::default()
    })?;

    let mut predictions = Vec::with_capacity(task.normalized_train.len());
    let mut expected_outputs = Vec::with_capacity(task.normalized_train.len());
    let mut total_cell_accuracy = 0.0_f32;
    let mut dimension_match_count = 0_u32;
    let mut exact_fit_pairs = 0_u32;
    for pair in &task.normalized_train {
        let Ok(predicted) = ArcInterpreter::execute(&program, &pair.input.grid) else {
            return Ok(None);
        };
        let comparison = compare_grids(&predicted, &pair.output.grid);
        if !comparison.dimension_mismatch {
            dimension_match_count += 1;
        }
        if comparison.exact_match {
            exact_fit_pairs += 1;
        }
        total_cell_accuracy += comparison.cell_accuracy;
        predictions.push(predicted);
        expected_outputs.push(pair.output.grid.clone());
    }

    let fit_score = exact_fit_pairs as f32 / task.normalized_train.len().max(1) as f32;
    let stability_score = dimension_match_count as f32 / task.normalized_train.len().max(1) as f32;
    let simplicity_score = 1.0 / (1.0 + program_size(&program) as f32);
    let local_score = (fit_score * 0.7)
        + ((total_cell_accuracy / task.normalized_train.len().max(1) as f32) * 0.2)
        + (simplicity_score * 0.1)
        + (stability_score * 0.05);

    let trace_locator = TraceLocator::new(format!(
        "trace://arc-solvers/symbolic/{}/{}/candidate-{}",
        task.raw.id.as_str(),
        if refinement { "repair" } else { "seed" },
        index
    ))?;
    let candidate_identity = CandidateIdentity::new(
        HypothesisKind::StaticProgram,
        Some(&program),
        None,
        None,
        None,
    )?;
    let hypothesis = Hypothesis::new(
        HypothesisKind::StaticProgram,
        lane_id.clone(),
        0,
        candidate_identity,
        Some(program.clone()),
        None,
        None,
        local_score,
        trace_locator,
        BudgetCounterDelta {
            candidates_generated: 1,
            train_pair_execs: train_pair_count,
            refinement_steps: if refinement { 1 } else { 0 },
            ..BudgetCounterDelta::default()
        },
    )?;
    let dedup = deduplicator.record(&hypothesis);
    if matches!(
        dedup.status,
        CandidateDeduplicationStatus::Duplicate {
            existing_hypothesis_id: _
        }
    ) {
        return Ok(None);
    }

    Ok(Some(EvaluatedCandidate {
        hypothesis,
        program,
        local_score,
        fit_score,
        simplicity_score,
        stability_score,
        exact_fit: exact_fit_pairs == task.normalized_train.len() as u32,
        predictions,
        expected_outputs,
    }))
}

fn build_lane_batch(
    lane_id: SolverLaneId,
    phase: ProposalPhase,
    trace_locator: TraceLocator,
    candidates: &[EvaluatedCandidate],
    refusal: Option<RefusalEnvelope>,
    budget_exhausted: bool,
    budget_delta: BudgetCounterDelta,
) -> Result<LaneProposalBatch, crate::TraceBundleError> {
    let proposals = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let rationale = ArcDigest::from_serializable(&(
                candidate.fit_score,
                candidate.simplicity_score,
                candidate.stability_score,
                candidate.exact_fit,
            ))
            .expect("rationale digest should build");
            TracedLaneProposal {
                hypothesis: candidate.hypothesis.clone(),
                local_rank: index as u32,
                rationale_digest: rationale,
            }
        })
        .collect::<Vec<_>>();

    let status = if !proposals.is_empty() {
        LaneBatchStatus::Proposed
    } else if refusal.is_some() {
        LaneBatchStatus::Refused
    } else if budget_exhausted {
        LaneBatchStatus::BudgetExhausted
    } else {
        LaneBatchStatus::Empty
    };
    LaneProposalBatch::new(
        lane_id,
        phase,
        status,
        proposals,
        refusal,
        trace_locator,
        budget_delta,
    )
}

fn infer_repairs(candidate: &EvaluatedCandidate) -> Vec<SymbolicRepairOperator> {
    if candidate.exact_fit {
        return Vec::new();
    }
    let mut repairs = Vec::new();
    if let Some((from, to)) = infer_recolor_mapping(candidate) {
        repairs.push(SymbolicRepairOperator::Recolor { from, to });
    }
    if let Some(transform) = infer_output_transform(candidate) {
        repairs.push(SymbolicRepairOperator::OutputTransform(transform));
    }
    repairs
}

fn infer_recolor_mapping(candidate: &EvaluatedCandidate) -> Option<(u8, u8)> {
    let mut mapping = None;
    let mut saw_difference = false;
    for (predicted, expected) in candidate
        .predictions
        .iter()
        .zip(candidate.expected_outputs.iter())
    {
        if predicted.width() != expected.width() || predicted.height() != expected.height() {
            return None;
        }
        for (left, right) in predicted.cells().iter().zip(expected.cells()) {
            if left == right {
                continue;
            }
            saw_difference = true;
            match mapping {
                None => mapping = Some((*left, *right)),
                Some(existing) if existing == (*left, *right) => {}
                Some(_) => return None,
            }
        }
    }

    if saw_difference { mapping } else { None }
}

fn infer_output_transform(candidate: &EvaluatedCandidate) -> Option<SymbolicOutputTransformRepair> {
    [
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 1 },
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 2 },
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 3 },
        SymbolicOutputTransformRepair::ReflectHorizontal,
        SymbolicOutputTransformRepair::ReflectVertical,
    ]
    .into_iter()
    .find(|transform| {
        candidate
            .predictions
            .iter()
            .zip(candidate.expected_outputs.iter())
            .all(|(predicted, expected)| {
                transform_grid(predicted, *transform)
                    .map(|transformed| transformed == *expected)
                    .unwrap_or(false)
            })
    })
}

fn apply_repair(program: ArcProgram, operator: SymbolicRepairOperator) -> ArcProgram {
    let body = match operator {
        SymbolicRepairOperator::Recolor { from, to } => ArcGridExpr::Recolor {
            source: Box::new(program.body),
            from,
            to,
        },
        SymbolicRepairOperator::OutputTransform(
            SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns },
        ) => ArcGridExpr::RotateQuarterTurns {
            source: Box::new(program.body),
            quarter_turns,
        },
        SymbolicRepairOperator::OutputTransform(
            SymbolicOutputTransformRepair::ReflectHorizontal,
        ) => ArcGridExpr::ReflectHorizontal {
            source: Box::new(program.body),
        },
        SymbolicRepairOperator::OutputTransform(SymbolicOutputTransformRepair::ReflectVertical) => {
            ArcGridExpr::ReflectVertical {
                source: Box::new(program.body),
            }
        }
    };

    ArcProgram {
        metadata: ArcProgramMetadata {
            label: Some(format!("repair::{operator:?}")),
            ..program.metadata
        },
        body,
        ..program
    }
}

fn compare_grids(predicted: &ArcGrid, expected: &ArcGrid) -> LocalGridComparison {
    if predicted.width() != expected.width() || predicted.height() != expected.height() {
        return LocalGridComparison {
            exact_match: false,
            dimension_mismatch: true,
            cell_accuracy: 0.0,
        };
    }

    let total = predicted.cell_count().max(1) as f32;
    let matches = predicted
        .cells()
        .iter()
        .zip(expected.cells().iter())
        .filter(|(left, right)| left == right)
        .count() as f32;
    LocalGridComparison {
        exact_match: matches == total,
        dimension_mismatch: false,
        cell_accuracy: matches / total,
    }
}

fn sort_candidates(candidates: &mut [EvaluatedCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .local_score
            .total_cmp(&left.local_score)
            .then_with(|| {
                ArcDigest::from_serializable(&left.program)
                    .expect("program digest should build")
                    .cmp(
                        &ArcDigest::from_serializable(&right.program)
                            .expect("program digest should build"),
                    )
            })
    });
}

fn program_size(program: &ArcProgram) -> usize {
    count_nodes(&program.body) + program.bindings.len()
}

fn count_nodes(expr: &ArcGridExpr) -> usize {
    match expr {
        ArcGridExpr::Input | ArcGridExpr::Var { .. } | ArcGridExpr::Empty { .. } => 1,
        ArcGridExpr::Sequence { steps } => 1 + steps.iter().map(count_nodes).sum::<usize>(),
        ArcGridExpr::CropToSelector { source, .. } => 1 + count_nodes(source),
        ArcGridExpr::PaintSelector { base, source, .. } => {
            1 + count_nodes(base) + count_nodes(source)
        }
        ArcGridExpr::RotateQuarterTurns { source, .. }
        | ArcGridExpr::ReflectHorizontal { source }
        | ArcGridExpr::ReflectVertical { source }
        | ArcGridExpr::Recolor { source, .. } => 1 + count_nodes(source),
        ArcGridExpr::IfAnyObjects {
            source,
            then_branch,
            else_branch,
            ..
        } => 1 + count_nodes(source) + count_nodes(then_branch) + count_nodes(else_branch),
        ArcGridExpr::Let { value, body, .. } => 1 + count_nodes(value) + count_nodes(body),
    }
}

fn proposal_trace_locator(
    task: &CanonicalTask,
    phase: &str,
) -> Result<TraceLocator, arc_core::TraceLocatorError> {
    TraceLocator::new(format!(
        "trace://arc-solvers/symbolic/{}/{}",
        task.raw.id.as_str(),
        phase
    ))
}

#[derive(Clone, Copy)]
struct LocalGridComparison {
    exact_match: bool,
    dimension_mismatch: bool,
    cell_accuracy: f32,
}

fn transform_grid(
    grid: &ArcGrid,
    transform: SymbolicOutputTransformRepair,
) -> Result<ArcGrid, arc_core::ArcGridError> {
    match transform {
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 1 } => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.width() {
                for x in 0..grid.height() {
                    cells.push(grid.cell(y, grid.height() - 1 - x).unwrap_or_default());
                }
            }
            ArcGrid::new(grid.height(), grid.width(), cells)
        }
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 2 } => {
            let rotated = transform_grid(
                grid,
                SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 1 },
            )?;
            transform_grid(
                &rotated,
                SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 1 },
            )
        }
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 3 } => {
            let rotated = transform_grid(
                grid,
                SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 2 },
            )?;
            transform_grid(
                &rotated,
                SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: 1 },
            )
        }
        SymbolicOutputTransformRepair::RotateQuarterTurns { quarter_turns: _ } => {
            Err(arc_core::ArcGridError::InvalidWidth(0))
        }
        SymbolicOutputTransformRepair::ReflectHorizontal => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(grid.cell(grid.width() - 1 - x, y).unwrap_or_default());
                }
            }
            ArcGrid::new(grid.width(), grid.height(), cells)
        }
        SymbolicOutputTransformRepair::ReflectVertical => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(grid.cell(x, grid.height() - 1 - y).unwrap_or_default());
                }
            }
            ArcGrid::new(grid.width(), grid.height(), cells)
        }
    }
}

fn delta_between(after: BudgetCounterSummary, before: BudgetCounterSummary) -> BudgetCounterDelta {
    BudgetCounterDelta {
        wall_ms: after.wall_ms.saturating_sub(before.wall_ms),
        candidates_generated: after
            .candidates_generated
            .saturating_sub(before.candidates_generated),
        verifier_evals: after.verifier_evals.saturating_sub(before.verifier_evals),
        train_pair_execs: after
            .train_pair_execs
            .saturating_sub(before.train_pair_execs),
        refinement_steps: after
            .refinement_steps
            .saturating_sub(before.refinement_steps),
        model_forward_calls: after
            .model_forward_calls
            .saturating_sub(before.model_forward_calls),
        ttt_updates: after.ttt_updates.saturating_sub(before.ttt_updates),
        peak_memory_mb: after.peak_memory_mb.saturating_sub(before.peak_memory_mb),
    }
}
