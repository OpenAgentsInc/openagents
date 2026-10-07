use std::collections::BTreeSet;

use psionic_eval::build_gemma_e4b_finetune_eval_benchmark_package;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    GemmaE4bCudaAdapterCheckpoint, GemmaE4bCudaAdapterExportRequest, GemmaE4bCudaAdapterSftConfig,
    GemmaE4bCudaAdapterSftError, GemmaE4bCudaAdapterSftTrainer, GemmaE4bCudaAdapterTargetSet,
    GemmaE4bFinetuningMvpError, GemmaE4bLmHeadSupervisionSample, GemmaE4bServedBaseModelBinding,
    TrainingRunSummary, canonical_gemma_e4b_finetuning_mvp_contract,
};

/// Stable schema version for the bounded Gemma e4b dataset contract.
pub const GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_finetune_dataset_contract.v1";

/// Stable schema version for the bounded Gemma e4b eval-pack binding.
pub const GEMMA_E4B_FINETUNE_EVAL_PACK_BINDING_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_finetune_eval_pack_binding.v1";

/// Stable schema version for one bounded Gemma e4b baseline sweep request.
pub const GEMMA_E4B_BASELINE_SWEEP_REQUEST_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_baseline_sweep_request.v1";

/// Stable schema version for one bounded Gemma e4b eval receipt.
pub const GEMMA_E4B_FINETUNE_EVAL_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_finetune_eval_receipt.v1";

/// Stable schema version for the promoted-checkpoint vibe-eval packet.
pub const GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_promoted_checkpoint_vibe_eval_packet.v1";

/// Stable template id for the promoted-checkpoint vibe-eval packet.
pub const GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_TEMPLATE_ID: &str =
    "gemma4.e4b.promoted_checkpoint_vibe_eval.v1";

/// Stable schema version for the bounded operator review.
pub const GEMMA_E4B_OPERATOR_REVIEW_SCHEMA_VERSION: &str = "psionic.gemma4_e4b_operator_review.v1";

/// Stable schema version for the bounded promotion decision.
pub const GEMMA_E4B_PROMOTION_DECISION_SCHEMA_VERSION: &str =
    "psionic.gemma4_e4b_promotion_decision.v1";

/// Declared assistant-mask posture for the first Gemma e4b finetune lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bAssistantMaskKind {
    /// Only assistant-completion tokens are trainable.
    AssistantResponsesOnly,
}

/// Review status for one vibe-eval case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bReviewVerdictStatus {
    Passed,
    Failed,
}

/// Review state for the bounded operator promotion review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bOperatorReviewState {
    Pending,
    Approved,
    Rejected,
}

/// Subject kind scored by the bounded finetune eval receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bFinetuneEvalSubjectKind {
    UntunedBase,
    CheckpointCandidate,
}

/// Vibe-eval case family required for promoted checkpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bVibeEvalCaseKind {
    TemplateIntegrity,
    Steerability,
    ToolUse,
    Formatting,
}

/// Gate status on the bounded promotion decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bPromotionGateStatus {
    Passed,
    Held,
    Failed,
}

/// Final promotion decision for one candidate checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GemmaE4bPromotionDecisionState {
    Promote,
    HoldForReview,
    Reject,
}

/// Benchmark-overlap and decontamination review bound to one uploaded dataset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bBenchmarkOverlapCheck {
    /// Stable review identifier.
    pub check_id: String,
    /// Benchmarks reviewed for overlap.
    pub compared_benchmark_refs: Vec<String>,
    /// Exact overlap hits that would block the dataset.
    pub exact_overlap_refs: Vec<String>,
    /// Near-duplicate hits that would block the dataset.
    pub near_duplicate_overlap_refs: Vec<String>,
    /// Whether the dataset cleared the review.
    pub passed: bool,
    /// Short operator-facing detail.
    pub detail: String,
}

impl GemmaE4bBenchmarkOverlapCheck {
    fn validate(&self) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_nonempty(self.check_id.as_str(), "benchmark_overlap_check.check_id")?;
        if self.compared_benchmark_refs.is_empty() {
            return Err(GemmaE4bFinetuneEvalError::MissingField {
                field: String::from("benchmark_overlap_check.compared_benchmark_refs"),
            });
        }
        for benchmark_ref in &self.compared_benchmark_refs {
            ensure_nonempty(
                benchmark_ref.as_str(),
                "benchmark_overlap_check.compared_benchmark_refs[]",
            )?;
        }
        ensure_nonempty(self.detail.as_str(), "benchmark_overlap_check.detail")?;
        if self.passed
            && (!self.exact_overlap_refs.is_empty() || !self.near_duplicate_overlap_refs.is_empty())
        {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("benchmark_overlap_check.passed"),
                detail: String::from(
                    "passed overlap checks cannot retain exact or near-duplicate hits",
                ),
            });
        }
        Ok(())
    }
}

/// Dataset contract required before the bounded Gemma e4b lane can train.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuneDatasetContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable train split reference.
    pub train_split_ref: String,
    /// Stable held-out validation split reference.
    pub held_out_validation_split_ref: String,
    /// Stable final report or test split reference.
    pub final_report_split_ref: String,
    /// Stable short baseline split reference.
    pub baseline_short_split_ref: String,
    /// Template digest the dataset was rendered against.
    pub chat_template_digest: String,
    /// Assistant-mask posture.
    pub assistant_mask_kind: GemmaE4bAssistantMaskKind,
    /// Assistant-mask coverage in basis points.
    pub assistant_mask_coverage_bps: u32,
    /// Benchmark-overlap and decontamination review.
    pub benchmark_overlap_check: GemmaE4bBenchmarkOverlapCheck,
    /// Stable digest over the dataset contract.
    pub dataset_digest: String,
}

impl GemmaE4bFinetuneDatasetContract {
    /// Returns the stable digest over the dataset contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.dataset_digest.clear();
        stable_digest(b"psionic_gemma_e4b_finetune_dataset_contract|", &clone)
    }

    /// Validates the dataset contract against the bounded Gemma lane.
    pub fn validate(&self) -> Result<(), GemmaE4bFinetuneEvalError> {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract()?;
        ensure_exact(
            self.schema_version.as_str(),
            "dataset_contract.schema_version",
            GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION,
        )?;
        ensure_nonempty(self.dataset_ref.as_str(), "dataset_contract.dataset_ref")?;
        ensure_nonempty(
            self.train_split_ref.as_str(),
            "dataset_contract.train_split_ref",
        )?;
        ensure_nonempty(
            self.held_out_validation_split_ref.as_str(),
            "dataset_contract.held_out_validation_split_ref",
        )?;
        ensure_nonempty(
            self.final_report_split_ref.as_str(),
            "dataset_contract.final_report_split_ref",
        )?;
        ensure_nonempty(
            self.baseline_short_split_ref.as_str(),
            "dataset_contract.baseline_short_split_ref",
        )?;
        let split_refs = BTreeSet::from([
            self.train_split_ref.as_str(),
            self.held_out_validation_split_ref.as_str(),
            self.final_report_split_ref.as_str(),
            self.baseline_short_split_ref.as_str(),
        ]);
        if split_refs.len() != 4 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("dataset_contract.split_refs"),
                detail: String::from(
                    "train, held_out_validation, final_report, and baseline_short splits must stay distinct",
                ),
            });
        }
        if self.chat_template_digest
            != contract
                .tokenizer
                .template_digest
                .as_deref()
                .unwrap_or_default()
        {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("dataset_contract.chat_template_digest"),
                detail: String::from(
                    "dataset chat template digest drifted from the bounded Gemma prompt fixture",
                ),
            });
        }
        if self.assistant_mask_coverage_bps != 10_000 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("dataset_contract.assistant_mask_coverage_bps"),
                detail: String::from(
                    "the bounded Gemma lane requires full assistant-mask coverage over supervised targets",
                ),
            });
        }
        self.benchmark_overlap_check.validate()?;
        if !self.benchmark_overlap_check.passed {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("dataset_contract.benchmark_overlap_check"),
                detail: String::from(
                    "datasets that failed decontamination or benchmark-overlap review cannot enter the bounded Gemma lane",
                ),
            });
        }
        if self.dataset_digest != self.stable_digest() {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("dataset_contract.dataset_digest"),
                detail: String::from("dataset digest drifted"),
            });
        }
        Ok(())
    }
}

/// Binding to the canonical Gemma e4b finetune eval pack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuneEvalPackBinding {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable `benchmark_ref@version` storage key.
    pub benchmark_package_storage_key: String,
    /// Stable digest over the canonical benchmark package.
    pub benchmark_package_digest: String,
    /// Required operator review packet template.
    pub required_vibe_packet_template_id: String,
    /// Bounded claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the binding.
    pub binding_digest: String,
}

impl GemmaE4bFinetuneEvalPackBinding {
    /// Returns the stable digest over the binding payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.binding_digest.clear();
        stable_digest(b"psionic_gemma_e4b_finetune_eval_pack_binding|", &clone)
    }

    /// Validates the binding against the canonical eval pack.
    pub fn validate(&self) -> Result<(), GemmaE4bFinetuneEvalError> {
        let expected = canonical_gemma_e4b_finetune_eval_pack_binding()?;
        ensure_exact(
            self.schema_version.as_str(),
            "eval_pack_binding.schema_version",
            GEMMA_E4B_FINETUNE_EVAL_PACK_BINDING_SCHEMA_VERSION,
        )?;
        if self != &expected {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("eval_pack_binding"),
                detail: String::from(
                    "eval-pack binding drifted from the canonical Gemma e4b held-out package",
                ),
            });
        }
        Ok(())
    }
}

/// One narrow baseline candidate inside the Gemma e4b short sweep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bBaselineCandidate {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Candidate config used for the short run.
    pub config: GemmaE4bCudaAdapterSftConfig,
    /// Maximum number of train steps for the short run.
    pub max_steps: u64,
    /// Short detail.
    pub detail: String,
}

impl GemmaE4bBaselineCandidate {
    fn validate(&self) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_nonempty(
            self.candidate_id.as_str(),
            "baseline_candidate.candidate_id",
        )?;
        self.config
            .validate()
            .map_err(GemmaE4bCudaAdapterSftError::from)?;
        if self.max_steps == 0 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("baseline_candidate.max_steps"),
                detail: String::from("baseline candidates must run at least one step"),
            });
        }
        if self.max_steps >= self.config.budget.max_steps {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("baseline_candidate.max_steps"),
                detail: String::from(
                    "baseline candidates must stop before the full training budget to stay short",
                ),
            });
        }
        ensure_nonempty(self.detail.as_str(), "baseline_candidate.detail")?;
        Ok(())
    }
}

/// Request for the bounded Gemma e4b baseline sweep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bBaselineSweepRequest {
    /// Stable schema version.
    pub schema_version: String,
    /// Dataset contract bound before the sweep starts.
    pub dataset_contract: GemmaE4bFinetuneDatasetContract,
    /// Eval-pack binding bound before the sweep starts.
    pub eval_pack_binding: GemmaE4bFinetuneEvalPackBinding,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable adapter id prefix used for temporary export.
    pub adapter_id_prefix: String,
    /// Stable adapter revision prefix used for temporary export.
    pub adapter_revision_prefix: String,
    /// Narrow candidate list.
    pub candidates: Vec<GemmaE4bBaselineCandidate>,
    /// Logical baseline start timestamp.
    pub started_at_ms: u64,
    /// Logical duration for each short sweep step.
    pub step_duration_ms: u64,
}

impl GemmaE4bBaselineSweepRequest {
    /// Validates the baseline sweep request.
    pub fn validate(&self) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "baseline_sweep_request.schema_version",
            GEMMA_E4B_BASELINE_SWEEP_REQUEST_SCHEMA_VERSION,
        )?;
        self.dataset_contract.validate()?;
        self.eval_pack_binding.validate()?;
        ensure_nonempty(
            self.validator_policy_ref.as_str(),
            "baseline_sweep_request.validator_policy_ref",
        )?;
        ensure_nonempty(
            self.adapter_id_prefix.as_str(),
            "baseline_sweep_request.adapter_id_prefix",
        )?;
        ensure_nonempty(
            self.adapter_revision_prefix.as_str(),
            "baseline_sweep_request.adapter_revision_prefix",
        )?;
        if self.candidates.is_empty() {
            return Err(GemmaE4bFinetuneEvalError::MissingField {
                field: String::from("baseline_sweep_request.candidates"),
            });
        }
        if self.candidates.len() > 4 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("baseline_sweep_request.candidates"),
                detail: String::from("baseline sweep must stay narrow at four candidates or fewer"),
            });
        }
        let mut ids = BTreeSet::new();
        for candidate in &self.candidates {
            candidate.validate()?;
            if !ids.insert(candidate.candidate_id.as_str()) {
                return Err(GemmaE4bFinetuneEvalError::DuplicateId {
                    field: String::from("baseline_sweep_request.candidates[].candidate_id"),
                    value: candidate.candidate_id.clone(),
                });
            }
        }
        if self.step_duration_ms == 0 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("baseline_sweep_request.step_duration_ms"),
                detail: String::from("baseline sweep requires `step_duration_ms > 0`"),
            });
        }
        Ok(())
    }
}

/// One retained baseline-sweep result row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bBaselineCandidateResult {
    /// Stable candidate identifier.
    pub candidate_id: String,
    /// Final short-run training summary.
    pub run_summary: TrainingRunSummary,
    /// Final mean train loss observed during the short run.
    pub final_train_mean_loss: Option<f32>,
    /// Held-out validation mean loss observed before the ephemeral validation step updates weights.
    pub held_out_validation_mean_loss: f32,
    /// Short detail.
    pub detail: String,
}

/// Bounded outcome for the Gemma e4b short baseline sweep.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GemmaE4bBaselineSweepOutcome {
    /// Dataset reference scored by the sweep.
    pub dataset_ref: String,
    /// Eval pack storage key used by the sweep.
    pub eval_pack_storage_key: String,
    /// Held-out validation split reference.
    pub held_out_validation_split_ref: String,
    /// Candidate results.
    pub candidate_results: Vec<GemmaE4bBaselineCandidateResult>,
    /// Recommended candidate by lowest held-out validation loss.
    pub recommended_candidate_id: String,
}

/// Automatic eval receipt retained for either the untuned base or one checkpoint candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bFinetuneEvalReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Subject kind.
    pub subject_kind: GemmaE4bFinetuneEvalSubjectKind,
    /// Subject identifier.
    pub subject_id: String,
    /// Stable digest or artifact identity for the subject.
    pub subject_digest: String,
    /// Stable benchmark storage key.
    pub benchmark_package_storage_key: String,
    /// Stable benchmark package digest.
    pub benchmark_package_digest: String,
    /// Held-out validation split reference.
    pub held_out_validation_split_ref: String,
    /// Final report or test split reference.
    pub final_report_split_ref: String,
    /// Held-out pass rate in basis points.
    pub held_out_pass_rate_bps: u32,
    /// Held-out aggregate score in basis points.
    pub held_out_score_bps: u32,
    /// Whether chat-template validation passed.
    pub chat_template_passed: bool,
    /// Whether assistant-mask validation passed.
    pub assistant_mask_passed: bool,
    /// Whether tool-call formatting validation passed.
    pub tool_call_format_passed: bool,
    /// Whether formatting validation passed.
    pub formatting_passed: bool,
    /// Whether steerability validation passed.
    pub steerability_passed: bool,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl GemmaE4bFinetuneEvalReceipt {
    /// Returns the stable digest over the eval receipt payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_digest(b"psionic_gemma_e4b_finetune_eval_receipt|", &clone)
    }

    /// Validates the eval receipt against the dataset contract and eval pack.
    pub fn validate(
        &self,
        dataset_contract: &GemmaE4bFinetuneDatasetContract,
        eval_pack_binding: &GemmaE4bFinetuneEvalPackBinding,
    ) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "eval_receipt.schema_version",
            GEMMA_E4B_FINETUNE_EVAL_RECEIPT_SCHEMA_VERSION,
        )?;
        ensure_nonempty(self.receipt_id.as_str(), "eval_receipt.receipt_id")?;
        ensure_nonempty(self.subject_id.as_str(), "eval_receipt.subject_id")?;
        ensure_nonempty(self.subject_digest.as_str(), "eval_receipt.subject_digest")?;
        ensure_nonempty(self.detail.as_str(), "eval_receipt.detail")?;
        ensure_exact(
            self.benchmark_package_storage_key.as_str(),
            "eval_receipt.benchmark_package_storage_key",
            eval_pack_binding.benchmark_package_storage_key.as_str(),
        )?;
        ensure_exact(
            self.benchmark_package_digest.as_str(),
            "eval_receipt.benchmark_package_digest",
            eval_pack_binding.benchmark_package_digest.as_str(),
        )?;
        ensure_exact(
            self.held_out_validation_split_ref.as_str(),
            "eval_receipt.held_out_validation_split_ref",
            dataset_contract.held_out_validation_split_ref.as_str(),
        )?;
        ensure_exact(
            self.final_report_split_ref.as_str(),
            "eval_receipt.final_report_split_ref",
            dataset_contract.final_report_split_ref.as_str(),
        )?;
        if self.held_out_pass_rate_bps == 0 || self.held_out_score_bps == 0 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("eval_receipt.held_out_bps"),
                detail: String::from("held-out pass rate and score must be positive"),
            });
        }
        if self.receipt_digest != self.stable_digest() {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("eval_receipt.receipt_digest"),
                detail: String::from("eval receipt digest drifted"),
            });
        }
        Ok(())
    }

    fn automatic_clearance(&self) -> bool {
        self.chat_template_passed
            && self.assistant_mask_passed
            && self.tool_call_format_passed
            && self.formatting_passed
            && self.steerability_passed
    }
}

/// One canned promoted-checkpoint vibe-eval case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bVibeEvalCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Case family.
    pub case_kind: GemmaE4bVibeEvalCaseKind,
    /// Prompt or operator instruction.
    pub prompt: String,
    /// What the operator should verify.
    pub expected_property: String,
}

/// Canned vibe-eval packet retained for one promoted checkpoint candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bPromotedCheckpointVibePacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable template id.
    pub packet_template_id: String,
    /// Stable packet identifier.
    pub packet_id: String,
    /// Candidate checkpoint identifier.
    pub checkpoint_id: String,
    /// Candidate checkpoint digest.
    pub checkpoint_digest: String,
    /// Canonical review cases.
    pub cases: Vec<GemmaE4bVibeEvalCase>,
    /// Stable digest over the packet.
    pub packet_digest: String,
}

impl GemmaE4bPromotedCheckpointVibePacket {
    /// Returns the stable digest over the packet payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.packet_digest.clear();
        stable_digest(
            b"psionic_gemma_e4b_promoted_checkpoint_vibe_packet|",
            &clone,
        )
    }

    /// Validates the vibe-eval packet against the supplied checkpoint.
    pub fn validate(
        &self,
        checkpoint: &GemmaE4bCudaAdapterCheckpoint,
    ) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "vibe_packet.schema_version",
            GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.packet_template_id.as_str(),
            "vibe_packet.packet_template_id",
            GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_TEMPLATE_ID,
        )?;
        ensure_nonempty(self.packet_id.as_str(), "vibe_packet.packet_id")?;
        ensure_exact(
            self.checkpoint_id.as_str(),
            "vibe_packet.checkpoint_id",
            checkpoint.checkpoint_id.as_str(),
        )?;
        ensure_exact(
            self.checkpoint_digest.as_str(),
            "vibe_packet.checkpoint_digest",
            checkpoint.checkpoint_digest.as_str(),
        )?;
        if self.cases.len() != 4 {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("vibe_packet.cases"),
                detail: String::from("vibe packet must retain the four canonical review cases"),
            });
        }
        let kinds = self
            .cases
            .iter()
            .map(|case| case.case_kind)
            .collect::<BTreeSet<_>>();
        let expected_kinds = BTreeSet::from([
            GemmaE4bVibeEvalCaseKind::TemplateIntegrity,
            GemmaE4bVibeEvalCaseKind::Steerability,
            GemmaE4bVibeEvalCaseKind::ToolUse,
            GemmaE4bVibeEvalCaseKind::Formatting,
        ]);
        if kinds != expected_kinds {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("vibe_packet.cases[].case_kind"),
                detail: String::from(
                    "vibe packet case kinds drifted from the canonical review set",
                ),
            });
        }
        for case in &self.cases {
            ensure_nonempty(case.case_id.as_str(), "vibe_packet.cases[].case_id")?;
            ensure_nonempty(case.prompt.as_str(), "vibe_packet.cases[].prompt")?;
            ensure_nonempty(
                case.expected_property.as_str(),
                "vibe_packet.cases[].expected_property",
            )?;
        }
        if self.packet_digest != self.stable_digest() {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("vibe_packet.packet_digest"),
                detail: String::from("vibe packet digest drifted"),
            });
        }
        Ok(())
    }
}

/// One operator verdict row over one vibe-eval case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bOperatorReviewCaseVerdict {
    /// Stable case identifier.
    pub case_id: String,
    /// Pass or fail decision.
    pub status: GemmaE4bReviewVerdictStatus,
    /// Short operator-facing detail.
    pub detail: String,
}

/// Early operator review required before promotion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bOperatorPromotionReview {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable review identifier.
    pub review_id: String,
    /// Stable packet id consumed by the review.
    pub packet_id: String,
    /// Reviewer identifier.
    pub reviewer_id: String,
    /// Current review state.
    pub state: GemmaE4bOperatorReviewState,
    /// Case verdicts recorded so far.
    pub case_verdicts: Vec<GemmaE4bOperatorReviewCaseVerdict>,
    /// Short summary.
    pub summary: String,
    /// Stable digest over the review.
    pub review_digest: String,
}

impl GemmaE4bOperatorPromotionReview {
    /// Returns the stable digest over the review payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.review_digest.clear();
        stable_digest(b"psionic_gemma_e4b_operator_review|", &clone)
    }

    /// Validates the review against the canned vibe packet.
    pub fn validate(
        &self,
        vibe_packet: &GemmaE4bPromotedCheckpointVibePacket,
    ) -> Result<(), GemmaE4bFinetuneEvalError> {
        ensure_exact(
            self.schema_version.as_str(),
            "operator_review.schema_version",
            GEMMA_E4B_OPERATOR_REVIEW_SCHEMA_VERSION,
        )?;
        ensure_nonempty(self.review_id.as_str(), "operator_review.review_id")?;
        ensure_exact(
            self.packet_id.as_str(),
            "operator_review.packet_id",
            vibe_packet.packet_id.as_str(),
        )?;
        ensure_nonempty(self.reviewer_id.as_str(), "operator_review.reviewer_id")?;
        ensure_nonempty(self.summary.as_str(), "operator_review.summary")?;
        let allowed_case_ids = vibe_packet
            .cases
            .iter()
            .map(|case| case.case_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut seen_case_ids = BTreeSet::new();
        for verdict in &self.case_verdicts {
            ensure_nonempty(
                verdict.case_id.as_str(),
                "operator_review.case_verdicts[].case_id",
            )?;
            if !allowed_case_ids.contains(verdict.case_id.as_str()) {
                return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                    field: String::from("operator_review.case_verdicts[].case_id"),
                    detail: format!(
                        "operator review referenced unknown vibe-eval case `{}`",
                        verdict.case_id
                    ),
                });
            }
            if !seen_case_ids.insert(verdict.case_id.as_str()) {
                return Err(GemmaE4bFinetuneEvalError::DuplicateId {
                    field: String::from("operator_review.case_verdicts[].case_id"),
                    value: verdict.case_id.clone(),
                });
            }
            ensure_nonempty(
                verdict.detail.as_str(),
                "operator_review.case_verdicts[].detail",
            )?;
        }
        if self.state != GemmaE4bOperatorReviewState::Pending
            && seen_case_ids.len() != allowed_case_ids.len()
        {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("operator_review.case_verdicts"),
                detail: String::from(
                    "approved or rejected reviews must retain a verdict for every vibe-eval case",
                ),
            });
        }
        let has_failures = self
            .case_verdicts
            .iter()
            .any(|verdict| verdict.status == GemmaE4bReviewVerdictStatus::Failed);
        match self.state {
            GemmaE4bOperatorReviewState::Pending => {}
            GemmaE4bOperatorReviewState::Approved => {
                if has_failures {
                    return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                        field: String::from("operator_review.state"),
                        detail: String::from(
                            "approved operator reviews cannot retain failed vibe-eval verdicts",
                        ),
                    });
                }
            }
            GemmaE4bOperatorReviewState::Rejected => {
                if !has_failures {
                    return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                        field: String::from("operator_review.state"),
                        detail: String::from(
                            "rejected operator reviews must retain at least one failed vibe-eval verdict",
                        ),
                    });
                }
            }
        }
        if self.review_digest != self.stable_digest() {
            return Err(GemmaE4bFinetuneEvalError::InvalidValue {
                field: String::from("operator_review.review_digest"),
                detail: String::from("operator review digest drifted"),
            });
        }
        Ok(())
    }
}

/// One gate row in the bounded promotion decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bPromotionGateRow {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Gate status.
    pub status: GemmaE4bPromotionGateStatus,
    /// Short detail.
    pub detail: String,
}

/// Final bounded promotion decision for one checkpoint candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemmaE4bCheckpointPromotionDecision {
    /// Stable schema version.
    pub schema_version: String,
    /// Candidate checkpoint identifier.
    pub checkpoint_id: String,
    /// Candidate checkpoint digest.
    pub checkpoint_digest: String,
    /// Eval pack storage key.
    pub benchmark_package_storage_key: String,
    /// Final decision state.
    pub decision_state: GemmaE4bPromotionDecisionState,
    /// Gate rows scored by the decision.
    pub gate_rows: Vec<GemmaE4bPromotionGateRow>,
    /// Short detail.
    pub detail: String,
    /// Stable digest over the decision.
    pub decision_digest: String,
}

impl GemmaE4bCheckpointPromotionDecision {
    /// Returns the stable digest over the decision payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.decision_digest.clear();
        stable_digest(b"psionic_gemma_e4b_promotion_decision|", &clone)
    }
}

/// Errors surfaced by the bounded Gemma e4b eval-first lane.
#[derive(Debug, Error)]
pub enum GemmaE4bFinetuneEvalError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("invalid value for `{field}`: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("duplicate value for `{field}`: `{value}`")]
    DuplicateId { field: String, value: String },
    #[error("field `{field}` must stay `{expected}` but was `{actual}`")]
    ExactMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("baseline sweep did not retain any candidate results")]
    MissingBaselineResults,
    #[error(transparent)]
    Mvp(#[from] GemmaE4bFinetuningMvpError),
    #[error(transparent)]
    EvalRuntime(#[from] psionic_eval::EvalRuntimeError),
    #[error("gemma e4b sft lane error: {detail}")]
    Sft { detail: String },
}

/// Returns the canonical eval-pack binding for the bounded Gemma e4b lane.
pub fn canonical_gemma_e4b_finetune_eval_pack_binding()
-> Result<GemmaE4bFinetuneEvalPackBinding, GemmaE4bFinetuneEvalError> {
    let package = build_gemma_e4b_finetune_eval_benchmark_package()?;
    let mut binding = GemmaE4bFinetuneEvalPackBinding {
        schema_version: String::from(GEMMA_E4B_FINETUNE_EVAL_PACK_BINDING_SCHEMA_VERSION),
        benchmark_package_storage_key: package.key.storage_key(),
        benchmark_package_digest: package.stable_digest(),
        required_vibe_packet_template_id: String::from(
            GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_TEMPLATE_ID,
        ),
        claim_boundary: String::from(
            "This binding freezes one held-out eval package plus one required promoted-checkpoint vibe-review packet for the bounded Gemma e4b adapter-SFT lane.",
        ),
        binding_digest: String::new(),
    };
    binding.binding_digest = binding.stable_digest();
    Ok(binding)
}

/// Runs the narrow baseline sweep for newly uploaded Gemma datasets.
pub fn run_gemma_e4b_baseline_sweep(
    base_binding: GemmaE4bServedBaseModelBinding,
    target_set: GemmaE4bCudaAdapterTargetSet,
    train_samples: Vec<GemmaE4bLmHeadSupervisionSample>,
    held_out_validation_samples: Vec<GemmaE4bLmHeadSupervisionSample>,
    request: &GemmaE4bBaselineSweepRequest,
) -> Result<GemmaE4bBaselineSweepOutcome, GemmaE4bFinetuneEvalError> {
    request.validate()?;
    let mut candidate_results = Vec::new();
    for (ordinal, candidate) in request.candidates.iter().enumerate() {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            candidate.config.clone(),
            target_set.clone(),
            base_binding.clone(),
            train_samples.clone(),
        )?;
        let mut run = trainer.initialize_run()?;
        let train_progress = trainer.advance_run(
            &mut run,
            Some(candidate.max_steps),
            request.started_at_ms,
            request.step_duration_ms,
        )?;
        let exported_artifact = trainer.export_run_artifact(
            &run,
            &GemmaE4bCudaAdapterExportRequest {
                dataset_ref: request.dataset_contract.dataset_ref.clone(),
                validator_policy_ref: request.validator_policy_ref.clone(),
                adapter_id: format!("{}-{}", request.adapter_id_prefix, candidate.candidate_id),
                adapter_revision: format!(
                    "{}-{}",
                    request.adapter_revision_prefix, candidate.candidate_id
                ),
            },
        )?;
        let validation_trainer = GemmaE4bCudaAdapterSftTrainer::new(
            candidate.config.clone(),
            target_set.clone(),
            base_binding.clone(),
            held_out_validation_samples.clone(),
        )?;
        let mut validation_run = validation_trainer
            .initialize_run_from_loaded_adapter(&exported_artifact.load_lm_head_lora_artifact()?)?;
        // The first gradient record is the held-out loss before the ephemeral validation step mutates the scratch run.
        let validation_progress = validation_trainer.advance_run(
            &mut validation_run,
            Some(1),
            request.started_at_ms + ((ordinal as u64 + 1) * request.step_duration_ms),
            request.step_duration_ms,
        )?;
        let held_out_validation_mean_loss = validation_progress
            .gradient_records
            .first()
            .map(|record| record.mean_loss)
            .ok_or(GemmaE4bFinetuneEvalError::MissingBaselineResults)?;
        candidate_results.push(GemmaE4bBaselineCandidateResult {
            candidate_id: candidate.candidate_id.clone(),
            run_summary: run.summary(),
            final_train_mean_loss: train_progress
                .gradient_records
                .last()
                .map(|record| record.mean_loss),
            held_out_validation_mean_loss,
            detail: candidate.detail.clone(),
        });
    }
    let recommended_candidate_id = candidate_results
        .iter()
        .min_by(|left, right| {
            left.held_out_validation_mean_loss
                .partial_cmp(&right.held_out_validation_mean_loss)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|result| result.candidate_id.clone())
        .ok_or(GemmaE4bFinetuneEvalError::MissingBaselineResults)?;
    Ok(GemmaE4bBaselineSweepOutcome {
        dataset_ref: request.dataset_contract.dataset_ref.clone(),
        eval_pack_storage_key: request
            .eval_pack_binding
            .benchmark_package_storage_key
            .clone(),
        held_out_validation_split_ref: request
            .dataset_contract
            .held_out_validation_split_ref
            .clone(),
        candidate_results,
        recommended_candidate_id,
    })
}

/// Returns the canned vibe-eval packet for one promoted checkpoint candidate.
pub fn canonical_gemma_e4b_promoted_checkpoint_vibe_packet(
    checkpoint: &GemmaE4bCudaAdapterCheckpoint,
) -> Result<GemmaE4bPromotedCheckpointVibePacket, GemmaE4bFinetuneEvalError> {
    checkpoint.validate()?;
    let mut packet = GemmaE4bPromotedCheckpointVibePacket {
        schema_version: String::from(GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_SCHEMA_VERSION),
        packet_template_id: String::from(GEMMA_E4B_PROMOTED_CHECKPOINT_VIBE_PACKET_TEMPLATE_ID),
        packet_id: format!("{}.promoted-vibe-eval", checkpoint.checkpoint_id),
        checkpoint_id: checkpoint.checkpoint_id.clone(),
        checkpoint_digest: checkpoint.checkpoint_digest.clone(),
        cases: vec![
            GemmaE4bVibeEvalCase {
                case_id: String::from("template_integrity"),
                case_kind: GemmaE4bVibeEvalCaseKind::TemplateIntegrity,
                prompt: String::from(
                    "Continue a two-turn Gemma conversation without leaking raw `<|turn>` markers or swapping user and model roles.",
                ),
                expected_property: String::from(
                    "The checkpoint preserves bounded Gemma turn formatting and does not emit raw template scaffolding.",
                ),
            },
            GemmaE4bVibeEvalCase {
                case_id: String::from("steerability"),
                case_kind: GemmaE4bVibeEvalCaseKind::Steerability,
                prompt: String::from(
                    "Developer: answer in one sentence. User: explain why held-out validation matters.",
                ),
                expected_property: String::from(
                    "The checkpoint stays terse under developer instruction while still answering the user.",
                ),
            },
            GemmaE4bVibeEvalCase {
                case_id: String::from("tool_use"),
                case_kind: GemmaE4bVibeEvalCaseKind::ToolUse,
                prompt: String::from(
                    "What's the weather in Paris? Use the tool if needed and preserve the Gemma-native tool-call block shape.",
                ),
                expected_property: String::from(
                    "The checkpoint emits one well-formed Gemma-native tool call instead of prose or malformed JSON.",
                ),
            },
            GemmaE4bVibeEvalCase {
                case_id: String::from("formatting"),
                case_kind: GemmaE4bVibeEvalCaseKind::Formatting,
                prompt: String::from(
                    "Return exactly two markdown bullets describing promotion gates and no preamble.",
                ),
                expected_property: String::from(
                    "The checkpoint preserves requested output formatting without extra framing.",
                ),
            },
        ],
        packet_digest: String::new(),
    };
    packet.packet_digest = packet.stable_digest();
    Ok(packet)
}

/// Scores the final promotion decision for one checkpoint candidate.
pub fn decide_gemma_e4b_checkpoint_promotion(
    checkpoint: &GemmaE4bCudaAdapterCheckpoint,
    dataset_contract: &GemmaE4bFinetuneDatasetContract,
    eval_pack_binding: &GemmaE4bFinetuneEvalPackBinding,
    untuned_base_eval: &GemmaE4bFinetuneEvalReceipt,
    candidate_eval: &GemmaE4bFinetuneEvalReceipt,
    vibe_packet: &GemmaE4bPromotedCheckpointVibePacket,
    operator_review: &GemmaE4bOperatorPromotionReview,
) -> Result<GemmaE4bCheckpointPromotionDecision, GemmaE4bFinetuneEvalError> {
    checkpoint.validate()?;
    dataset_contract.validate()?;
    eval_pack_binding.validate()?;
    untuned_base_eval.validate(dataset_contract, eval_pack_binding)?;
    candidate_eval.validate(dataset_contract, eval_pack_binding)?;
    vibe_packet.validate(checkpoint)?;
    operator_review.validate(vibe_packet)?;

    ensure_exact(
        untuned_base_eval.subject_kind_label(),
        "untuned_base_eval.subject_kind",
        "untuned_base",
    )?;
    ensure_exact(
        candidate_eval.subject_kind_label(),
        "candidate_eval.subject_kind",
        "checkpoint_candidate",
    )?;
    ensure_exact(
        candidate_eval.subject_id.as_str(),
        "candidate_eval.subject_id",
        checkpoint.checkpoint_id.as_str(),
    )?;
    ensure_exact(
        candidate_eval.subject_digest.as_str(),
        "candidate_eval.subject_digest",
        checkpoint.checkpoint_digest.as_str(),
    )?;

    let automatic_clearance = candidate_eval.automatic_clearance();
    let held_out_score_non_regression =
        candidate_eval.held_out_score_bps >= untuned_base_eval.held_out_score_bps;
    let held_out_pass_rate_non_regression =
        candidate_eval.held_out_pass_rate_bps >= untuned_base_eval.held_out_pass_rate_bps;
    let operator_review_green = operator_review.state == GemmaE4bOperatorReviewState::Approved
        && operator_review
            .case_verdicts
            .iter()
            .all(|verdict| verdict.status == GemmaE4bReviewVerdictStatus::Passed);

    let review_status = match operator_review.state {
        GemmaE4bOperatorReviewState::Pending => GemmaE4bPromotionGateStatus::Held,
        GemmaE4bOperatorReviewState::Approved => GemmaE4bPromotionGateStatus::Passed,
        GemmaE4bOperatorReviewState::Rejected => GemmaE4bPromotionGateStatus::Failed,
    };
    let gate_rows = vec![
        GemmaE4bPromotionGateRow {
            gate_id: String::from("held_out_score_non_regression"),
            status: if held_out_score_non_regression {
                GemmaE4bPromotionGateStatus::Passed
            } else {
                GemmaE4bPromotionGateStatus::Failed
            },
            detail: format!(
                "candidate held-out score {} bps vs untuned base {} bps",
                candidate_eval.held_out_score_bps, untuned_base_eval.held_out_score_bps
            ),
        },
        GemmaE4bPromotionGateRow {
            gate_id: String::from("held_out_pass_rate_non_regression"),
            status: if held_out_pass_rate_non_regression {
                GemmaE4bPromotionGateStatus::Passed
            } else {
                GemmaE4bPromotionGateStatus::Failed
            },
            detail: format!(
                "candidate held-out pass rate {} bps vs untuned base {} bps",
                candidate_eval.held_out_pass_rate_bps, untuned_base_eval.held_out_pass_rate_bps
            ),
        },
        GemmaE4bPromotionGateRow {
            gate_id: String::from("automatic_surface_clearance"),
            status: if automatic_clearance {
                GemmaE4bPromotionGateStatus::Passed
            } else {
                GemmaE4bPromotionGateStatus::Failed
            },
            detail: format!(
                "chat_template={} assistant_mask={} tool_call_format={} formatting={} steerability={}",
                candidate_eval.chat_template_passed,
                candidate_eval.assistant_mask_passed,
                candidate_eval.tool_call_format_passed,
                candidate_eval.formatting_passed,
                candidate_eval.steerability_passed
            ),
        },
        GemmaE4bPromotionGateRow {
            gate_id: String::from("operator_vibe_review"),
            status: review_status,
            detail: format!(
                "operator review state `{}` retained {} verdict rows",
                operator_review.state_label(),
                operator_review.case_verdicts.len()
            ),
        },
    ];
    let decision_state = if !held_out_score_non_regression
        || !held_out_pass_rate_non_regression
        || !automatic_clearance
        || operator_review.state == GemmaE4bOperatorReviewState::Rejected
    {
        GemmaE4bPromotionDecisionState::Reject
    } else if !operator_review_green {
        GemmaE4bPromotionDecisionState::HoldForReview
    } else {
        GemmaE4bPromotionDecisionState::Promote
    };
    let mut decision = GemmaE4bCheckpointPromotionDecision {
        schema_version: String::from(GEMMA_E4B_PROMOTION_DECISION_SCHEMA_VERSION),
        checkpoint_id: checkpoint.checkpoint_id.clone(),
        checkpoint_digest: checkpoint.checkpoint_digest.clone(),
        benchmark_package_storage_key: eval_pack_binding.benchmark_package_storage_key.clone(),
        decision_state,
        gate_rows,
        detail: String::from(
            "Promotion stays blocked until held-out quality clears the untuned base, automatic template or formatting checks stay green, and the early operator vibe review approves the candidate.",
        ),
        decision_digest: String::new(),
    };
    decision.decision_digest = decision.stable_digest();
    Ok(decision)
}

impl GemmaE4bFinetuneEvalReceipt {
    fn subject_kind_label(&self) -> &'static str {
        match self.subject_kind {
            GemmaE4bFinetuneEvalSubjectKind::UntunedBase => "untuned_base",
            GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate => "checkpoint_candidate",
        }
    }
}

impl GemmaE4bOperatorPromotionReview {
    fn state_label(&self) -> &'static str {
        match self.state {
            GemmaE4bOperatorReviewState::Pending => "pending",
            GemmaE4bOperatorReviewState::Approved => "approved",
            GemmaE4bOperatorReviewState::Rejected => "rejected",
        }
    }
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), GemmaE4bFinetuneEvalError> {
    if value.trim().is_empty() {
        return Err(GemmaE4bFinetuneEvalError::MissingField {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), GemmaE4bFinetuneEvalError> {
    if actual != expected {
        return Err(GemmaE4bFinetuneEvalError::ExactMismatch {
            field: field.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn stable_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).expect("payload should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

impl From<GemmaE4bCudaAdapterSftError> for GemmaE4bFinetuneEvalError {
    fn from(error: GemmaE4bCudaAdapterSftError) -> Self {
        Self::Sft {
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GemmaE4bCudaAdapterSftConfig, TrainingLoopBudget, TrainingOptimizerConfig,
        TrainingOptimizerResidencyPolicy, canonical_gemma_e4b_cuda_adapter_target_set,
    };

    fn sample_binding() -> GemmaE4bServedBaseModelBinding {
        let contract = canonical_gemma_e4b_finetuning_mvp_contract().expect("contract");
        GemmaE4bServedBaseModelBinding {
            model_id: contract.model_id,
            base_model_revision: contract.base_model_revision,
            base_served_artifact_digest: String::from("sha256:gemma4-e4b-base"),
            tokenizer: contract.tokenizer,
            hidden_size: 4,
        }
    }

    fn sample_train_samples() -> Vec<GemmaE4bLmHeadSupervisionSample> {
        vec![
            GemmaE4bLmHeadSupervisionSample::new("train-a", vec![1.0, 0.0, 0.0, 0.0], 48, 11),
            GemmaE4bLmHeadSupervisionSample::new("train-b", vec![0.0, 1.0, 0.0, 0.0], 106, 12),
            GemmaE4bLmHeadSupervisionSample::new("train-c", vec![0.0, 0.0, 1.0, 0.0], 50, 10),
            GemmaE4bLmHeadSupervisionSample::new("train-d", vec![0.0, 0.0, 0.0, 1.0], 1, 9),
        ]
    }

    fn sample_validation_samples() -> Vec<GemmaE4bLmHeadSupervisionSample> {
        vec![
            GemmaE4bLmHeadSupervisionSample::new("val-a", vec![0.8, 0.2, 0.0, 0.0], 48, 11),
            GemmaE4bLmHeadSupervisionSample::new("val-b", vec![0.0, 0.8, 0.2, 0.0], 106, 12),
            GemmaE4bLmHeadSupervisionSample::new("val-c", vec![0.0, 0.0, 0.8, 0.2], 50, 10),
            GemmaE4bLmHeadSupervisionSample::new("val-d", vec![0.2, 0.0, 0.0, 0.8], 1, 9),
        ]
    }

    fn sample_dataset_contract() -> GemmaE4bFinetuneDatasetContract {
        let template_digest = canonical_gemma_e4b_finetuning_mvp_contract()
            .expect("contract")
            .tokenizer
            .template_digest
            .expect("template digest");
        let mut contract = GemmaE4bFinetuneDatasetContract {
            schema_version: String::from(GEMMA_E4B_FINETUNE_DATASET_CONTRACT_SCHEMA_VERSION),
            dataset_ref: String::from("dataset://openagents/gemma4-e4b-helpdesk@2026.04"),
            train_split_ref: String::from("split://gemma4-e4b-helpdesk/train"),
            held_out_validation_split_ref: String::from(
                "split://gemma4-e4b-helpdesk/held_out_validation",
            ),
            final_report_split_ref: String::from("split://gemma4-e4b-helpdesk/final_report"),
            baseline_short_split_ref: String::from("split://gemma4-e4b-helpdesk/baseline_short"),
            chat_template_digest: template_digest,
            assistant_mask_kind: GemmaE4bAssistantMaskKind::AssistantResponsesOnly,
            assistant_mask_coverage_bps: 10_000,
            benchmark_overlap_check: GemmaE4bBenchmarkOverlapCheck {
                check_id: String::from("gemma4-e4b-helpdesk-overlap-check"),
                compared_benchmark_refs: vec![
                    String::from("benchmark://psionic/gemma4/e4b/finetune_eval"),
                    String::from("benchmark://psion/actual_pretraining/checkpoint_eval"),
                ],
                exact_overlap_refs: Vec::new(),
                near_duplicate_overlap_refs: Vec::new(),
                passed: true,
                detail: String::from("curated dataset cleared overlap review"),
            },
            dataset_digest: String::new(),
        };
        contract.dataset_digest = contract.stable_digest();
        contract
    }

    fn sample_eval_pack_binding() -> GemmaE4bFinetuneEvalPackBinding {
        canonical_gemma_e4b_finetune_eval_pack_binding().expect("eval-pack binding")
    }

    fn sample_candidate_config(run_id: &str, learning_rate: f32) -> GemmaE4bCudaAdapterSftConfig {
        GemmaE4bCudaAdapterSftConfig {
            run_id: run_id.to_string(),
            budget: TrainingLoopBudget::new(4, 1, 1).expect("budget"),
            batch_size: 2,
            optimizer: TrainingOptimizerConfig::adamw(learning_rate, 0.9, 0.99, 1e-8)
                .with_gradient_clip_norm(1.0),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
        }
    }

    fn sample_eval_receipt(
        subject_kind: GemmaE4bFinetuneEvalSubjectKind,
        subject_id: &str,
        subject_digest: &str,
        held_out_pass_rate_bps: u32,
        held_out_score_bps: u32,
    ) -> GemmaE4bFinetuneEvalReceipt {
        let binding = sample_eval_pack_binding();
        let dataset = sample_dataset_contract();
        let mut receipt = GemmaE4bFinetuneEvalReceipt {
            schema_version: String::from(GEMMA_E4B_FINETUNE_EVAL_RECEIPT_SCHEMA_VERSION),
            receipt_id: format!("{subject_id}.eval"),
            subject_kind,
            subject_id: subject_id.to_string(),
            subject_digest: subject_digest.to_string(),
            benchmark_package_storage_key: binding.benchmark_package_storage_key.clone(),
            benchmark_package_digest: binding.benchmark_package_digest.clone(),
            held_out_validation_split_ref: dataset.held_out_validation_split_ref.clone(),
            final_report_split_ref: dataset.final_report_split_ref.clone(),
            held_out_pass_rate_bps,
            held_out_score_bps,
            chat_template_passed: true,
            assistant_mask_passed: true,
            tool_call_format_passed: true,
            formatting_passed: true,
            steerability_passed: true,
            detail: String::from("bounded eval stayed green"),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        receipt
    }

    fn sample_operator_review(
        packet: &GemmaE4bPromotedCheckpointVibePacket,
        state: GemmaE4bOperatorReviewState,
        failed_case_id: Option<&str>,
    ) -> GemmaE4bOperatorPromotionReview {
        let verdicts = packet
            .cases
            .iter()
            .map(|case| GemmaE4bOperatorReviewCaseVerdict {
                case_id: case.case_id.clone(),
                status: if Some(case.case_id.as_str()) == failed_case_id {
                    GemmaE4bReviewVerdictStatus::Failed
                } else {
                    GemmaE4bReviewVerdictStatus::Passed
                },
                detail: format!("checked {}", case.case_id),
            })
            .collect::<Vec<_>>();
        let mut review = GemmaE4bOperatorPromotionReview {
            schema_version: String::from(GEMMA_E4B_OPERATOR_REVIEW_SCHEMA_VERSION),
            review_id: String::from("gemma4-e4b-operator-review"),
            packet_id: packet.packet_id.clone(),
            reviewer_id: String::from("operator-1"),
            state,
            case_verdicts: verdicts,
            summary: String::from("bounded operator review complete"),
            review_digest: String::new(),
        };
        review.review_digest = review.stable_digest();
        review
    }

    #[test]
    fn gemma_e4b_dataset_contract_rejects_template_drift() {
        let mut dataset = sample_dataset_contract();
        dataset.chat_template_digest = String::from("drifted");
        dataset.dataset_digest = dataset.stable_digest();
        let error = dataset.validate().expect_err("template drift must refuse");
        assert!(error.to_string().contains("chat template digest"));
    }

    #[test]
    fn gemma_e4b_baseline_sweep_returns_recommended_candidate()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GemmaE4bBaselineSweepRequest {
            schema_version: String::from(GEMMA_E4B_BASELINE_SWEEP_REQUEST_SCHEMA_VERSION),
            dataset_contract: sample_dataset_contract(),
            eval_pack_binding: sample_eval_pack_binding(),
            validator_policy_ref: String::from("policy://validator/gemma4/e4b-text-sft"),
            adapter_id_prefix: String::from("gemma4-e4b-baseline"),
            adapter_revision_prefix: String::from("r"),
            candidates: vec![
                GemmaE4bBaselineCandidate {
                    candidate_id: String::from("lr-low"),
                    config: sample_candidate_config("gemma-e4b-baseline-low", 0.02),
                    max_steps: 2,
                    detail: String::from("conservative short sweep"),
                },
                GemmaE4bBaselineCandidate {
                    candidate_id: String::from("lr-high"),
                    config: sample_candidate_config("gemma-e4b-baseline-high", 0.12),
                    max_steps: 2,
                    detail: String::from("faster short sweep"),
                },
            ],
            started_at_ms: 1_000,
            step_duration_ms: 25,
        };
        let outcome = run_gemma_e4b_baseline_sweep(
            sample_binding(),
            canonical_gemma_e4b_cuda_adapter_target_set(),
            sample_train_samples(),
            sample_validation_samples(),
            &request,
        )?;
        assert_eq!(outcome.candidate_results.len(), 2);
        assert!(
            outcome
                .candidate_results
                .iter()
                .all(|result| result.held_out_validation_mean_loss > 0.0)
        );
        assert!(
            outcome
                .candidate_results
                .iter()
                .any(|result| result.candidate_id == outcome.recommended_candidate_id)
        );
        Ok(())
    }

    #[test]
    fn gemma_e4b_promotion_decision_rejects_regression_against_untuned_base()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            sample_candidate_config("gemma-e4b-promotion", 0.08),
            canonical_gemma_e4b_cuda_adapter_target_set(),
            sample_binding(),
            sample_train_samples(),
        )?;
        let outcome = trainer.run_sft(&crate::GemmaE4bCudaAdapterSftRunRequest {
            dataset_ref: String::from("dataset://openagents/gemma4-e4b-helpdesk@2026.04"),
            validator_policy_ref: String::from("policy://validator/gemma4/e4b-text-sft"),
            adapter_id: String::from("gemma4-e4b-helpdesk"),
            adapter_revision: String::from("r1"),
            eval_pack_binding: sample_eval_pack_binding(),
            dataset_contract: sample_dataset_contract(),
            started_at_ms: 1_000,
            step_duration_ms: 25,
        })?;
        let packet =
            canonical_gemma_e4b_promoted_checkpoint_vibe_packet(&outcome.final_checkpoint)?;
        let review = sample_operator_review(&packet, GemmaE4bOperatorReviewState::Approved, None);
        let base_eval = sample_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::UntunedBase,
            "gemma4:e4b@v1",
            "sha256:gemma4-e4b-base",
            8600,
            8500,
        );
        let candidate_eval = sample_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate,
            outcome.final_checkpoint.checkpoint_id.as_str(),
            outcome.final_checkpoint.checkpoint_digest.as_str(),
            8500,
            8400,
        );
        let decision = decide_gemma_e4b_checkpoint_promotion(
            &outcome.final_checkpoint,
            &sample_dataset_contract(),
            &sample_eval_pack_binding(),
            &base_eval,
            &candidate_eval,
            &packet,
            &review,
        )?;
        assert_eq!(
            decision.decision_state,
            GemmaE4bPromotionDecisionState::Reject
        );
        Ok(())
    }

    #[test]
    fn gemma_e4b_promotion_decision_rejects_failed_manual_vibe_review_even_when_automatic_metrics_are_green()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = GemmaE4bCudaAdapterSftTrainer::new(
            sample_candidate_config("gemma-e4b-promotion-manual", 0.08),
            canonical_gemma_e4b_cuda_adapter_target_set(),
            sample_binding(),
            sample_train_samples(),
        )?;
        let outcome = trainer.run_sft(&crate::GemmaE4bCudaAdapterSftRunRequest {
            dataset_ref: String::from("dataset://openagents/gemma4-e4b-helpdesk@2026.04"),
            validator_policy_ref: String::from("policy://validator/gemma4/e4b-text-sft"),
            adapter_id: String::from("gemma4-e4b-helpdesk"),
            adapter_revision: String::from("r1"),
            eval_pack_binding: sample_eval_pack_binding(),
            dataset_contract: sample_dataset_contract(),
            started_at_ms: 1_000,
            step_duration_ms: 25,
        })?;
        let packet =
            canonical_gemma_e4b_promoted_checkpoint_vibe_packet(&outcome.final_checkpoint)?;
        let review = sample_operator_review(
            &packet,
            GemmaE4bOperatorReviewState::Rejected,
            Some("formatting"),
        );
        let base_eval = sample_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::UntunedBase,
            "gemma4:e4b@v1",
            "sha256:gemma4-e4b-base",
            8200,
            8200,
        );
        let candidate_eval = sample_eval_receipt(
            GemmaE4bFinetuneEvalSubjectKind::CheckpointCandidate,
            outcome.final_checkpoint.checkpoint_id.as_str(),
            outcome.final_checkpoint.checkpoint_digest.as_str(),
            8400,
            8450,
        );
        let decision = decide_gemma_e4b_checkpoint_promotion(
            &outcome.final_checkpoint,
            &sample_dataset_contract(),
            &sample_eval_pack_binding(),
            &base_eval,
            &candidate_eval,
            &packet,
            &review,
        )?;
        assert_eq!(
            decision.decision_state,
            GemmaE4bPromotionDecisionState::Reject
        );
        assert!(
            decision
                .gate_rows
                .iter()
                .any(|row| row.gate_id == "operator_vibe_review")
        );
        Ok(())
    }
}
