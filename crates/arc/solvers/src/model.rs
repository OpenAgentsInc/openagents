use std::collections::BTreeMap;
use std::fmt;

use arc_core::{
    ARC_CORE_SCHEMA_VERSION, ArcAction, ArcGameState, ArcGrid, ArcTaskId,
    ContractSerializationError, TraceLocator, canonical_sha256_hex,
};
use serde::de;
use serde::ser::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::dsl::ArcProgram;

/// Ownership summary for the typed ARC solver object model.
pub const SOLVER_MODEL_BOUNDARY_SUMMARY: &str = "arc-solvers owns solver-specific hypothesis, refusal, budget, deduplication, and attempt-envelope contracts above arc-core";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArcDigest(String);

impl ArcDigest {
    pub fn new(raw: impl Into<String>) -> Result<Self, ArcDigestError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ArcDigestError::Empty);
        }
        if trimmed.len() != 64 {
            return Err(ArcDigestError::InvalidLength(trimmed.len()));
        }
        if let Some(ch) = trimmed
            .chars()
            .find(|candidate| !candidate.is_ascii_hexdigit())
        {
            return Err(ArcDigestError::InvalidHex(ch));
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    pub fn from_serializable<T: Serialize>(value: &T) -> Result<Self, ArcDigestError> {
        let digest = canonical_sha256_hex(value).map_err(ArcDigestError::Serialize)?;
        Self::new(digest)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ArcDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ArcDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ArcDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error)]
pub enum ArcDigestError {
    #[error("ARC digests must not be empty")]
    Empty,
    #[error("ARC digests must be 64 hexadecimal characters, got length {0}")]
    InvalidLength(usize),
    #[error("ARC digests must contain hexadecimal characters only, got `{0}`")]
    InvalidHex(char),
    #[error("failed to compute ARC digest: {0}")]
    Serialize(#[from] ContractSerializationError),
}

impl PartialEq for ArcDigestError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Empty, Self::Empty) => true,
            (Self::InvalidLength(left), Self::InvalidLength(right)) => left == right,
            (Self::InvalidHex(left), Self::InvalidHex(right)) => left == right,
            (Self::Serialize(left), Self::Serialize(right)) => {
                left.to_string() == right.to_string()
            }
            _ => false,
        }
    }
}

impl Eq for ArcDigestError {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SolverLaneId(String);

impl SolverLaneId {
    pub fn new(raw: impl Into<String>) -> Result<Self, SolverIdError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(SolverIdError::Empty);
        }
        if trimmed.chars().any(char::is_whitespace) {
            return Err(SolverIdError::ContainsWhitespace(trimmed.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SolverLaneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for SolverLaneId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SolverLaneId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HypothesisId(String);

impl HypothesisId {
    pub fn new(
        lane_id: &SolverLaneId,
        attempt_index: u8,
        candidate_identity: &CandidateIdentity,
    ) -> Result<Self, ArcDigestError> {
        let digest = ArcDigest::from_serializable(&HypothesisIdInput {
            lane_id,
            attempt_index,
            canonical_signature: &candidate_identity.canonical_signature,
        })?;
        Ok(Self(format!("hyp-{}", digest.as_str())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HypothesisId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for HypothesisId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for HypothesisId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.trim().is_empty() {
            return Err(de::Error::custom("ARC hypothesis ids must not be empty"));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SolverIdError {
    #[error("ARC solver ids must not be empty")]
    Empty,
    #[error("ARC solver ids must not contain whitespace: {0}")]
    ContainsWhitespace(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisKind {
    StaticProgram,
    StaticAnswer,
    InteractivePlan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverPhase {
    Propose,
    Refine,
    Verify,
    Arbitrate,
    Finalize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolverRefusalCode {
    UnsupportedTask,
    UnsupportedSemantics,
    InvalidTaskContract,
    InvalidCandidate,
    MinimumBudgetNotMet,
    BudgetExceeded,
    PolicyRestricted,
    DuplicateCandidate,
    IndistinctSecondAttempt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusalEnvelope {
    pub code: SolverRefusalCode,
    pub phase: SolverPhase,
    pub detail: String,
}

impl RefusalEnvelope {
    pub fn new(
        code: SolverRefusalCode,
        phase: SolverPhase,
        detail: impl Into<String>,
    ) -> Result<Self, RefusalEnvelopeError> {
        let detail = detail.into();
        let trimmed = detail.trim();
        if trimmed.is_empty() {
            return Err(RefusalEnvelopeError::EmptyDetail);
        }
        Ok(Self {
            code,
            phase,
            detail: trimmed.to_owned(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RefusalEnvelopeError {
    #[error("ARC solver refusal detail must not be empty")]
    EmptyDetail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBudget {
    pub max_wall_ms: u64,
    pub max_candidates: u32,
    pub max_verifier_evals: u32,
    pub max_train_pair_execs: u32,
    pub max_refinement_steps: u32,
    pub max_model_forward_calls: u32,
    pub max_ttt_updates: u32,
    pub max_memory_mb: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCounterDelta {
    pub wall_ms: u64,
    pub candidates_generated: u32,
    pub verifier_evals: u32,
    pub train_pair_execs: u32,
    pub refinement_steps: u32,
    pub model_forward_calls: u32,
    pub ttt_updates: u32,
    pub peak_memory_mb: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetCounterSummary {
    pub wall_ms: u64,
    pub candidates_generated: u32,
    pub verifier_evals: u32,
    pub train_pair_execs: u32,
    pub refinement_steps: u32,
    pub model_forward_calls: u32,
    pub ttt_updates: u32,
    pub peak_memory_mb: u32,
}

impl BudgetCounterSummary {
    pub fn checked_add(self, delta: BudgetCounterDelta) -> Result<Self, BudgetLedgerError> {
        Ok(Self {
            wall_ms: self
                .wall_ms
                .checked_add(delta.wall_ms)
                .ok_or(BudgetLedgerError::Overflow(BudgetCounterKind::WallMs))?,
            candidates_generated: self
                .candidates_generated
                .checked_add(delta.candidates_generated)
                .ok_or(BudgetLedgerError::Overflow(
                    BudgetCounterKind::CandidatesGenerated,
                ))?,
            verifier_evals: self
                .verifier_evals
                .checked_add(delta.verifier_evals)
                .ok_or(BudgetLedgerError::Overflow(
                    BudgetCounterKind::VerifierEvals,
                ))?,
            train_pair_execs: self
                .train_pair_execs
                .checked_add(delta.train_pair_execs)
                .ok_or(BudgetLedgerError::Overflow(
                    BudgetCounterKind::TrainPairExecs,
                ))?,
            refinement_steps: self
                .refinement_steps
                .checked_add(delta.refinement_steps)
                .ok_or(BudgetLedgerError::Overflow(
                    BudgetCounterKind::RefinementSteps,
                ))?,
            model_forward_calls: self
                .model_forward_calls
                .checked_add(delta.model_forward_calls)
                .ok_or(BudgetLedgerError::Overflow(
                    BudgetCounterKind::ModelForwardCalls,
                ))?,
            ttt_updates: self
                .ttt_updates
                .checked_add(delta.ttt_updates)
                .ok_or(BudgetLedgerError::Overflow(BudgetCounterKind::TttUpdates))?,
            peak_memory_mb: self
                .peak_memory_mb
                .checked_add(delta.peak_memory_mb)
                .ok_or(BudgetLedgerError::Overflow(BudgetCounterKind::PeakMemoryMb))?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetCounterKind {
    WallMs,
    CandidatesGenerated,
    VerifierEvals,
    TrainPairExecs,
    RefinementSteps,
    ModelForwardCalls,
    TttUpdates,
    PeakMemoryMb,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BudgetLedgerError {
    #[error("ARC budget counter `{0:?}` overflowed while applying a delta")]
    Overflow(BudgetCounterKind),
    #[error(
        "ARC budget counter `{counter:?}` would exceed its limit: attempted {attempted}, limit {limit}"
    )]
    Overdraw {
        counter: BudgetCounterKind,
        attempted: u64,
        limit: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetLedger {
    budget: TaskBudget,
    used: BudgetCounterSummary,
}

impl BudgetLedger {
    #[must_use]
    pub fn new(budget: TaskBudget) -> Self {
        Self {
            budget,
            used: BudgetCounterSummary::default(),
        }
    }

    #[must_use]
    pub fn budget(&self) -> TaskBudget {
        self.budget
    }

    #[must_use]
    pub fn used(&self) -> BudgetCounterSummary {
        self.used
    }

    pub fn apply(
        &mut self,
        delta: BudgetCounterDelta,
    ) -> Result<BudgetCounterSummary, BudgetLedgerError> {
        let next = self.used.checked_add(delta)?;
        enforce_budget_limits(self.budget, next)?;
        self.used = next;
        Ok(self.used)
    }
}

fn enforce_budget_limits(
    budget: TaskBudget,
    summary: BudgetCounterSummary,
) -> Result<(), BudgetLedgerError> {
    check_limit(
        BudgetCounterKind::WallMs,
        summary.wall_ms,
        budget.max_wall_ms,
    )?;
    check_limit(
        BudgetCounterKind::CandidatesGenerated,
        u64::from(summary.candidates_generated),
        u64::from(budget.max_candidates),
    )?;
    check_limit(
        BudgetCounterKind::VerifierEvals,
        u64::from(summary.verifier_evals),
        u64::from(budget.max_verifier_evals),
    )?;
    check_limit(
        BudgetCounterKind::TrainPairExecs,
        u64::from(summary.train_pair_execs),
        u64::from(budget.max_train_pair_execs),
    )?;
    check_limit(
        BudgetCounterKind::RefinementSteps,
        u64::from(summary.refinement_steps),
        u64::from(budget.max_refinement_steps),
    )?;
    check_limit(
        BudgetCounterKind::ModelForwardCalls,
        u64::from(summary.model_forward_calls),
        u64::from(budget.max_model_forward_calls),
    )?;
    check_limit(
        BudgetCounterKind::TttUpdates,
        u64::from(summary.ttt_updates),
        u64::from(budget.max_ttt_updates),
    )?;
    check_limit(
        BudgetCounterKind::PeakMemoryMb,
        u64::from(summary.peak_memory_mb),
        u64::from(budget.max_memory_mb),
    )
}

fn check_limit(
    counter: BudgetCounterKind,
    attempted: u64,
    limit: u64,
) -> Result<(), BudgetLedgerError> {
    if attempted > limit {
        return Err(BudgetLedgerError::Overdraw {
            counter,
            attempted,
            limit,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedActionStep {
    pub action: ArcAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_state: Option<ArcGameState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_level_index: Option<u16>,
    #[serde(default)]
    pub reset_marker: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateIdentity {
    pub canonical_signature: ArcDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer_digest: Option<ArcDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_digest: Option<ArcDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_plan_digest: Option<ArcDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_digest: Option<ArcDigest>,
}

impl CandidateIdentity {
    pub fn new(
        kind: HypothesisKind,
        program: Option<&ArcProgram>,
        static_answer: Option<&ArcGrid>,
        interactive_plan: Option<&[PlannedActionStep]>,
        semantic_digest: Option<ArcDigest>,
    ) -> Result<Self, CandidateIdentityError> {
        let answer_digest = static_answer
            .map(ArcDigest::from_serializable)
            .transpose()
            .map_err(CandidateIdentityError::Digest)?;
        let program_digest = program
            .map(ArcDigest::from_serializable)
            .transpose()
            .map_err(CandidateIdentityError::Digest)?;
        let action_plan_digest = interactive_plan
            .map(|steps| ArcDigest::from_serializable(&steps.to_vec()))
            .transpose()
            .map_err(CandidateIdentityError::Digest)?;

        if answer_digest.is_none()
            && program_digest.is_none()
            && action_plan_digest.is_none()
            && semantic_digest.is_none()
        {
            return Err(CandidateIdentityError::MissingMaterial);
        }

        let canonical_signature = ArcDigest::from_serializable(&CandidateSignatureInput {
            kind,
            answer_digest: answer_digest.as_ref(),
            program_equivalence_digest: semantic_digest.as_ref().or(program_digest.as_ref()),
            action_plan_digest: action_plan_digest.as_ref(),
        })
        .map_err(CandidateIdentityError::Digest)?;

        Ok(Self {
            canonical_signature,
            answer_digest,
            program_digest,
            action_plan_digest,
            semantic_digest,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandidateIdentityError {
    #[error(
        "ARC candidate identity requires at least one program, answer, action plan, or semantic digest"
    )]
    MissingMaterial,
    #[error("failed to compute ARC candidate digests: {0}")]
    Digest(#[from] ArcDigestError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub kind: HypothesisKind,
    pub lane_id: SolverLaneId,
    pub attempt_index: u8,
    pub candidate_identity: CandidateIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<ArcProgram>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_answer: Option<ArcGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interactive_plan: Option<Vec<PlannedActionStep>>,
    pub local_score: f32,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

impl Hypothesis {
    pub fn new(
        kind: HypothesisKind,
        lane_id: SolverLaneId,
        attempt_index: u8,
        candidate_identity: CandidateIdentity,
        program: Option<ArcProgram>,
        static_answer: Option<ArcGrid>,
        interactive_plan: Option<Vec<PlannedActionStep>>,
        local_score: f32,
        trace_locator: TraceLocator,
        budget_delta: BudgetCounterDelta,
    ) -> Result<Self, HypothesisError> {
        if !local_score.is_finite() {
            return Err(HypothesisError::NonFiniteLocalScore(local_score));
        }
        match kind {
            HypothesisKind::StaticProgram if program.is_none() => {
                return Err(HypothesisError::MissingProgram);
            }
            HypothesisKind::StaticAnswer if static_answer.is_none() => {
                return Err(HypothesisError::MissingStaticAnswer);
            }
            HypothesisKind::InteractivePlan => {
                let has_steps = interactive_plan
                    .as_ref()
                    .map(|steps| !steps.is_empty())
                    .unwrap_or(false);
                if !has_steps {
                    return Err(HypothesisError::MissingActionPlan);
                }
            }
            _ => {}
        }

        let id = HypothesisId::new(&lane_id, attempt_index, &candidate_identity)
            .map_err(HypothesisError::Id)?;

        Ok(Self {
            id,
            kind,
            lane_id,
            attempt_index,
            candidate_identity,
            program,
            static_answer,
            interactive_plan,
            local_score,
            trace_locator,
            budget_delta,
        })
    }

    #[must_use]
    pub fn materially_distinct_from(&self, prior: &Self) -> SecondAttemptDistinctness {
        let mut changed_fields = Vec::new();
        if self.kind != prior.kind {
            changed_fields.push(SecondAttemptDistinctnessField::HypothesisKind);
        }
        if self.candidate_identity.answer_digest != prior.candidate_identity.answer_digest {
            changed_fields.push(SecondAttemptDistinctnessField::AnswerDigest);
        }
        if self.candidate_identity.program_digest != prior.candidate_identity.program_digest {
            changed_fields.push(SecondAttemptDistinctnessField::ProgramDigest);
        }
        if self.candidate_identity.action_plan_digest != prior.candidate_identity.action_plan_digest
        {
            changed_fields.push(SecondAttemptDistinctnessField::ActionPlanDigest);
        }

        SecondAttemptDistinctness {
            materially_distinct: !changed_fields.is_empty()
                && self.candidate_identity.canonical_signature
                    != prior.candidate_identity.canonical_signature,
            changed_fields,
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum HypothesisError {
    #[error("ARC hypothesis local_score must be finite, got {0}")]
    NonFiniteLocalScore(f32),
    #[error("ARC static-program hypotheses require a program")]
    MissingProgram,
    #[error("ARC static-answer hypotheses require an output grid")]
    MissingStaticAnswer,
    #[error("ARC interactive hypotheses require a non-empty action plan")]
    MissingActionPlan,
    #[error("failed to compute ARC hypothesis id: {0}")]
    Id(#[from] ArcDigestError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CandidateDeduplicator {
    seen: BTreeMap<ArcDigest, HypothesisId>,
}

impl CandidateDeduplicator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, hypothesis: &Hypothesis) -> CandidateDeduplicationDecision {
        let signature = hypothesis.candidate_identity.canonical_signature.clone();
        if let Some(existing_hypothesis_id) = self.seen.get(&signature).cloned() {
            return CandidateDeduplicationDecision {
                signature,
                status: CandidateDeduplicationStatus::Duplicate {
                    existing_hypothesis_id,
                },
            };
        }

        self.seen.insert(signature.clone(), hypothesis.id.clone());
        CandidateDeduplicationDecision {
            signature,
            status: CandidateDeduplicationStatus::Accepted,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateDeduplicationDecision {
    pub signature: ArcDigest,
    pub status: CandidateDeduplicationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateDeduplicationStatus {
    Accepted,
    Duplicate {
        existing_hypothesis_id: HypothesisId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecondAttemptDistinctnessField {
    AnswerDigest,
    ProgramDigest,
    ActionPlanDigest,
    HypothesisKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondAttemptDistinctness {
    pub materially_distinct: bool,
    pub changed_fields: Vec<SecondAttemptDistinctnessField>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveAttemptStatus {
    Solved,
    Unsolved,
    BudgetExhausted,
    Refused,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolveAttemptVerificationSummary {
    pub exact_fit: bool,
    pub verifier_pass: bool,
    pub simplicity_score: f32,
    pub stability_score: f32,
    pub spuriousness_risk: f32,
}

impl SolveAttemptVerificationSummary {
    fn validate(self) -> Result<Self, SolveAttemptEnvelopeError> {
        for score in [
            self.simplicity_score,
            self.stability_score,
            self.spuriousness_risk,
        ] {
            if !score.is_finite() {
                return Err(SolveAttemptEnvelopeError::NonFiniteScore(score));
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SolveAttemptEnvelope {
    pub schema_version: u32,
    pub task_id: ArcTaskId,
    pub attempt_index: u8,
    pub task_budget: TaskBudget,
    pub status: SolveAttemptStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_answer: Option<ArcGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_lane: Option<SolverLaneId>,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_summary: Option<SolveAttemptVerificationSummary>,
    pub budget_summary: BudgetCounterSummary,
    pub trace_digest: ArcDigest,
    pub trace_locator: TraceLocator,
    pub seed_bundle_digest: ArcDigest,
    pub solver_manifest_digest: ArcDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalEnvelope>,
}

impl SolveAttemptEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: ArcTaskId,
        attempt_index: u8,
        task_budget: TaskBudget,
        status: SolveAttemptStatus,
        selected_answer: Option<ArcGrid>,
        selected_lane: Option<SolverLaneId>,
        confidence: f32,
        verification_summary: Option<SolveAttemptVerificationSummary>,
        budget_summary: BudgetCounterSummary,
        trace_digest: ArcDigest,
        trace_locator: TraceLocator,
        seed_bundle_digest: ArcDigest,
        solver_manifest_digest: ArcDigest,
        refusal: Option<RefusalEnvelope>,
    ) -> Result<Self, SolveAttemptEnvelopeError> {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(SolveAttemptEnvelopeError::InvalidConfidence(confidence));
        }
        if let Some(summary) = verification_summary {
            summary.validate()?;
        }
        enforce_budget_limits(task_budget, budget_summary)
            .map_err(SolveAttemptEnvelopeError::Budget)?;

        match status {
            SolveAttemptStatus::Solved => {
                if selected_answer.is_none() {
                    return Err(SolveAttemptEnvelopeError::SolvedMissingAnswer);
                }
                if selected_lane.is_none() {
                    return Err(SolveAttemptEnvelopeError::SolvedMissingLane);
                }
                let summary = verification_summary
                    .ok_or(SolveAttemptEnvelopeError::SolvedMissingVerificationSummary)?;
                if !summary.verifier_pass {
                    return Err(SolveAttemptEnvelopeError::SolvedWithoutVerifierPass);
                }
                if refusal.is_some() {
                    return Err(SolveAttemptEnvelopeError::UnexpectedRefusal(status));
                }
            }
            SolveAttemptStatus::Refused => {
                if refusal.is_none() {
                    return Err(SolveAttemptEnvelopeError::RefusedMissingRefusal);
                }
                if selected_answer.is_some() || selected_lane.is_some() {
                    return Err(SolveAttemptEnvelopeError::RefusalMustNotSelectAnswer);
                }
            }
            _ => {
                if refusal.is_some() {
                    return Err(SolveAttemptEnvelopeError::UnexpectedRefusal(status));
                }
            }
        }

        Ok(Self {
            schema_version: ARC_CORE_SCHEMA_VERSION,
            task_id,
            attempt_index,
            task_budget,
            status,
            selected_answer,
            selected_lane,
            confidence,
            verification_summary,
            budget_summary,
            trace_digest,
            trace_locator,
            seed_bundle_digest,
            solver_manifest_digest,
            refusal,
        })
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum SolveAttemptEnvelopeError {
    #[error("ARC solve attempt confidence must be finite and within 0.0..=1.0, got {0}")]
    InvalidConfidence(f32),
    #[error("ARC solve attempt verifier-derived scores must be finite, got {0}")]
    NonFiniteScore(f32),
    #[error("ARC solved attempts require a selected answer")]
    SolvedMissingAnswer,
    #[error("ARC solved attempts require a selected lane")]
    SolvedMissingLane,
    #[error("ARC solved attempts require a verification summary")]
    SolvedMissingVerificationSummary,
    #[error("ARC solved attempts require verifier_pass = true")]
    SolvedWithoutVerifierPass,
    #[error("ARC refused attempts require a refusal envelope")]
    RefusedMissingRefusal,
    #[error("ARC refused attempts must not include a selected answer or lane")]
    RefusalMustNotSelectAnswer,
    #[error("ARC solve attempt status `{0:?}` must not include a refusal envelope")]
    UnexpectedRefusal(SolveAttemptStatus),
    #[error("ARC solve attempt budget summary exceeds the declared budget: {0}")]
    Budget(#[from] BudgetLedgerError),
}

#[derive(Serialize)]
struct HypothesisIdInput<'a> {
    lane_id: &'a SolverLaneId,
    attempt_index: u8,
    canonical_signature: &'a ArcDigest,
}

#[derive(Serialize)]
struct CandidateSignatureInput<'a> {
    kind: HypothesisKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    answer_digest: Option<&'a ArcDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    program_equivalence_digest: Option<&'a ArcDigest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_plan_digest: Option<&'a ArcDigest>,
}
