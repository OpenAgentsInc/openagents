use arc_core::{ArcGrid, ArcTaskId, CanonicalTask, TraceLocator};
use psionic_serve::{
    DecoderModelDescriptor, GenerationOptions, GenerationRequest, TextGenerationExecutor,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    ArcDigest, ArcDigestError, BudgetCounterDelta, BudgetLedger, BudgetLedgerError,
    CandidateIdentity, CandidateIdentityError, Hypothesis, HypothesisError, HypothesisKind,
    LaneBatchStatus, LaneProposalBatch, ProposalPhase, RefusalEnvelope, SolverLaneId, SolverPhase,
    SolverRefusalCode, TaskBudget, TracedLaneProposal,
};

/// Ownership summary for the bounded transductive lane.
pub const TRANSDUCTIVE_LANE_BOUNDARY_SUMMARY: &str = "arc-solvers owns ARC prompt rendering and bounded Psionic-backed transductive adapter integration";

pub const TRANSDUCTIVE_LANE_ID: &str = "transductive";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcTransductivePrompt {
    pub prompt_text: String,
    pub prompt_digest: ArcDigest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcTransductiveAdapterRequest {
    pub task_id: ArcTaskId,
    pub prompt: ArcTransductivePrompt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcTransductiveAdapterResponse {
    pub request_id: String,
    pub model_id: String,
    pub raw_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_grid: Option<ArcGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
}

pub trait ArcLocalModelAdapter {
    fn generate_candidate(
        &mut self,
        request: &ArcTransductiveAdapterRequest,
    ) -> Result<ArcTransductiveAdapterResponse, ArcTransductiveLaneError>;
}

#[derive(Clone, Debug)]
pub struct PsionicTextGenerationAdapter<E> {
    executor: E,
    model: DecoderModelDescriptor,
    options: GenerationOptions,
    request_prefix: String,
}

impl<E> PsionicTextGenerationAdapter<E> {
    #[must_use]
    pub fn new(
        executor: E,
        model: DecoderModelDescriptor,
        options: GenerationOptions,
        request_prefix: impl Into<String>,
    ) -> Self {
        Self {
            executor,
            model,
            options,
            request_prefix: request_prefix.into(),
        }
    }

    #[must_use]
    pub fn into_inner(self) -> E {
        self.executor
    }
}

impl<E> ArcLocalModelAdapter for PsionicTextGenerationAdapter<E>
where
    E: TextGenerationExecutor,
    E::Error: std::fmt::Display,
{
    fn generate_candidate(
        &mut self,
        request: &ArcTransductiveAdapterRequest,
    ) -> Result<ArcTransductiveAdapterResponse, ArcTransductiveLaneError> {
        let generation_request = GenerationRequest::new_text(
            format!("{}-{}", self.request_prefix, request.task_id.as_str()),
            self.model.clone(),
            None,
            request.prompt.prompt_text.clone(),
            self.options.clone(),
        );
        let response = self
            .executor
            .generate(&generation_request)
            .map_err(|error| ArcTransductiveLaneError::AdapterRuntime(error.to_string()))?;
        let raw_text = response.output.text.clone();
        let (candidate_grid, parse_error) = match parse_arc_grid_text(raw_text.as_str()) {
            Ok(grid) => (Some(grid), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Ok(ArcTransductiveAdapterResponse {
            request_id: response.request_id,
            model_id: response.model_id,
            raw_text,
            candidate_grid,
            parse_error,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcTransductiveLaneConfig {
    pub max_candidates: usize,
}

impl Default for ArcTransductiveLaneConfig {
    fn default() -> Self {
        Self { max_candidates: 1 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcTransductiveLaneRun {
    pub adapter_response: ArcTransductiveAdapterResponse,
    pub proposal_batch: LaneProposalBatch,
    pub refinement_batch: LaneProposalBatch,
}

pub struct ArcTransductiveLane<A> {
    adapter: A,
    config: ArcTransductiveLaneConfig,
}

impl<A> ArcTransductiveLane<A> {
    #[must_use]
    pub fn new(adapter: A, config: ArcTransductiveLaneConfig) -> Self {
        Self { adapter, config }
    }
}

impl<A> ArcTransductiveLane<A>
where
    A: ArcLocalModelAdapter,
{
    pub fn run(
        &mut self,
        task: &CanonicalTask,
        budget: TaskBudget,
    ) -> Result<ArcTransductiveLaneRun, ArcTransductiveLaneError> {
        let lane_id = SolverLaneId::new(TRANSDUCTIVE_LANE_ID)?;
        let proposal_trace = TraceLocator::new(format!(
            "trace://arc-solvers/transductive/{}/proposal",
            task.raw.id.as_str()
        ))?;
        let refinement_trace = TraceLocator::new(format!(
            "trace://arc-solvers/transductive/{}/refinement",
            task.raw.id.as_str()
        ))?;

        if task.normalized_test_inputs.len() != 1 {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::UnsupportedTask,
                SolverPhase::Propose,
                "bounded transductive lane currently supports exactly one test input",
            )?;
            return Ok(ArcTransductiveLaneRun {
                adapter_response: ArcTransductiveAdapterResponse {
                    request_id: format!("transductive-{}", task.raw.id.as_str()),
                    model_id: String::from("unavailable"),
                    raw_text: String::new(),
                    candidate_grid: None,
                    parse_error: Some(String::from("multiple test inputs are unsupported")),
                },
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
            });
        }

        if self.config.max_candidates == 0
            || budget.max_candidates == 0
            || budget.max_model_forward_calls == 0
        {
            let refusal = RefusalEnvelope::new(
                SolverRefusalCode::MinimumBudgetNotMet,
                SolverPhase::Propose,
                "transductive lane requires at least one candidate slot and one model forward call",
            )?;
            return Ok(ArcTransductiveLaneRun {
                adapter_response: ArcTransductiveAdapterResponse {
                    request_id: format!("transductive-{}", task.raw.id.as_str()),
                    model_id: String::from("unavailable"),
                    raw_text: String::new(),
                    candidate_grid: None,
                    parse_error: Some(String::from("model-forward budget was zero")),
                },
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
            });
        }

        let prompt = render_transductive_prompt(task)?;
        let request = ArcTransductiveAdapterRequest {
            task_id: task.raw.id.clone(),
            prompt,
        };
        let mut ledger = BudgetLedger::new(budget);
        ledger.apply(BudgetCounterDelta {
            candidates_generated: 1,
            model_forward_calls: 1,
            ..BudgetCounterDelta::default()
        })?;
        let adapter_response = self.adapter.generate_candidate(&request)?;

        let (proposal_batch, refinement_batch) =
            if let Some(candidate_grid) = adapter_response.candidate_grid.clone() {
                let hypothesis = Hypothesis::new(
                    HypothesisKind::StaticAnswer,
                    lane_id.clone(),
                    0,
                    CandidateIdentity::new(
                        HypothesisKind::StaticAnswer,
                        None,
                        Some(&candidate_grid),
                        None,
                        None,
                    )?,
                    None,
                    Some(candidate_grid),
                    None,
                    shape_score(
                        task,
                        adapter_response
                            .candidate_grid
                            .as_ref()
                            .expect("candidate grid exists"),
                    ),
                    TraceLocator::new(format!(
                        "trace://arc-solvers/transductive/{}/candidate-0",
                        task.raw.id.as_str()
                    ))?,
                    ledger.used().into(),
                )?;
                let proposal = TracedLaneProposal {
                    hypothesis,
                    local_rank: 0,
                    rationale_digest: ArcDigest::from_serializable(&adapter_response.raw_text)?,
                };
                (
                    LaneProposalBatch::new(
                        lane_id.clone(),
                        ProposalPhase::Propose,
                        LaneBatchStatus::Proposed,
                        vec![proposal],
                        None,
                        proposal_trace,
                        ledger.used().into(),
                    )?,
                    LaneProposalBatch::new(
                        lane_id,
                        ProposalPhase::Refine,
                        LaneBatchStatus::Empty,
                        Vec::new(),
                        None,
                        refinement_trace,
                        BudgetCounterDelta::default(),
                    )?,
                )
            } else {
                let refusal = RefusalEnvelope::new(
                    SolverRefusalCode::InvalidCandidate,
                    SolverPhase::Propose,
                    adapter_response.parse_error.clone().unwrap_or_else(|| {
                        String::from("local model adapter returned no ARC candidate")
                    }),
                )?;
                (
                    LaneProposalBatch::new(
                        lane_id.clone(),
                        ProposalPhase::Propose,
                        LaneBatchStatus::Refused,
                        Vec::new(),
                        Some(refusal),
                        proposal_trace,
                        ledger.used().into(),
                    )?,
                    LaneProposalBatch::new(
                        lane_id,
                        ProposalPhase::Refine,
                        LaneBatchStatus::Empty,
                        Vec::new(),
                        None,
                        refinement_trace,
                        BudgetCounterDelta::default(),
                    )?,
                )
            };

        Ok(ArcTransductiveLaneRun {
            adapter_response,
            proposal_batch,
            refinement_batch,
        })
    }
}

#[derive(Debug, Error)]
pub enum ArcTransductiveLaneError {
    #[error("failed to compute transductive prompt digests: {0}")]
    Digest(#[from] ArcDigestError),
    #[error("failed to create transductive solver ids or refusals: {0}")]
    SolverId(#[from] crate::SolverIdError),
    #[error("failed to create transductive refusal: {0}")]
    Refusal(#[from] crate::RefusalEnvelopeError),
    #[error("failed to create transductive hypothesis: {0}")]
    Hypothesis(#[from] HypothesisError),
    #[error("failed to create transductive candidate identity: {0}")]
    CandidateIdentity(#[from] CandidateIdentityError),
    #[error("transductive lane budget accounting failed: {0}")]
    Budget(#[from] BudgetLedgerError),
    #[error("failed to create transductive trace locator: {0}")]
    TraceLocator(#[from] arc_core::TraceLocatorError),
    #[error("failed to build transductive proposal batches: {0}")]
    Trace(#[from] crate::TraceBundleError),
    #[error("Psionic local adapter failed: {0}")]
    AdapterRuntime(String),
    #[error("failed to parse ARC candidate grid: {0}")]
    Parse(String),
}

fn render_transductive_prompt(
    task: &CanonicalTask,
) -> Result<ArcTransductivePrompt, ArcTransductiveLaneError> {
    let mut prompt = String::from(
        "You solve ARC tasks. Return only JSON in the form {\"grid\": [[...], ...]} using digits 0-9.\n",
    );
    for (index, pair) in task.normalized_train.iter().enumerate() {
        prompt.push_str(&format!(
            "train_{index}_input={}\ntrain_{index}_output={}\n",
            render_grid_json(&pair.input.grid)?,
            render_grid_json(&pair.output.grid)?,
        ));
    }
    let test_input = &task.normalized_test_inputs[0].grid;
    prompt.push_str(&format!(
        "test_input={}\nanswer=",
        render_grid_json(test_input)?
    ));

    Ok(ArcTransductivePrompt {
        prompt_digest: ArcDigest::from_serializable(&prompt)?,
        prompt_text: prompt,
    })
}

fn render_grid_json(grid: &ArcGrid) -> Result<String, ArcTransductiveLaneError> {
    let mut rows = Vec::with_capacity(grid.height() as usize);
    for y in 0..grid.height() {
        let mut row = Vec::with_capacity(grid.width() as usize);
        for x in 0..grid.width() {
            row.push(grid.cell(x, y).unwrap_or_default());
        }
        rows.push(row);
    }
    serde_json::to_string(&rows).map_err(|error| ArcTransductiveLaneError::Parse(error.to_string()))
}

fn parse_arc_grid_text(raw_text: &str) -> Result<ArcGrid, ArcTransductiveLaneError> {
    let parsed = serde_json::from_str::<Value>(raw_text)
        .or_else(|_| serde_json::from_str::<Value>(raw_text.trim()))
        .map_err(|error| ArcTransductiveLaneError::Parse(error.to_string()))?;
    let grid_value = parsed.get("grid").unwrap_or(&parsed);
    let rows = grid_value.as_array().ok_or_else(|| {
        ArcTransductiveLaneError::Parse(String::from("candidate must be a grid array"))
    })?;
    if rows.is_empty() {
        return Err(ArcTransductiveLaneError::Parse(String::from(
            "candidate grid must not be empty",
        )));
    }

    let mut width = None;
    let mut cells = Vec::new();
    for row in rows {
        let row = row.as_array().ok_or_else(|| {
            ArcTransductiveLaneError::Parse(String::from("grid rows must be arrays"))
        })?;
        if row.is_empty() {
            return Err(ArcTransductiveLaneError::Parse(String::from(
                "grid rows must not be empty",
            )));
        }
        match width {
            None => width = Some(row.len() as u8),
            Some(existing) if existing == row.len() as u8 => {}
            Some(_) => {
                return Err(ArcTransductiveLaneError::Parse(String::from(
                    "all grid rows must have the same width",
                )));
            }
        }
        for value in row {
            let Some(color) = value.as_u64() else {
                return Err(ArcTransductiveLaneError::Parse(String::from(
                    "grid cells must be integer digits",
                )));
            };
            cells.push(color as u8);
        }
    }

    ArcGrid::new(width.unwrap_or(0), rows.len() as u8, cells)
        .map_err(|error| ArcTransductiveLaneError::Parse(error.to_string()))
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
