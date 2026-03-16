use arc_core::{ArcGrid, ArcGridError, ArcTaskId, CanonicalTask, TraceLocator, TraceLocatorError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, ArcGridExpr, ArcInterpreter, ArcInterpreterError, ArcProgram,
    BudgetCounterDelta, BudgetCounterSummary, BudgetLedger, BudgetLedgerError, Hypothesis,
    HypothesisKind, RefusalEnvelope, SolverPhase, SolverRefusalCode, TaskBudget,
};

/// Ownership summary for common verifier behavior.
pub const VERIFIER_BOUNDARY_SUMMARY: &str =
    "arc-solvers owns common candidate verification, falsification, and verifier-side budget use";

pub trait CandidateVerifier {
    fn evaluate(
        &self,
        task: &CanonicalTask,
        hypothesis: &Hypothesis,
        budget: TaskBudget,
    ) -> Result<VerificationReport, ArcVerifierError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcVerifierAugmentation {
    FlipHorizontal,
    FlipVertical,
    RotateClockwise,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcVerifierConfig {
    pub augmentations: Vec<ArcVerifierAugmentation>,
    pub holdout_min_examples: usize,
}

impl Default for ArcVerifierConfig {
    fn default() -> Self {
        Self {
            augmentations: vec![
                ArcVerifierAugmentation::FlipHorizontal,
                ArcVerifierAugmentation::FlipVertical,
                ArcVerifierAugmentation::RotateClockwise,
            ],
            holdout_min_examples: 3,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ArcCommonVerifier {
    config: ArcVerifierConfig,
}

impl ArcCommonVerifier {
    #[must_use]
    pub fn new(config: ArcVerifierConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn config(&self) -> &ArcVerifierConfig {
        &self.config
    }
}

impl CandidateVerifier for ArcCommonVerifier {
    fn evaluate(
        &self,
        task: &CanonicalTask,
        hypothesis: &Hypothesis,
        budget: TaskBudget,
    ) -> Result<VerificationReport, ArcVerifierError> {
        let mut ledger = BudgetLedger::new(budget);
        let verifier_config_digest = ArcDigest::from_serializable(&self.config)?;
        let trace_locator = verification_trace_locator(&task.raw.id, &hypothesis.id)?;
        let minimum_train_pair_execs = task.normalized_train.len() as u32;

        if hypothesis.kind == HypothesisKind::InteractivePlan {
            return Ok(VerificationReport::refused(
                hypothesis,
                verifier_config_digest,
                trace_locator,
                RefusalEnvelope::new(
                    SolverRefusalCode::UnsupportedTask,
                    SolverPhase::Verify,
                    "interactive-plan verification is not supported by the static common verifier",
                )?,
            ));
        }

        if budget.max_verifier_evals == 0 || budget.max_train_pair_execs < minimum_train_pair_execs
        {
            return Ok(VerificationReport::refused(
                hypothesis,
                verifier_config_digest,
                trace_locator,
                RefusalEnvelope::new(
                    SolverRefusalCode::MinimumBudgetNotMet,
                    SolverPhase::Verify,
                    format!(
                        "verification requires at least 1 verifier eval and {} train-pair executions",
                        minimum_train_pair_execs
                    ),
                )?,
            ));
        }

        ledger.apply(BudgetCounterDelta {
            verifier_evals: 1,
            ..BudgetCounterDelta::default()
        })?;

        let pair_results = evaluate_train_pairs(task, hypothesis, &mut ledger)?;
        let exact_fit = pair_results.iter().all(|result| result.exact_match);
        let pair_fit_ratio = pair_results
            .iter()
            .filter(|result| result.exact_match)
            .count() as f32
            / pair_results.len().max(1) as f32;

        let augmentation_check =
            run_augmentation_stability_check(&self.config, task, hypothesis, &mut ledger)?;
        let holdout_check =
            run_holdout_on_train_check(&self.config, task, hypothesis, &mut ledger)?;
        let falsification_checks = vec![augmentation_check.clone(), holdout_check.clone()];

        let stability_score = falsification_checks
            .iter()
            .find(|check| check.kind == FalsificationCheckKind::AugmentationStability)
            .and_then(|check| check.score)
            .unwrap_or(pair_fit_ratio);
        let holdout_score = falsification_checks
            .iter()
            .find(|check| check.kind == FalsificationCheckKind::HoldoutOnTrain)
            .and_then(|check| check.score)
            .unwrap_or(pair_fit_ratio);
        let simplicity_score = simplicity_score(hypothesis);
        let execution_failures = pair_results.iter().any(|result| result.refusal.is_some());
        let spuriousness_risk = compute_spuriousness_risk(
            pair_fit_ratio,
            stability_score,
            holdout_score,
            execution_failures,
        );
        let verifier_pass = exact_fit
            && !execution_failures
            && falsification_checks
                .iter()
                .all(|check| check.status != FalsificationCheckStatus::Failed);

        Ok(VerificationReport {
            hypothesis_id: hypothesis.id.clone(),
            verifier_config_digest,
            exact_fit,
            pair_results,
            falsification_checks,
            simplicity_score,
            stability_score,
            spuriousness_risk,
            verifier_pass,
            trace_locator,
            budget_delta: ledger.used().into(),
            refusal: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairVerificationResult {
    pub pair_index: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicted_output_digest: Option<ArcDigest>,
    pub expected_output_digest: ArcDigest,
    pub exact_match: bool,
    pub mismatched_cells: u32,
    pub dimension_mismatch: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalEnvelope>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalsificationCheckKind {
    AugmentationStability,
    HoldoutOnTrain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FalsificationCheckStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FalsificationCheckResult {
    pub kind: FalsificationCheckKind,
    pub status: FalsificationCheckStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
    pub evaluated_cases: u32,
    pub passed_cases: u32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub hypothesis_id: crate::HypothesisId,
    pub verifier_config_digest: ArcDigest,
    pub exact_fit: bool,
    pub pair_results: Vec<PairVerificationResult>,
    pub falsification_checks: Vec<FalsificationCheckResult>,
    pub simplicity_score: f32,
    pub stability_score: f32,
    pub spuriousness_risk: f32,
    pub verifier_pass: bool,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalEnvelope>,
}

impl VerificationReport {
    fn refused(
        hypothesis: &Hypothesis,
        verifier_config_digest: ArcDigest,
        trace_locator: TraceLocator,
        refusal: RefusalEnvelope,
    ) -> Self {
        Self {
            hypothesis_id: hypothesis.id.clone(),
            verifier_config_digest,
            exact_fit: false,
            pair_results: Vec::new(),
            falsification_checks: Vec::new(),
            simplicity_score: simplicity_score(hypothesis),
            stability_score: 0.0,
            spuriousness_risk: 1.0,
            verifier_pass: false,
            trace_locator,
            budget_delta: BudgetCounterDelta::default(),
            refusal: Some(refusal),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ArcVerifierError {
    #[error("failed to compute verifier digest: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to build verifier trace locator: {0}")]
    TraceLocator(#[from] TraceLocatorError),
    #[error("verifier budget accounting failed: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("verifier failed to build a transformed ARC grid: {0}")]
    Grid(#[from] ArcGridError),
    #[error("failed to create verifier refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
}

fn verification_trace_locator(
    task_id: &ArcTaskId,
    hypothesis_id: &crate::HypothesisId,
) -> Result<TraceLocator, TraceLocatorError> {
    TraceLocator::new(format!(
        "trace://arc-solvers/verifier/{}/{}",
        task_id.as_str(),
        hypothesis_id.as_str()
    ))
}

fn evaluate_train_pairs(
    task: &CanonicalTask,
    hypothesis: &Hypothesis,
    ledger: &mut BudgetLedger,
) -> Result<Vec<PairVerificationResult>, ArcVerifierError> {
    let mut results = Vec::with_capacity(task.normalized_train.len());
    for (pair_index, pair) in task.normalized_train.iter().enumerate() {
        ledger.apply(BudgetCounterDelta {
            train_pair_execs: 1,
            ..BudgetCounterDelta::default()
        })?;
        let expected = &pair.output.grid;
        let expected_output_digest = ArcDigest::from_serializable(expected)?;
        let execution = execute_hypothesis_on_grid(hypothesis, &pair.input.grid);
        let result = match execution {
            Ok(predicted) => {
                let comparison = compare_grids(&predicted, expected);
                PairVerificationResult {
                    pair_index: pair_index as u16,
                    predicted_output_digest: Some(ArcDigest::from_serializable(&predicted)?),
                    expected_output_digest,
                    exact_match: comparison.exact_match,
                    mismatched_cells: comparison.mismatched_cells,
                    dimension_mismatch: comparison.dimension_mismatch,
                    refusal: None,
                }
            }
            Err(refusal) => PairVerificationResult {
                pair_index: pair_index as u16,
                predicted_output_digest: None,
                expected_output_digest,
                exact_match: false,
                mismatched_cells: expected.cell_count() as u32,
                dimension_mismatch: false,
                refusal: Some(refusal),
            },
        };
        results.push(result);
    }
    Ok(results)
}

fn run_augmentation_stability_check(
    config: &ArcVerifierConfig,
    task: &CanonicalTask,
    hypothesis: &Hypothesis,
    ledger: &mut BudgetLedger,
) -> Result<FalsificationCheckResult, ArcVerifierError> {
    let pair_count = task.normalized_train.len() as u32;
    let augmentation_cases = pair_count * config.augmentations.len() as u32;
    if augmentation_cases == 0 {
        return Ok(FalsificationCheckResult {
            kind: FalsificationCheckKind::AugmentationStability,
            status: FalsificationCheckStatus::Skipped,
            score: None,
            evaluated_cases: 0,
            passed_cases: 0,
            detail: String::from("no verifier augmentations configured"),
        });
    }

    let used = ledger.used();
    let remaining = ledger
        .budget()
        .max_train_pair_execs
        .saturating_sub(used.train_pair_execs);
    if remaining < augmentation_cases {
        return Ok(FalsificationCheckResult {
            kind: FalsificationCheckKind::AugmentationStability,
            status: FalsificationCheckStatus::Skipped,
            score: None,
            evaluated_cases: 0,
            passed_cases: 0,
            detail: format!(
                "skipped because {} augmented executions were required but only {} train-pair executions remained",
                augmentation_cases, remaining
            ),
        });
    }

    let mut passed_cases = 0_u32;
    for pair in &task.normalized_train {
        for augmentation in &config.augmentations {
            ledger.apply(BudgetCounterDelta {
                train_pair_execs: 1,
                ..BudgetCounterDelta::default()
            })?;
            let augmented_input = transform_grid(&pair.input.grid, *augmentation)?;
            let augmented_expected = transform_grid(&pair.output.grid, *augmentation)?;
            let exact_match = match hypothesis.kind {
                HypothesisKind::StaticProgram => {
                    execute_hypothesis_on_grid(hypothesis, &augmented_input)
                        .map(|predicted| predicted == augmented_expected)
                        .unwrap_or(false)
                }
                HypothesisKind::StaticAnswer => hypothesis
                    .static_answer
                    .as_ref()
                    .map(|answer| transform_grid(answer, *augmentation))
                    .transpose()?
                    .map(|predicted| predicted == augmented_expected)
                    .unwrap_or(false),
                HypothesisKind::InteractivePlan => false,
            };
            if exact_match {
                passed_cases += 1;
            }
        }
    }

    let score = passed_cases as f32 / augmentation_cases as f32;
    Ok(FalsificationCheckResult {
        kind: FalsificationCheckKind::AugmentationStability,
        status: if score == 1.0 {
            FalsificationCheckStatus::Passed
        } else {
            FalsificationCheckStatus::Failed
        },
        score: Some(score),
        evaluated_cases: augmentation_cases,
        passed_cases,
        detail: format!(
            "{} of {} augmented executions preserved the hypothesis",
            passed_cases, augmentation_cases
        ),
    })
}

fn run_holdout_on_train_check(
    config: &ArcVerifierConfig,
    task: &CanonicalTask,
    hypothesis: &Hypothesis,
    ledger: &mut BudgetLedger,
) -> Result<FalsificationCheckResult, ArcVerifierError> {
    let holdout_cases = task.normalized_train.len() as u32;
    if task.normalized_train.len() < config.holdout_min_examples {
        return Ok(FalsificationCheckResult {
            kind: FalsificationCheckKind::HoldoutOnTrain,
            status: FalsificationCheckStatus::Skipped,
            score: None,
            evaluated_cases: 0,
            passed_cases: 0,
            detail: format!(
                "skipped because holdout-on-train requires at least {} demonstrations",
                config.holdout_min_examples
            ),
        });
    }

    let used = ledger.used();
    let remaining = ledger
        .budget()
        .max_train_pair_execs
        .saturating_sub(used.train_pair_execs);
    if remaining < holdout_cases {
        return Ok(FalsificationCheckResult {
            kind: FalsificationCheckKind::HoldoutOnTrain,
            status: FalsificationCheckStatus::Skipped,
            score: None,
            evaluated_cases: 0,
            passed_cases: 0,
            detail: format!(
                "skipped because {} holdout executions were required but only {} train-pair executions remained",
                holdout_cases, remaining
            ),
        });
    }

    let mut passed_cases = 0_u32;
    for pair in &task.normalized_train {
        ledger.apply(BudgetCounterDelta {
            train_pair_execs: 1,
            ..BudgetCounterDelta::default()
        })?;
        let exact_match = execute_hypothesis_on_grid(hypothesis, &pair.input.grid)
            .map(|predicted| predicted == pair.output.grid)
            .unwrap_or(false);
        if exact_match {
            passed_cases += 1;
        }
    }

    let score = passed_cases as f32 / holdout_cases.max(1) as f32;
    Ok(FalsificationCheckResult {
        kind: FalsificationCheckKind::HoldoutOnTrain,
        status: if score == 1.0 {
            FalsificationCheckStatus::Passed
        } else {
            FalsificationCheckStatus::Failed
        },
        score: Some(score),
        evaluated_cases: holdout_cases,
        passed_cases,
        detail: format!(
            "{} of {} leave-one-out targets matched the hypothesis output",
            passed_cases, holdout_cases
        ),
    })
}

fn execute_hypothesis_on_grid(
    hypothesis: &Hypothesis,
    input: &ArcGrid,
) -> Result<ArcGrid, RefusalEnvelope> {
    match hypothesis.kind {
        HypothesisKind::StaticProgram => {
            let program = hypothesis.program.as_ref().ok_or_else(|| {
                RefusalEnvelope::new(
                    SolverRefusalCode::InvalidCandidate,
                    SolverPhase::Verify,
                    "static-program hypothesis was missing its ARC program",
                )
                .expect("non-empty refusal")
            })?;
            ArcInterpreter::execute(program, input).map_err(|error| interpreter_refusal(error))
        }
        HypothesisKind::StaticAnswer => hypothesis.static_answer.clone().ok_or_else(|| {
            RefusalEnvelope::new(
                SolverRefusalCode::InvalidCandidate,
                SolverPhase::Verify,
                "static-answer hypothesis was missing its output grid",
            )
            .expect("non-empty refusal")
        }),
        HypothesisKind::InteractivePlan => Err(RefusalEnvelope::new(
            SolverRefusalCode::UnsupportedTask,
            SolverPhase::Verify,
            "interactive-plan verification is not supported by the static common verifier",
        )
        .expect("non-empty refusal")),
    }
}

fn interpreter_refusal(error: ArcInterpreterError) -> RefusalEnvelope {
    let (code, detail) = match error {
        ArcInterpreterError::UnknownSymbol(symbol) => (
            SolverRefusalCode::InvalidCandidate,
            format!("interpreter could not resolve symbol `{symbol}`"),
        ),
        ArcInterpreterError::InvalidQuarterTurns(value) => (
            SolverRefusalCode::UnsupportedSemantics,
            format!("interpreter refused rotate-quarter-turns value {value}"),
        ),
        ArcInterpreterError::PaintOutOfBounds { x, y } => (
            SolverRefusalCode::InvalidCandidate,
            format!("interpreter paint step would leave the grid at ({x}, {y})"),
        ),
        ArcInterpreterError::NoObjectsMatched => (
            SolverRefusalCode::UnsupportedSemantics,
            String::from("interpreter selector matched no objects on the current grid"),
        ),
        ArcInterpreterError::EmptySequence => (
            SolverRefusalCode::InvalidCandidate,
            String::from("interpreter sequence expression was empty"),
        ),
        ArcInterpreterError::Grid(inner) => (
            SolverRefusalCode::InvalidCandidate,
            format!("interpreter produced an invalid ARC grid: {inner}"),
        ),
    };
    RefusalEnvelope::new(code, SolverPhase::Verify, detail).expect("non-empty refusal")
}

fn simplicity_score(hypothesis: &Hypothesis) -> f32 {
    match hypothesis.kind {
        HypothesisKind::StaticProgram => hypothesis
            .program
            .as_ref()
            .map(|program| 1.0 / (1.0 + program_complexity(program) as f32))
            .unwrap_or(0.0),
        HypothesisKind::StaticAnswer => hypothesis
            .static_answer
            .as_ref()
            .map(|grid| 1.0 / (1.0 + grid.cell_count() as f32))
            .unwrap_or(0.0),
        HypothesisKind::InteractivePlan => hypothesis
            .interactive_plan
            .as_ref()
            .map(|steps| 1.0 / (1.0 + steps.len() as f32))
            .unwrap_or(0.0),
    }
}

fn program_complexity(program: &ArcProgram) -> usize {
    program.bindings.len() + count_grid_expr_nodes(&program.body)
}

fn count_grid_expr_nodes(expr: &ArcGridExpr) -> usize {
    match expr {
        ArcGridExpr::Input | ArcGridExpr::Var { .. } | ArcGridExpr::Empty { .. } => 1,
        ArcGridExpr::Sequence { steps } => {
            1 + steps.iter().map(count_grid_expr_nodes).sum::<usize>()
        }
        ArcGridExpr::CropToSelector { source, .. } => 1 + count_grid_expr_nodes(source),
        ArcGridExpr::PaintSelector { base, source, .. } => {
            1 + count_grid_expr_nodes(base) + count_grid_expr_nodes(source)
        }
        ArcGridExpr::RotateQuarterTurns { source, .. }
        | ArcGridExpr::ReflectHorizontal { source }
        | ArcGridExpr::ReflectVertical { source }
        | ArcGridExpr::Recolor { source, .. } => 1 + count_grid_expr_nodes(source),
        ArcGridExpr::IfAnyObjects {
            source,
            then_branch,
            else_branch,
            ..
        } => {
            1 + count_grid_expr_nodes(source)
                + count_grid_expr_nodes(then_branch)
                + count_grid_expr_nodes(else_branch)
        }
        ArcGridExpr::Let { value, body, .. } => {
            1 + count_grid_expr_nodes(value) + count_grid_expr_nodes(body)
        }
    }
}

fn compute_spuriousness_risk(
    pair_fit_ratio: f32,
    stability_score: f32,
    holdout_score: f32,
    execution_failures: bool,
) -> f32 {
    let mut risk = 1.0 - ((0.5 * pair_fit_ratio) + (0.3 * stability_score) + (0.2 * holdout_score));
    if execution_failures {
        risk += 0.25;
    }
    risk.clamp(0.0, 1.0)
}

#[derive(Clone, Copy)]
struct GridComparison {
    exact_match: bool,
    mismatched_cells: u32,
    dimension_mismatch: bool,
}

fn compare_grids(predicted: &ArcGrid, expected: &ArcGrid) -> GridComparison {
    if predicted.width() != expected.width() || predicted.height() != expected.height() {
        return GridComparison {
            exact_match: false,
            mismatched_cells: predicted.cell_count().max(expected.cell_count()) as u32,
            dimension_mismatch: true,
        };
    }

    let mismatched_cells = predicted
        .cells()
        .iter()
        .zip(expected.cells().iter())
        .filter(|(left, right)| left != right)
        .count() as u32;
    GridComparison {
        exact_match: mismatched_cells == 0,
        mismatched_cells,
        dimension_mismatch: false,
    }
}

fn transform_grid(
    grid: &ArcGrid,
    augmentation: ArcVerifierAugmentation,
) -> Result<ArcGrid, ArcGridError> {
    let (width, height, cells) = match augmentation {
        ArcVerifierAugmentation::FlipHorizontal => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(grid.cell(grid.width() - 1 - x, y).unwrap_or_default());
                }
            }
            (grid.width(), grid.height(), cells)
        }
        ArcVerifierAugmentation::FlipVertical => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.height() {
                for x in 0..grid.width() {
                    cells.push(grid.cell(x, grid.height() - 1 - y).unwrap_or_default());
                }
            }
            (grid.width(), grid.height(), cells)
        }
        ArcVerifierAugmentation::RotateClockwise => {
            let mut cells = Vec::with_capacity(grid.cell_count());
            for y in 0..grid.width() {
                for x in 0..grid.height() {
                    cells.push(grid.cell(y, grid.height() - 1 - x).unwrap_or_default());
                }
            }
            (grid.height(), grid.width(), cells)
        }
    };

    ArcGrid::new(width, height, cells)
}

impl From<BudgetCounterSummary> for BudgetCounterDelta {
    fn from(value: BudgetCounterSummary) -> Self {
        Self {
            wall_ms: value.wall_ms,
            candidates_generated: value.candidates_generated,
            verifier_evals: value.verifier_evals,
            train_pair_execs: value.train_pair_execs,
            refinement_steps: value.refinement_steps,
            model_forward_calls: value.model_forward_calls,
            ttt_updates: value.ttt_updates,
            peak_memory_mb: value.peak_memory_mb,
        }
    }
}
