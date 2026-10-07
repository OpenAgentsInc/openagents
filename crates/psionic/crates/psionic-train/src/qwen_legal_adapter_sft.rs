use std::path::Path;

use psionic_adapters::{AdapterArtifactIdentity, LmHeadLoraAdapterArtifact, LmHeadLoraLoadError};
use psionic_data::{
    LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION, TokenizerDigest, TokenizerFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    FixedBudgetTrainingRun, ModelAdapterDelta, ModelIoArtifactReceipt, ModelIoError,
    OPEN_ADAPTER_QWEN35_LEGAL_ADAPTER_FAMILY, OPEN_ADAPTER_QWEN35_LEGAL_CUDA_BACKEND_LABEL,
    OpenAdapterAdmissibleModelFamily, OpenAdapterArtifactExportRequest,
    OpenAdapterGradientBatchRecord, OpenAdapterHiddenStateSample, OpenAdapterLmHeadTarget,
    OpenAdapterPrecisionPolicy, OpenAdapterReferenceModel, OpenAdapterSftError,
    OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError, PortableModelBundle,
    PortableTokenizerAssetFormat, PortableTokenizerBinding, TrainingCoreError, TrainingLoopBudget,
    TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy, TrainingRunSummary,
    TrainingStepReceipt,
};

/// Stable lane id for the first legal benchmark Qwen adapter smoke.
pub const QWEN_LEGAL_ADAPTER_SFT_LANE_ID: &str = "qwen_legal_adapter_sft_v1";
/// Public base-model id for the bounded smoke lane.
pub const QWEN35_4B_LEGAL_SMOKE_MODEL_ID: &str = "Qwen/Qwen3.5-4B";
/// Stable served-model id used by Psionic serving and eval metadata.
pub const QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID: &str = "qwen3.5-4b";
/// Retained target model for the Harvey legal hillclimb lane.
pub const QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID: &str = "Qwen/Qwen3.6-35B-A3B";
/// Stable served-model id for the retained target lane.
pub const QWEN36_35B_A3B_LEGAL_RETAINED_SERVED_MODEL_ID: &str = "qwen3.6-35b-a3b";
/// Model-family acceptance label expected by Psionic qwen35 runtime gates.
pub const QWEN35_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL: &str = "qwen35";
/// Stable checkpoint family for the first Qwen legal adapter lane.
pub const QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY: &str = "psionic.qwen35_4b.legal_adapter_sft";
/// Stable schema version for typed Qwen legal adapter checkpoints.
pub const QWEN_LEGAL_ADAPTER_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_adapter_checkpoint.v1";
/// Stable schema version for Autopilot4-importable score metadata.
pub const QWEN_LEGAL_SCORE_IMPORT_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_score_import_bundle.v1";
/// Stable schema version for the legal RL hillclimb plan.
pub const QWEN_LEGAL_RL_HILLCLIMB_PLAN_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_hillclimb_plan.v1";
/// Stable schema version for the legal RL benchmark readiness report.
pub const QWEN_LEGAL_RL_BENCHMARK_REPORT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_benchmark_report.v1";
/// Stable schema version for the legal RL optimization window report.
pub const QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_optimization_window.v1";
/// Stable schema version for the legal RL perfect-score push report.
pub const QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_perfect_score_push.v1";
/// Stable schema version for the legal RL retained rehearsal report.
pub const QWEN_LEGAL_RL_RETAINED_REHEARSAL_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_retained_rehearsal.v1";
/// Stable schema version for the legal RL expanded corpus report.
pub const QWEN_LEGAL_RL_EXPANDED_CORPUS_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_expanded_corpus.v1";
/// Stable schema version for the legal RL full-corpus matrix report.
pub const QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_full_corpus_matrix.v1";
/// Stable schema version for the legal RL residual burn-down report.
pub const QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_residual_burn_down.v1";
/// Stable schema version for the legal RL final-campaign rehearsal report.
pub const QWEN_LEGAL_RL_FINAL_CAMPAIGN_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_rl_final_campaign.v1";
/// Stable plan id for the next Harvey legal hillclimb phase.
pub const QWEN_LEGAL_RL_HILLCLIMB_PLAN_ID: &str = "qwen_legal_rl_hillclimb_plan_v1";
/// Stable report id for the next Harvey legal RL benchmark projection.
pub const QWEN_LEGAL_RL_BENCHMARK_REPORT_ID: &str = "qwen_legal_rl_benchmark_report_phase_002";
/// Stable report id for the phase-three Harvey legal RL optimization window.
pub const QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_REPORT_ID: &str =
    "qwen_legal_rl_optimization_window_phase_003";
/// Stable report id for the phase-four Harvey legal perfect-score push.
pub const QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_REPORT_ID: &str =
    "qwen_legal_rl_perfect_score_push_phase_004";
/// Stable report id for the phase-five Harvey legal retained rehearsal.
pub const QWEN_LEGAL_RL_RETAINED_REHEARSAL_REPORT_ID: &str =
    "qwen_legal_rl_retained_rehearsal_phase_005";
/// Stable report id for the phase-six Harvey legal expanded corpus dry run.
pub const QWEN_LEGAL_RL_EXPANDED_CORPUS_REPORT_ID: &str = "qwen_legal_rl_expanded_corpus_phase_006";
/// Stable report id for the phase-seven Harvey legal full-corpus matrix dry run.
pub const QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_REPORT_ID: &str =
    "qwen_legal_rl_full_corpus_matrix_phase_007";
/// Stable report id for the phase-eight Harvey legal residual burn-down dry run.
pub const QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_REPORT_ID: &str =
    "qwen_legal_rl_residual_burn_down_phase_008";
/// Stable report id for the phase-nine Harvey legal final-campaign rehearsal.
pub const QWEN_LEGAL_RL_FINAL_CAMPAIGN_REPORT_ID: &str = "qwen_legal_rl_final_campaign_phase_009";
/// Phase-two retained target aligned with Blueprint optimizer batch `phase_002`.
pub const QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS: u16 = 7_000;
/// Phase-three retained target aligned with Blueprint shadow-eval shortlist.
pub const QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS: u16 = 7_800;
/// Phase-four retained target aligned with Blueprint perfect-score push plan.
pub const QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS: u16 = 8_500;
/// Phase-five retained target aligned with Blueprint retained rehearsal plan.
pub const QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS: u16 = 9_000;
/// Phase-six retained target aligned with Blueprint expanded corpus plan.
pub const QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS: u16 = 9_500;
/// Phase-seven retained target aligned with Blueprint full-corpus matrix plan.
pub const QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS: u16 = 9_800;
/// Phase-eight retained target aligned with Blueprint residual burn-down plan.
pub const QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS: u16 = 9_900;
/// Phase-nine retained target aligned with Blueprint final-campaign rehearsal plan.
pub const QWEN_LEGAL_PHASE_NINE_TARGET_SCORE_BPS: u16 = 9_950;
/// Blueprint frontier consumed by the Psionic legal RL plan.
pub const QWEN_LEGAL_BLUEPRINT_OPTIMIZER_FRONTIER_REF: &str =
    "blueprint://harvey_legal_qwen_optimizer_frontier/optimizer_frontier_001";
/// Blueprint shadow-eval shortlist consumed by the phase-three RL window.
pub const QWEN_LEGAL_BLUEPRINT_SHADOW_EVAL_SHORTLIST_REF: &str = "blueprint://harvey_legal_qwen_phase_three_shadow_eval_shortlist/optimizer_shortlist.harvey_legal_qwen.phase_003.shadow_eval";
/// Blueprint phase-four perfect-score push plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_PERFECT_SCORE_PUSH_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_four_perfect_score_push_plan/optimizer_plan.harvey_legal_qwen.phase_004.perfect_score_push";
/// Blueprint phase-five retained rehearsal plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_RETAINED_REHEARSAL_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_five_retained_rehearsal_plan/optimizer_plan.harvey_legal_qwen.phase_005.retained_rehearsal";
/// Blueprint phase-six expanded corpus plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_EXPANDED_CORPUS_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_six_expanded_corpus_plan/optimizer_plan.harvey_legal_qwen.phase_006.expanded_corpus";
/// Blueprint phase-seven full-corpus matrix plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_FULL_CORPUS_MATRIX_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_seven_full_corpus_matrix_plan/optimizer_plan.harvey_legal_qwen.phase_007.full_corpus_matrix";
/// Blueprint phase-eight residual burn-down plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_RESIDUAL_BURN_DOWN_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_eight_residual_burn_down_plan/optimizer_plan.harvey_legal_qwen.phase_008.residual_burn_down";
/// Blueprint phase-nine final-campaign rehearsal plan consumed by Psionic.
pub const QWEN_LEGAL_BLUEPRINT_FINAL_CAMPAIGN_PLAN_REF: &str = "blueprint://harvey_legal_qwen_phase_nine_final_campaign_plan/optimizer_plan.harvey_legal_qwen.phase_009.final_campaign_rehearsal";
/// Stable target-set id for the first narrow LM-head-only adapter.
pub const QWEN_LEGAL_ADAPTER_TARGET_SET_ID: &str = "qwen3.5-4b.legal.lm_head_lora.v1";
/// Stable adapter target id for the first smoke lane.
pub const QWEN_LEGAL_ADAPTER_TARGET_ID: &str = "lm_head";
/// Stable LoRA rank for the first Qwen legal adapter smoke.
pub const QWEN_LEGAL_ADAPTER_LORA_RANK: usize = 8;
/// Stable LoRA alpha for the first Qwen legal adapter smoke.
pub const QWEN_LEGAL_ADAPTER_LORA_ALPHA: f32 = 16.0;
/// Synthetic artifact digest admitted only for the deterministic unit smoke.
pub const QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST: &str =
    "sha256:synthetic-qwen35-4b-legal-smoke";

/// Whether a base model binding is a deterministic smoke fixture or a real artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalBaseArtifactMode {
    /// Deterministic hidden-state smoke without full Qwen weights.
    SyntheticHiddenStateSmoke,
    /// Real Qwen artifact must be materialized and explicitly bound.
    RealArtifactRequired,
}

/// Explicit first target set for the Qwen legal adapter smoke lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterTargetSet {
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable adapter target identifier.
    pub adapter_target_id: String,
    /// Stable LoRA rank.
    pub lora_rank: usize,
    /// Stable LoRA alpha.
    pub lora_alpha: f32,
    /// Evolution note for widening beyond LM-head LoRA.
    pub evolution_note: String,
}

impl QwenLegalAdapterTargetSet {
    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.target_set_id != QWEN_LEGAL_ADAPTER_TARGET_SET_ID {
            return Err(QwenLegalAdapterSftError::InvalidTargetSet {
                detail: String::from("target-set id drifted from the bounded Qwen legal lane"),
            });
        }
        if self.adapter_target_id != QWEN_LEGAL_ADAPTER_TARGET_ID {
            return Err(QwenLegalAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Qwen legal smoke target surface must stay `{QWEN_LEGAL_ADAPTER_TARGET_ID}`"
                ),
            });
        }
        if self.lora_rank != QWEN_LEGAL_ADAPTER_LORA_RANK {
            return Err(QwenLegalAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Qwen legal target set must stay rank {QWEN_LEGAL_ADAPTER_LORA_RANK}"
                ),
            });
        }
        if (self.lora_alpha - QWEN_LEGAL_ADAPTER_LORA_ALPHA).abs() > f32::EPSILON {
            return Err(QwenLegalAdapterSftError::InvalidTargetSet {
                detail: format!(
                    "Qwen legal target set must stay alpha {QWEN_LEGAL_ADAPTER_LORA_ALPHA}"
                ),
            });
        }
        if self.evolution_note.trim().is_empty() {
            return Err(QwenLegalAdapterSftError::InvalidTargetSet {
                detail: String::from("target-set evolution note must be present"),
            });
        }
        Ok(())
    }

    /// Returns the stable trainable parameter count for this target set.
    #[must_use]
    pub fn parameter_count(&self, hidden_size: usize, vocab_size: usize) -> u64 {
        u64::try_from(
            self.lora_rank
                .saturating_mul(hidden_size.saturating_add(vocab_size)),
        )
        .unwrap_or(u64::MAX)
    }
}

/// Returns the canonical first Qwen legal adapter target set.
#[must_use]
pub fn canonical_qwen_legal_adapter_target_set() -> QwenLegalAdapterTargetSet {
    QwenLegalAdapterTargetSet {
        target_set_id: String::from(QWEN_LEGAL_ADAPTER_TARGET_SET_ID),
        adapter_target_id: String::from(QWEN_LEGAL_ADAPTER_TARGET_ID),
        lora_rank: QWEN_LEGAL_ADAPTER_LORA_RANK,
        lora_alpha: QWEN_LEGAL_ADAPTER_LORA_ALPHA,
        evolution_note: String::from(
            "The first smoke trains only an LM-head LoRA adapter from frozen hidden states. Attention and MLP LoRA targets are admitted only after the legal-record, checkpoint, export, and eval-import loop is green.",
        ),
    }
}

/// Compatibility binding between the trainer and one served Qwen base lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalServedBaseModelBinding {
    /// Public model id, fixed to the first smoke target.
    pub public_model_id: String,
    /// Stable served-model id published by Psionic serving metadata.
    pub served_model_id: String,
    /// Psionic model-family acceptance label.
    pub model_family_acceptance_label: String,
    /// Stable base-model revision.
    pub base_model_revision: String,
    /// Stable served-artifact digest for the frozen base.
    pub base_served_artifact_digest: String,
    /// Optional materialized artifact path for real-model execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_path: Option<String>,
    /// Whether this binding is synthetic smoke or real artifact execution.
    pub artifact_mode: QwenLegalBaseArtifactMode,
    /// Tokenizer identity the lane must preserve.
    pub tokenizer: TokenizerDigest,
    /// Prompt or chat-template digest the legal dataset was rendered against.
    pub prompt_template_digest: String,
    /// Hidden width surfaced by the served base or synthetic smoke fixture.
    pub hidden_size: usize,
    /// Maximum context window expected by the run.
    pub context_window_tokens: u32,
}

impl QwenLegalServedBaseModelBinding {
    /// Returns the served vocabulary width derived from the tokenizer.
    #[must_use]
    pub fn vocab_size(&self) -> usize {
        usize::try_from(self.tokenizer.vocab_size).unwrap_or(usize::MAX)
    }

    /// Returns `model@revision` for portable tokenizer/model binding.
    #[must_use]
    pub fn base_model_ref(&self) -> String {
        format!("{}@{}", self.public_model_id, self.base_model_revision)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.public_model_id != QWEN35_4B_LEGAL_SMOKE_MODEL_ID {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("public model id drifted from Qwen3.5-4B smoke target"),
            });
        }
        if self.served_model_id != QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("served model id drifted from qwen3.5-4b"),
            });
        }
        if self.model_family_acceptance_label != QWEN35_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("model family acceptance label must stay qwen35"),
            });
        }
        if self.base_model_revision.trim().is_empty() {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("base model revision must be present"),
            });
        }
        if self.base_served_artifact_digest.trim().is_empty() {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("base artifact digest must be present"),
            });
        }
        if self.tokenizer.tokenizer_digest.trim().is_empty() {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("tokenizer digest must be present"),
            });
        }
        if self.tokenizer.family != TokenizerFamily::BytePairEncoding {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("Qwen3.5 smoke expects a byte-pair tokenizer family"),
            });
        }
        match &self.tokenizer.template_digest {
            Some(template_digest) if template_digest == &self.prompt_template_digest => {}
            Some(_) => {
                return Err(QwenLegalAdapterSftError::Compatibility {
                    detail: String::from(
                        "served tokenizer template digest drifted from prompt template digest",
                    ),
                });
            }
            None => {
                return Err(QwenLegalAdapterSftError::Compatibility {
                    detail: String::from("tokenizer template digest must be bound"),
                });
            }
        }
        if self.hidden_size == 0 || self.context_window_tokens == 0 || self.vocab_size() == 0 {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from("served model shape and context window must be non-zero"),
            });
        }
        match self.artifact_mode {
            QwenLegalBaseArtifactMode::SyntheticHiddenStateSmoke => {
                if self.base_served_artifact_digest
                    != QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST
                {
                    return Err(QwenLegalAdapterSftError::Compatibility {
                        detail: String::from(
                            "synthetic smoke must use the explicit synthetic base artifact digest",
                        ),
                    });
                }
                if self.artifact_path.is_some() {
                    return Err(QwenLegalAdapterSftError::Compatibility {
                        detail: String::from(
                            "synthetic smoke must not pretend a real Qwen artifact path is bound",
                        ),
                    });
                }
            }
            QwenLegalBaseArtifactMode::RealArtifactRequired => {
                if self.base_served_artifact_digest
                    == QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST
                {
                    return Err(QwenLegalAdapterSftError::Compatibility {
                        detail: String::from("real artifact execution cannot use synthetic digest"),
                    });
                }
                let Some(path) = &self.artifact_path else {
                    return Err(QwenLegalAdapterSftError::Compatibility {
                        detail: String::from(
                            "real artifact execution requires an explicit artifact path",
                        ),
                    });
                };
                if path.trim().is_empty() {
                    return Err(QwenLegalAdapterSftError::Compatibility {
                        detail: String::from("real artifact path must be non-empty"),
                    });
                }
            }
        }
        Ok(())
    }

    /// Validates that a real artifact path is materialized on local disk.
    pub fn validate_real_artifact_materialized(&self) -> Result<(), QwenLegalAdapterSftError> {
        self.validate()?;
        if self.artifact_mode != QwenLegalBaseArtifactMode::RealArtifactRequired {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: String::from(
                    "materialized artifact validation only applies to real artifact execution",
                ),
            });
        }
        let path = self.artifact_path.as_deref().ok_or_else(|| {
            QwenLegalAdapterSftError::Compatibility {
                detail: String::from("real artifact execution requires an explicit artifact path"),
            }
        })?;
        if !Path::new(path).is_file() {
            return Err(QwenLegalAdapterSftError::Compatibility {
                detail: format!("real artifact path is not materialized: {path}"),
            });
        }
        Ok(())
    }
}

/// Legal benchmark dataset binding consumed by the Qwen adapter smoke lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalDatasetBinding {
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable digest for the exported legal training record bundle.
    pub dataset_digest: String,
    /// Schema version required from `LegalBenchmarkTrainingRecord`.
    pub training_record_schema_version: String,
    /// Training split reference.
    pub train_split_ref: String,
    /// Validation split reference.
    pub validation_split_ref: String,
    /// Hidden-criterion exclusion policy reference.
    pub hidden_criterion_policy_ref: String,
}

impl QwenLegalDatasetBinding {
    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        require_nonempty(self.dataset_ref.as_str(), "dataset_ref")?;
        require_nonempty(self.dataset_digest.as_str(), "dataset_digest")?;
        require_nonempty(self.train_split_ref.as_str(), "train_split_ref")?;
        require_nonempty(self.validation_split_ref.as_str(), "validation_split_ref")?;
        require_nonempty(
            self.hidden_criterion_policy_ref.as_str(),
            "hidden_criterion_policy_ref",
        )?;
        if self.training_record_schema_version != LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::DatasetDrift {
                detail: format!(
                    "expected legal training record schema `{LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION}`"
                ),
            });
        }
        Ok(())
    }
}

/// Eval-pack binding emitted with the smoke output for Autopilot4 import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalEvalPackBinding {
    /// Stable eval-pack id.
    pub eval_pack_id: String,
    /// Stable eval-pack digest.
    pub eval_pack_digest: String,
    /// Legal benchmark suite id.
    pub benchmark_suite_id: String,
    /// Retained/public smoke slice id.
    pub retained_slice_id: String,
    /// Scorer version or image digest used by the evaluator.
    pub scorer_version: String,
    /// Downstream import target for score/history dashboards.
    pub import_target: String,
}

impl QwenLegalEvalPackBinding {
    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        require_nonempty(self.eval_pack_id.as_str(), "eval_pack_id")?;
        require_nonempty(self.eval_pack_digest.as_str(), "eval_pack_digest")?;
        require_nonempty(self.benchmark_suite_id.as_str(), "benchmark_suite_id")?;
        require_nonempty(self.retained_slice_id.as_str(), "retained_slice_id")?;
        require_nonempty(self.scorer_version.as_str(), "scorer_version")?;
        require_nonempty(self.import_target.as_str(), "import_target")?;
        Ok(())
    }
}

/// One bounded Qwen legal LM-head supervision sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLmHeadSupervisionSample {
    /// Stable sample identifier.
    pub sample_id: String,
    /// Final hidden state emitted by the frozen base before LM-head projection.
    pub final_hidden_state: Vec<f32>,
    /// Target token the adapter should increase likelihood for.
    pub target_token_id: u32,
    /// Approximate source-token count preserved for telemetry.
    pub source_token_count: u32,
    /// Stable source legal training record id.
    pub legal_training_record_id: String,
}

impl QwenLegalLmHeadSupervisionSample {
    /// Creates one bounded Qwen legal LM-head supervision sample.
    #[must_use]
    pub fn new(
        sample_id: impl Into<String>,
        final_hidden_state: Vec<f32>,
        target_token_id: u32,
        source_token_count: u32,
        legal_training_record_id: impl Into<String>,
    ) -> Self {
        Self {
            sample_id: sample_id.into(),
            final_hidden_state,
            target_token_id,
            source_token_count,
            legal_training_record_id: legal_training_record_id.into(),
        }
    }

    fn into_open_adapter_sample(
        self,
    ) -> Result<OpenAdapterHiddenStateSample, OpenAdapterTrainingExecutionError> {
        if self.legal_training_record_id.trim().is_empty() {
            return Err(OpenAdapterTrainingExecutionError::MissingSampleId);
        }
        OpenAdapterHiddenStateSample::new(
            self.sample_id,
            self.final_hidden_state,
            self.target_token_id,
            self.source_token_count,
        )
    }
}

/// Execution config for the first Qwen legal adapter smoke trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterSftConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Fixed training-loop budget.
    pub budget: TrainingLoopBudget,
    /// Deterministic batch size.
    pub batch_size: usize,
    /// Optimizer config applied to the frozen target set.
    pub optimizer: TrainingOptimizerConfig,
    /// Optimizer residency policy.
    pub optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
}

impl QwenLegalAdapterSftConfig {
    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        require_nonempty(self.run_id.as_str(), "run_id")?;
        if self.batch_size == 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("batch_size must be greater than zero"),
            });
        }
        Ok(())
    }
}

/// Higher-level full-run request for the Qwen legal adapter smoke lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalAdapterSftRunRequest {
    /// Stable dataset binding from legal benchmark training records.
    pub dataset_binding: QwenLegalDatasetBinding,
    /// Eval pack bound before the run starts.
    pub eval_pack_binding: QwenLegalEvalPackBinding,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable adapter identifier.
    pub adapter_id: String,
    /// Stable adapter revision.
    pub adapter_revision: String,
    /// Logical training start timestamp.
    pub started_at_ms: u64,
    /// Logical duration assigned to each trainer step.
    pub step_duration_ms: u64,
}

impl QwenLegalAdapterSftRunRequest {
    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        self.dataset_binding.validate()?;
        self.eval_pack_binding.validate()?;
        require_nonempty(self.validator_policy_ref.as_str(), "validator_policy_ref")?;
        require_nonempty(self.adapter_id.as_str(), "adapter_id")?;
        require_nonempty(self.adapter_revision.as_str(), "adapter_revision")?;
        if self.step_duration_ms == 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("step_duration_ms must be greater than zero"),
            });
        }
        Ok(())
    }
}

/// Typed exported adapter artifact emitted by the Qwen legal adapter trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterExportedArtifact {
    /// Stable served compatibility digest.
    pub compatibility_digest: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Stable eval-pack digest.
    pub eval_pack_digest: String,
    /// Stable adapter identity.
    pub adapter_identity: AdapterArtifactIdentity,
    /// Stable adapter identity digest.
    pub adapter_identity_digest: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// LoRA alpha needed to reload the artifact.
    pub adapter_alpha: f32,
    /// Raw `safetensors` artifact bytes.
    pub adapter_bytes: Vec<u8>,
}

impl QwenLegalAdapterExportedArtifact {
    /// Reloads the exported artifact through the shared LM-head LoRA parser.
    pub fn load_lm_head_lora_artifact(
        &self,
    ) -> Result<LmHeadLoraAdapterArtifact, QwenLegalAdapterSftError> {
        Ok(LmHeadLoraAdapterArtifact::from_safetensors_bytes(
            self.adapter_bytes.as_slice(),
            self.adapter_identity.clone(),
            self.adapter_alpha,
        )?)
    }
}

/// Typed checkpoint snapshot for the Qwen legal adapter smoke lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterCheckpoint {
    /// Stable checkpoint schema version.
    pub schema_version: String,
    /// Stable checkpoint identifier.
    pub checkpoint_id: String,
    /// Stable lane id.
    pub lane_id: String,
    /// Stable served compatibility digest.
    pub compatibility_digest: String,
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable base artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Stable prompt-template digest.
    pub prompt_template_digest: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Stable eval-pack digest.
    pub eval_pack_digest: String,
    /// Logical checkpoint timestamp.
    pub saved_at_ms: u64,
    /// Exact serialized run state for continuation.
    pub run: FixedBudgetTrainingRun,
    /// Stable checkpoint digest.
    pub checkpoint_digest: String,
}

impl QwenLegalAdapterCheckpoint {
    /// Returns the stable digest over the checkpoint payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.checkpoint_digest.clear();
        stable_digest(b"psionic_qwen_legal_adapter_checkpoint|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_ADAPTER_CHECKPOINT_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint schema version drifted"),
            });
        }
        require_nonempty(self.checkpoint_id.as_str(), "checkpoint_id")?;
        if self.lane_id != QWEN_LEGAL_ADAPTER_SFT_LANE_ID {
            return Err(QwenLegalAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint lane id drifted"),
            });
        }
        if self.checkpoint_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint digest drifted"),
            });
        }
        Ok(())
    }
}

/// Summary emitted by the higher-level Qwen legal adapter smoke lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterSftSummary {
    /// Final fixed-budget run summary.
    pub run_summary: TrainingRunSummary,
    /// Stable lane id.
    pub lane_id: String,
    /// Public base model id.
    pub public_model_id: String,
    /// Served base model id.
    pub served_model_id: String,
    /// Model-family acceptance label.
    pub model_family_acceptance_label: String,
    /// Stable served compatibility digest.
    pub compatibility_digest: String,
    /// Stable target-set identifier.
    pub target_set_id: String,
    /// Stable dataset reference.
    pub dataset_ref: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Stable eval-pack id.
    pub eval_pack_id: String,
    /// Stable eval-pack digest.
    pub eval_pack_digest: String,
    /// Legal benchmark suite id.
    pub benchmark_suite_id: String,
    /// Retained/public smoke slice id.
    pub retained_slice_id: String,
    /// Stable validator policy reference.
    pub validator_policy_ref: String,
    /// Stable base artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Stable prompt-template digest.
    pub prompt_template_digest: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// Stable adapter-identity digest.
    pub adapter_identity_digest: String,
    /// Stable initial state-dict digest.
    pub initial_state_dict_digest: String,
    /// Stable final state-dict digest.
    pub final_state_dict_digest: String,
    /// Stable final checkpoint id.
    pub final_checkpoint_id: String,
}

/// Autopilot4-importable score/eval metadata emitted by the smoke lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalScoreImportBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle id.
    pub bundle_id: String,
    /// Stable lane id.
    pub lane_id: String,
    /// Legal benchmark suite id.
    pub benchmark_suite_id: String,
    /// Retained/public smoke slice id.
    pub retained_slice_id: String,
    /// Stable dataset digest.
    pub dataset_digest: String,
    /// Stable eval-pack digest.
    pub eval_pack_digest: String,
    /// Stable base artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable prompt-template digest.
    pub prompt_template_digest: String,
    /// Stable adapter-artifact digest.
    pub adapter_artifact_digest: String,
    /// Stable adapter-identity digest.
    pub adapter_identity_digest: String,
    /// Stable checkpoint reference.
    pub checkpoint_ref: String,
    /// Stable run summary digest.
    pub run_summary_digest: String,
    /// Downstream import target.
    pub import_target: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl QwenLegalScoreImportBundle {
    /// Returns the stable digest over the bundle payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_digest(b"psionic_qwen_legal_score_import_bundle|", &clone)
    }
}

/// Optimizer method used by one legal RL hillclimb target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalRlOptimizerMethod {
    /// GRPO-style policy optimization over accepted legal-agent rollouts.
    Grpo,
    /// GEPA-style trace reflection and candidate selection.
    GepaTraceSelection,
    /// MIPRO-style prompt/example search over Blueprint modules.
    MiproPromptSearch,
    /// Supervised adapter refresh from curated failure examples.
    SupervisedFineTuneRefresh,
    /// Manual legal-review correction before optimizer search.
    ManualReview,
}

/// One failure-family target for the legal RL hillclimb.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlFailureFamilyTarget {
    /// Failure family shared with Blueprint and Autopilot4 reports.
    pub failure_family: String,
    /// Current miss rate or severity in basis points.
    pub baseline_miss_basis_points: u16,
    /// Target lift expected from this target in basis points.
    pub target_lift_basis_points: u16,
    /// Blueprint module slug receiving the resulting candidate.
    pub blueprint_module_slug: String,
    /// Optimizer method assigned to the target.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Reward signal or scalarization used by rollout selection.
    pub reward_signal: String,
    /// Dataset request ref emitted for pylon/fine-tune work.
    pub dataset_request_ref: String,
}

/// Rollout policy for retained-slice legal RL collection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlRolloutPolicy {
    /// Stable rollout window id.
    pub rollout_window_id: String,
    /// Minimum accepted rollouts before a candidate can be scored.
    pub min_accepted_rollouts: u16,
    /// Maximum quarantined rollouts tolerated in the window.
    pub max_quarantined_rollouts: u16,
    /// Validator policy applied before RL or fine-tune ingestion.
    pub validator_policy_ref: String,
    /// Maximum age for off-policy rollouts.
    pub off_policy_max_age_ms: u64,
    /// Whether private Harvey evaluation slices are mandatory.
    pub retained_slice_required: bool,
    /// Reward model or reward program bundle reference.
    pub reward_program_ref: String,
    /// Sampling policy used by the rollout generator.
    pub sampling_policy_ref: String,
}

/// Typed bridge from the adapter-SFT lane to the legal RL hillclimb loop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlHillclimbPlan {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable plan id.
    pub plan_id: String,
    /// Blueprint objective id.
    pub objective_id: String,
    /// Base Psionic lane feeding adapter and dataset artifacts.
    pub base_lane_id: String,
    /// Current smoke served model id.
    pub smoke_served_model_id: String,
    /// Retained target model id.
    pub retained_target_model_id: String,
    /// Retained target served model id.
    pub retained_target_served_model_id: String,
    /// Dataset binding used by the current SFT smoke lane.
    pub dataset_binding: QwenLegalDatasetBinding,
    /// Eval pack bound before optimization starts.
    pub eval_pack_binding: QwenLegalEvalPackBinding,
    /// Retained rollout policy.
    pub rollout_policy: QwenLegalRlRolloutPolicy,
    /// Failure-family targets assigned to optimizer methods.
    pub targets: Vec<QwenLegalRlFailureFamilyTarget>,
    /// Blueprint optimizer frontier ref consumed by this plan.
    pub blueprint_optimizer_frontier_ref: String,
    /// Qwen smoke candidate ref being widened toward retained work.
    pub qwen_candidate_ref: String,
    /// Score import bundle emitted by the current smoke run.
    pub score_import_bundle_ref: String,
    /// Stable receipt id for plan materialization.
    pub receipt_id: String,
    /// Stable digest for the full plan payload.
    pub plan_digest: String,
}

impl QwenLegalRlHillclimbPlan {
    /// Returns the stable digest over the plan payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.plan_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_hillclimb_plan|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_HILLCLIMB_PLAN_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb plan schema version drifted"),
            });
        }
        if self.plan_id != QWEN_LEGAL_RL_HILLCLIMB_PLAN_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb plan id drifted"),
            });
        }
        if self.objective_id != "harvey_legal_qwen_finetune_v1" {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb objective id drifted"),
            });
        }
        if self.base_lane_id != QWEN_LEGAL_ADAPTER_SFT_LANE_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb base lane id drifted"),
            });
        }
        if self.smoke_served_model_id != QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb smoke served model drifted"),
            });
        }
        if self.retained_target_model_id != QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID
            || self.retained_target_served_model_id != QWEN36_35B_A3B_LEGAL_RETAINED_SERVED_MODEL_ID
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb retained target must be Qwen3.6-35B-A3B"),
            });
        }
        self.dataset_binding.validate()?;
        self.eval_pack_binding.validate()?;
        require_nonempty(
            self.rollout_policy.validator_policy_ref.as_str(),
            "rollout_policy.validator_policy_ref",
        )?;
        require_nonempty(
            self.rollout_policy.reward_program_ref.as_str(),
            "rollout_policy.reward_program_ref",
        )?;
        require_nonempty(
            self.rollout_policy.sampling_policy_ref.as_str(),
            "rollout_policy.sampling_policy_ref",
        )?;
        if !self.rollout_policy.retained_slice_required {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb requires private Harvey slices"),
            });
        }
        if self.rollout_policy.min_accepted_rollouts <= self.rollout_policy.max_quarantined_rollouts
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted legal rollouts must outnumber quarantined rollouts"),
            });
        }
        if self.targets.is_empty() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb targets must be present"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(target.reward_signal.as_str(), "target.reward_signal")?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            if target.baseline_miss_basis_points == 0 || target.target_lift_basis_points == 0 {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from("legal RL targets require non-zero miss and lift values"),
                });
            }
        }
        require_nonempty(
            self.blueprint_optimizer_frontier_ref.as_str(),
            "blueprint_optimizer_frontier_ref",
        )?;
        require_nonempty(self.qwen_candidate_ref.as_str(), "qwen_candidate_ref")?;
        require_nonempty(
            self.score_import_bundle_ref.as_str(),
            "score_import_bundle_ref",
        )?;
        require_nonempty(self.receipt_id.as_str(), "receipt_id")?;
        if self.plan_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL hillclimb plan digest drifted"),
            });
        }
        Ok(())
    }
}

/// Builds the canonical next-phase legal RL hillclimb plan.
pub fn canonical_qwen_legal_rl_hillclimb_plan(
    dataset_binding: QwenLegalDatasetBinding,
    eval_pack_binding: QwenLegalEvalPackBinding,
    score_import_bundle_ref: impl Into<String>,
) -> Result<QwenLegalRlHillclimbPlan, QwenLegalAdapterSftError> {
    dataset_binding.validate()?;
    eval_pack_binding.validate()?;
    let mut plan = QwenLegalRlHillclimbPlan {
        schema_version: String::from(QWEN_LEGAL_RL_HILLCLIMB_PLAN_SCHEMA_VERSION),
        plan_id: String::from(QWEN_LEGAL_RL_HILLCLIMB_PLAN_ID),
        objective_id: String::from("harvey_legal_qwen_finetune_v1"),
        base_lane_id: String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID),
        smoke_served_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID),
        retained_target_model_id: String::from(QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID),
        retained_target_served_model_id: String::from(
            QWEN36_35B_A3B_LEGAL_RETAINED_SERVED_MODEL_ID,
        ),
        dataset_binding,
        eval_pack_binding,
        rollout_policy: QwenLegalRlRolloutPolicy {
            rollout_window_id: String::from("harvey-legal-retained-20-task-window-001"),
            min_accepted_rollouts: 60,
            max_quarantined_rollouts: 12,
            validator_policy_ref: String::from("policy://validator/legal-benchmark/qwen-rl-v1"),
            off_policy_max_age_ms: 86_400_000,
            retained_slice_required: true,
            reward_program_ref: String::from(
                "blueprint://program.harvey_legal.final_self_check/reward_bundle_v1",
            ),
            sampling_policy_ref: String::from(
                "policy://psionic/legal-benchmark/retained-diverse-rollouts@v1",
            ),
        },
        targets: vec![
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("document_coverage"),
                baseline_miss_basis_points: 4200,
                target_lift_basis_points: 900,
                blueprint_module_slug: String::from("harvey_legal.document_inventory"),
                optimizer_method: QwenLegalRlOptimizerMethod::MiproPromptSearch,
                reward_signal: String::from("document_manifest_recall_and_source_span_precision"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.document_coverage.retained_001",
                ),
            },
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("citation_evidence"),
                baseline_miss_basis_points: 3900,
                target_lift_basis_points: 860,
                blueprint_module_slug: String::from("harvey_legal.evidence_mapping"),
                optimizer_method: QwenLegalRlOptimizerMethod::GepaTraceSelection,
                reward_signal: String::from("citation_grounding_judge_agreement"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.citation_evidence.retained_001",
                ),
            },
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("legal_reasoning"),
                baseline_miss_basis_points: 3800,
                target_lift_basis_points: 820,
                blueprint_module_slug: String::from("harvey_legal.issue_fact_extraction"),
                optimizer_method: QwenLegalRlOptimizerMethod::Grpo,
                reward_signal: String::from("issue_rule_fact_conclusion_rubric_delta"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.legal_reasoning.retained_001",
                ),
            },
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("spreadsheet_reasoning"),
                baseline_miss_basis_points: 3300,
                target_lift_basis_points: 760,
                blueprint_module_slug: String::from("harvey_legal.evidence_mapping"),
                optimizer_method: QwenLegalRlOptimizerMethod::Grpo,
                reward_signal: String::from("spreadsheet_cell_lineage_and_arithmetic_exactness"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.spreadsheet_reasoning.retained_001",
                ),
            },
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("missing_fact"),
                baseline_miss_basis_points: 3100,
                target_lift_basis_points: 720,
                blueprint_module_slug: String::from("harvey_legal.issue_fact_extraction"),
                optimizer_method: QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh,
                reward_signal: String::from("missed_fact_recall_with_source_span_support"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.missing_fact.retained_001",
                ),
            },
            QwenLegalRlFailureFamilyTarget {
                failure_family: String::from("pre_submit_self_check"),
                baseline_miss_basis_points: 2800,
                target_lift_basis_points: 560,
                blueprint_module_slug: String::from("harvey_legal.final_self_check"),
                optimizer_method: QwenLegalRlOptimizerMethod::GepaTraceSelection,
                reward_signal: String::from("self_check_rewrite_acceptance_before_submission"),
                dataset_request_ref: String::from(
                    "dataset_request.harvey_legal.pre_submit_self_check.retained_001",
                ),
            },
        ],
        blueprint_optimizer_frontier_ref: String::from(QWEN_LEGAL_BLUEPRINT_OPTIMIZER_FRONTIER_REF),
        qwen_candidate_ref: String::from("model_candidate.harvey_legal_qwen.qwen3_5_4b.smoke_001"),
        score_import_bundle_ref: score_import_bundle_ref.into(),
        receipt_id: String::from("receipt.psionic.qwen_legal_rl_hillclimb_plan_001"),
        plan_digest: String::new(),
    };
    plan.plan_digest = plan.stable_digest();
    plan.validate()?;
    Ok(plan)
}

/// Offline benchmark projection and readiness report for the legal RL plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlBenchmarkReadinessReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Plan id this report evaluates.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the projection.
    pub baseline_score_basis_points: u16,
    /// Conservative target score for the next retained run.
    pub conservative_target_score_basis_points: u16,
    /// Unconstrained score if all target-family lifts land.
    pub unconstrained_projected_score_basis_points: u16,
    /// Sum of target lifts in the plan.
    pub total_target_lift_basis_points: u16,
    /// Retained model target.
    pub retained_target_model_id: String,
    /// Whether retained slices are required before a score claim.
    pub retained_slice_required: bool,
    /// Minimum accepted rollout count.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count.
    pub quarantined_rollout_budget: u16,
    /// Count of GRPO targets.
    pub grpo_target_count: usize,
    /// Count of GEPA trace-selection targets.
    pub gepa_target_count: usize,
    /// Count of MIPRO prompt-search targets.
    pub mipro_target_count: usize,
    /// Count of supervised fine-tune refresh targets.
    pub supervised_refresh_target_count: usize,
    /// Dataset request count emitted by target families.
    pub dataset_request_count: usize,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside a phase-three RL optimization window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlOptimizationWindowTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the resulting candidate.
    pub blueprint_module_slug: String,
    /// Dataset request admitted into the window.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in shadow eval.
    pub planned_shadow_lift_basis_points: u16,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
}

/// Phase-three offline RL optimization window report for the Harvey lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlOptimizationWindowReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source readiness report id.
    pub source_report_id: String,
    /// Plan id this window executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Stable digest of the readiness report payload.
    pub source_report_digest: String,
    /// Baseline retained score used for the window.
    pub baseline_score_basis_points: u16,
    /// Previous conservative target carried forward from phase two.
    pub previous_target_score_basis_points: u16,
    /// New phase-three conservative target.
    pub phase_three_target_score_basis_points: u16,
    /// Sum of family-level planned shadow lifts.
    pub total_planned_shadow_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase three.
    pub rollout_window_id: String,
    /// Minimum accepted rollout count for the phase-three window.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-three window.
    pub quarantined_rollout_budget: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Blueprint shadow-eval shortlist ref consumed by this report.
    pub blueprint_shadow_eval_shortlist_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlOptimizationWindowTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-four perfect-score push report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlPerfectScorePushTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the resulting candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the window.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in shadow eval.
    pub planned_shadow_lift_basis_points: u16,
    /// Whether this family requires explicit judge-adjudication traces.
    pub judge_adjudication_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
}

/// Phase-four offline RL/adjudication report for the Harvey perfect-score push.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlPerfectScorePushReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-three window report id.
    pub source_window_report_id: String,
    /// Stable digest of the phase-three report payload.
    pub source_window_report_digest: String,
    /// Plan id this push still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the push.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase three.
    pub previous_target_score_basis_points: u16,
    /// New phase-four conservative target.
    pub phase_four_target_score_basis_points: u16,
    /// Sum of family-level planned shadow lifts.
    pub total_planned_shadow_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase four.
    pub rollout_window_id: String,
    /// Minimum accepted rollout count for the phase-four push.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-four push.
    pub quarantined_rollout_budget: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint perfect-score push plan ref consumed by this report.
    pub blueprint_perfect_score_push_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlPerfectScorePushTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-five retained rehearsal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlRetainedRehearsalTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the rehearsed candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the rehearsal.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in retained rehearsal.
    pub planned_rehearsal_lift_basis_points: u16,
    /// Number of retained-slice replay passes required for this family.
    pub retained_replay_count: u8,
    /// Whether a multi-judge panel must adjudicate the family.
    pub judge_panel_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
    /// Per-family receipt that must exist before promotion review.
    pub receipt_ref: String,
}

/// Phase-five offline RL/adjudication report for private Harvey rehearsal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlRetainedRehearsalReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-four report id.
    pub source_push_report_id: String,
    /// Stable digest of the phase-four report payload.
    pub source_push_report_digest: String,
    /// Plan id this rehearsal still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the rehearsal.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase four.
    pub previous_target_score_basis_points: u16,
    /// New phase-five conservative target.
    pub phase_five_target_score_basis_points: u16,
    /// Sum of family-level planned retained-rehearsal lifts.
    pub total_planned_rehearsal_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase five.
    pub rollout_window_id: String,
    /// Number of retained task-runs required before score import.
    pub retained_rehearsal_run_count: u16,
    /// Minimum accepted rollout count for the phase-five rehearsal.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-five rehearsal.
    pub quarantined_rollout_budget: u16,
    /// Minimum adversarial holdout task-runs for the rehearsal.
    pub adversarial_holdout_run_count: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint retained-rehearsal plan ref consumed by this report.
    pub blueprint_retained_rehearsal_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlRetainedRehearsalTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-six expanded corpus report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlExpandedCorpusTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the expanded-corpus candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the expanded corpus dry run.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in expanded corpus scoring.
    pub planned_expanded_lift_basis_points: u16,
    /// Number of expanded-corpus tasks this family must touch.
    pub expanded_slice_task_count: u16,
    /// Whether every audited practice area must be represented.
    pub practice_area_balance_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
    /// Per-family receipt that must exist before promotion review.
    pub receipt_ref: String,
}

/// Phase-six offline RL/adjudication report for expanded Harvey corpus scoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlExpandedCorpusReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-five report id.
    pub source_rehearsal_report_id: String,
    /// Stable digest of the phase-five report payload.
    pub source_rehearsal_report_digest: String,
    /// Plan id this expanded-corpus dry run still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the expanded corpus dry run.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase five.
    pub previous_target_score_basis_points: u16,
    /// New phase-six conservative target.
    pub phase_six_target_score_basis_points: u16,
    /// Sum of family-level planned expanded-corpus lifts.
    pub total_planned_expanded_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase six.
    pub rollout_window_id: String,
    /// Expanded stratified task slice size.
    pub expanded_slice_task_count: u16,
    /// Audited Harvey practice areas that must be represented.
    pub practice_area_count: u8,
    /// Minimum accepted rollout count for the phase-six dry run.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-six dry run.
    pub quarantined_rollout_budget: u16,
    /// Minimum adversarial holdout task-runs for the dry run.
    pub adversarial_holdout_run_count: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint expanded corpus plan ref consumed by this report.
    pub blueprint_expanded_corpus_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlExpandedCorpusTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-seven full-corpus matrix report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlFullCorpusMatrixTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the full-corpus candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the full-corpus matrix.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in full-corpus scoring.
    pub planned_full_corpus_lift_basis_points: u16,
    /// Full Harvey corpus task count this family must be evaluated against.
    pub full_corpus_task_count: u16,
    /// Number of Qwen/Blueprint/RL matrix cells this family must cover.
    pub model_matrix_cell_count: u16,
    /// Whether every audited practice area must remain represented.
    pub practice_area_balance_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
    /// Per-family receipt that must exist before promotion review.
    pub receipt_ref: String,
}

/// Phase-seven offline RL/adjudication report for full Harvey corpus scoring.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlFullCorpusMatrixReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-six report id.
    pub source_expanded_report_id: String,
    /// Stable digest of the phase-six report payload.
    pub source_expanded_report_digest: String,
    /// Plan id this full-corpus matrix dry run still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the full-corpus matrix dry run.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase six.
    pub previous_target_score_basis_points: u16,
    /// New phase-seven conservative target.
    pub phase_seven_target_score_basis_points: u16,
    /// Sum of family-level planned full-corpus lifts.
    pub total_planned_full_corpus_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase seven.
    pub rollout_window_id: String,
    /// Full Harvey task corpus size.
    pub full_corpus_task_count: u16,
    /// Audited Harvey practice areas that must be represented.
    pub practice_area_count: u8,
    /// Number of Qwen/Blueprint/RL matrix cells required before import.
    pub model_matrix_cell_count: u16,
    /// Minimum accepted rollout count for the phase-seven dry run.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-seven dry run.
    pub quarantined_rollout_budget: u16,
    /// Minimum adversarial holdout task-runs for the dry run.
    pub adversarial_holdout_run_count: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint full-corpus matrix plan ref consumed by this report.
    pub blueprint_full_corpus_matrix_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlFullCorpusMatrixTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-eight residual burn-down report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlResidualBurnDownTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the residual burn-down candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the residual burn-down matrix.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in residual burn-down scoring.
    pub planned_residual_lift_basis_points: u16,
    /// Full Harvey corpus task count this family must be evaluated against.
    pub full_corpus_task_count: u16,
    /// Number of Qwen/Blueprint/RL matrix cells this family must cover.
    pub model_matrix_cell_count: u16,
    /// Remaining residual miss clusters permitted for this family.
    pub residual_cluster_budget: u16,
    /// Whether every audited practice area must remain represented.
    pub practice_area_balance_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
    /// Per-family receipt that must exist before promotion review.
    pub receipt_ref: String,
}

/// Phase-eight offline RL/adjudication report for near-perfect residual burn-down.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlResidualBurnDownReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-seven report id.
    pub source_full_corpus_report_id: String,
    /// Stable digest of the phase-seven report payload.
    pub source_full_corpus_report_digest: String,
    /// Plan id this residual burn-down dry run still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the residual burn-down dry run.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase seven.
    pub previous_target_score_basis_points: u16,
    /// New phase-eight conservative target.
    pub phase_eight_target_score_basis_points: u16,
    /// Sum of family-level planned residual burn-down lifts.
    pub total_planned_residual_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase eight.
    pub rollout_window_id: String,
    /// Full Harvey task corpus size.
    pub full_corpus_task_count: u16,
    /// Audited Harvey practice areas that must be represented.
    pub practice_area_count: u8,
    /// Number of Qwen/Blueprint/RL matrix cells required before import.
    pub model_matrix_cell_count: u16,
    /// Maximum residual miss clusters permitted before score import.
    pub residual_cluster_budget: u16,
    /// Minimum accepted rollout count for the phase-eight dry run.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-eight dry run.
    pub quarantined_rollout_budget: u16,
    /// Minimum adversarial holdout task-runs for the dry run.
    pub adversarial_holdout_run_count: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint residual burn-down plan ref consumed by this report.
    pub blueprint_residual_burn_down_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlResidualBurnDownTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// One failure-family allocation inside the phase-nine final-campaign report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlFinalCampaignTarget {
    /// Failure family shared with Blueprint and Autopilot4.
    pub failure_family: String,
    /// Optimizer method assigned to this family.
    pub optimizer_method: QwenLegalRlOptimizerMethod,
    /// Blueprint module receiving the final-campaign candidate.
    pub blueprint_module_slug: String,
    /// Dataset or review request admitted into the final campaign matrix.
    pub dataset_request_ref: String,
    /// Planned accepted rollouts or reviewed traces for this family.
    pub planned_accepted_rollouts: u16,
    /// Conservative lift this target should support in final-campaign scoring.
    pub planned_final_lift_basis_points: u16,
    /// Full Harvey corpus task count this family must be evaluated against.
    pub full_corpus_task_count: u16,
    /// Number of Qwen/Blueprint/RL matrix cells this family must cover.
    pub model_matrix_cell_count: u16,
    /// Remaining residual miss clusters permitted for this family.
    pub residual_cluster_budget: u16,
    /// Human-adjudicated sample task-runs required for this family.
    pub final_adjudication_sample_count: u16,
    /// Whether every audited practice area must remain represented.
    pub practice_area_balance_required: bool,
    /// Per-family scorecard that must exist before promotion review.
    pub scorecard_ref: String,
    /// Per-family receipt that must exist before promotion review.
    pub receipt_ref: String,
}

/// Phase-nine offline RL/adjudication report for final retained-campaign rehearsal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalRlFinalCampaignReport {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Source phase-eight report id.
    pub source_residual_burn_down_report_id: String,
    /// Stable digest of the phase-eight report payload.
    pub source_residual_burn_down_report_digest: String,
    /// Plan id this final-campaign rehearsal still executes.
    pub plan_id: String,
    /// Stable digest of the plan payload.
    pub plan_digest: String,
    /// Baseline retained score used for the final-campaign rehearsal.
    pub baseline_score_basis_points: u16,
    /// Previous target carried forward from phase eight.
    pub previous_target_score_basis_points: u16,
    /// New phase-nine conservative target.
    pub phase_nine_target_score_basis_points: u16,
    /// Sum of family-level planned final-campaign lifts.
    pub total_planned_final_lift_basis_points: u16,
    /// Target model family for retained scoring.
    pub retained_target_model_id: String,
    /// Rollout window id widened for phase nine.
    pub rollout_window_id: String,
    /// Full Harvey task corpus size.
    pub full_corpus_task_count: u16,
    /// Audited Harvey practice areas that must be represented.
    pub practice_area_count: u8,
    /// Number of Qwen/Blueprint/RL matrix cells required before import.
    pub model_matrix_cell_count: u16,
    /// Maximum residual miss clusters permitted before score import.
    pub residual_cluster_budget: u16,
    /// Minimum human-adjudicated sample task-runs for final rehearsal.
    pub final_adjudication_sample_count: u16,
    /// Minimum accepted rollout count for the phase-nine dry run.
    pub accepted_rollout_minimum: u16,
    /// Maximum quarantined rollout count for the phase-nine dry run.
    pub quarantined_rollout_budget: u16,
    /// Minimum adversarial holdout task-runs for the dry run.
    pub adversarial_holdout_run_count: u16,
    /// Maximum permitted holdout regression for promotion review.
    pub holdout_max_regression_basis_points: u16,
    /// Maximum unresolved judge-disagreement budget.
    pub calibrated_judge_disagreement_budget_basis_points: u16,
    /// Blueprint final-campaign plan ref consumed by this report.
    pub blueprint_final_campaign_plan_ref: String,
    /// Per-family target allocations.
    pub targets: Vec<QwenLegalRlFinalCampaignTarget>,
    /// Autopilot4 export/update target.
    pub benchmark_export_ref: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl QwenLegalRlBenchmarkReadinessReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_benchmark_report|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_BENCHMARK_REPORT_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_BENCHMARK_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report id drifted"),
            });
        }
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.baseline_score_basis_points == 0
            || self.baseline_score_basis_points >= self.conservative_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report requires a target above baseline"),
            });
        }
        if self.conservative_target_score_basis_points
            > self.unconstrained_projected_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("conservative target cannot exceed unconstrained projection"),
            });
        }
        if !self.retained_slice_required {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("retained slice is required for legal RL benchmark report"),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted rollout minimum must exceed quarantine budget"),
            });
        }
        if self.grpo_target_count == 0
            || self.gepa_target_count == 0
            || self.mipro_target_count == 0
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report requires GRPO, GEPA, and MIPRO"),
            });
        }
        if self.dataset_request_count == 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report requires dataset requests"),
            });
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL benchmark report digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlOptimizationWindowReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_optimization_window|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL optimization window schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL optimization window report id drifted"),
            });
        }
        require_nonempty(self.source_report_id.as_str(), "source_report_id")?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        require_nonempty(self.source_report_digest.as_str(), "source_report_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
            || self.phase_three_target_score_basis_points != QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL optimization window target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_three_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL optimization window requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_shadow_lift_basis_points)
            < self.phase_three_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned shadow lift does not support phase-three target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted legal rollouts must exceed quarantine budget"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-three Harvey window allows no holdout regression"),
            });
        }
        require_nonempty(
            self.blueprint_shadow_eval_shortlist_ref.as_str(),
            "blueprint_shadow_eval_shortlist_ref",
        )?;
        if self.targets.len() < 6 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from(
                    "legal RL optimization window requires all phase-two families",
                ),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            if target.planned_accepted_rollouts == 0 || target.planned_shadow_lift_basis_points == 0
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from("legal RL optimization targets require rollout and lift"),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL optimization window digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlPerfectScorePushReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_perfect_score_push|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL perfect-score push schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL perfect-score push report id drifted"),
            });
        }
        require_nonempty(
            self.source_window_report_id.as_str(),
            "source_window_report_id",
        )?;
        require_nonempty(
            self.source_window_report_digest.as_str(),
            "source_window_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
            || self.phase_four_target_score_basis_points != QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL perfect-score push target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_four_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL perfect-score push requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_shadow_lift_basis_points)
            < self.phase_four_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned perfect-score push lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted legal rollouts must exceed quarantine budget"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-four Harvey push allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 75 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_perfect_score_push_plan_ref.as_str(),
            "blueprint_perfect_score_push_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("perfect-score push requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            if target.planned_accepted_rollouts == 0 || target.planned_shadow_lift_basis_points == 0
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from("perfect-score push targets require rollout and lift"),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL perfect-score push digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlRetainedRehearsalReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_retained_rehearsal|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_RETAINED_REHEARSAL_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL retained rehearsal schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_RETAINED_REHEARSAL_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL retained rehearsal report id drifted"),
            });
        }
        require_nonempty(self.source_push_report_id.as_str(), "source_push_report_id")?;
        require_nonempty(
            self.source_push_report_digest.as_str(),
            "source_push_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
            || self.phase_five_target_score_basis_points != QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL retained rehearsal target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_five_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL retained rehearsal requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_rehearsal_lift_basis_points)
            < self.phase_five_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned retained rehearsal lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.retained_rehearsal_run_count < 60 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("retained rehearsal requires at least 60 task-runs"),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted legal rollouts must exceed quarantine budget"),
            });
        }
        if self.adversarial_holdout_run_count < 36 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("retained rehearsal requires adversarial holdout task-runs"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-five Harvey rehearsal allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 50 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("retained rehearsal judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_retained_rehearsal_plan_ref.as_str(),
            "blueprint_retained_rehearsal_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("retained rehearsal requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            require_nonempty(target.receipt_ref.as_str(), "target.receipt_ref")?;
            if target.planned_accepted_rollouts == 0
                || target.planned_rehearsal_lift_basis_points == 0
                || target.retained_replay_count < 3
                || !target.judge_panel_required
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from(
                        "retained rehearsal targets require rollouts, lift, replay, and judge panel",
                    ),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL retained rehearsal digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlExpandedCorpusReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_expanded_corpus|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_EXPANDED_CORPUS_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL expanded corpus schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_EXPANDED_CORPUS_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL expanded corpus report id drifted"),
            });
        }
        require_nonempty(
            self.source_rehearsal_report_id.as_str(),
            "source_rehearsal_report_id",
        )?;
        require_nonempty(
            self.source_rehearsal_report_digest.as_str(),
            "source_rehearsal_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
            || self.phase_six_target_score_basis_points != QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL expanded corpus target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_six_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL expanded corpus requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_expanded_lift_basis_points)
            < self.phase_six_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned expanded corpus lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.expanded_slice_task_count < 125 || self.practice_area_count < 24 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("expanded corpus requires 125 tasks and all practice areas"),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted legal rollouts must exceed quarantine budget"),
            });
        }
        if self.adversarial_holdout_run_count < 72 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("expanded corpus requires adversarial holdout task-runs"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-six Harvey dry run allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 35 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("expanded corpus judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_expanded_corpus_plan_ref.as_str(),
            "blueprint_expanded_corpus_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("expanded corpus requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            require_nonempty(target.receipt_ref.as_str(), "target.receipt_ref")?;
            if target.planned_accepted_rollouts == 0
                || target.planned_expanded_lift_basis_points == 0
                || target.expanded_slice_task_count < 125
                || !target.practice_area_balance_required
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from(
                        "expanded corpus targets require rollouts, lift, slice, and practice balance",
                    ),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL expanded corpus digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlFullCorpusMatrixReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_full_corpus_matrix|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL full-corpus matrix schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL full-corpus matrix report id drifted"),
            });
        }
        require_nonempty(
            self.source_expanded_report_id.as_str(),
            "source_expanded_report_id",
        )?;
        require_nonempty(
            self.source_expanded_report_digest.as_str(),
            "source_expanded_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
            || self.phase_seven_target_score_basis_points != QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL full-corpus matrix target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_seven_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL full-corpus matrix requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_full_corpus_lift_basis_points)
            < self.phase_seven_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned full-corpus lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.full_corpus_task_count < 1251
            || self.practice_area_count < 24
            || self.model_matrix_cell_count < 48
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from(
                    "full-corpus matrix requires corpus, practice areas, and matrix coverage",
                ),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted full-corpus rollouts must exceed quarantine budget"),
            });
        }
        if self.adversarial_holdout_run_count < 144 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("full-corpus matrix requires adversarial holdout task-runs"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-seven Harvey dry run allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 25 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("full-corpus matrix judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_full_corpus_matrix_plan_ref.as_str(),
            "blueprint_full_corpus_matrix_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("full-corpus matrix requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            require_nonempty(target.receipt_ref.as_str(), "target.receipt_ref")?;
            if target.planned_accepted_rollouts == 0
                || target.planned_full_corpus_lift_basis_points == 0
                || target.full_corpus_task_count < 1251
                || target.model_matrix_cell_count < 48
                || !target.practice_area_balance_required
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from(
                        "full-corpus matrix targets require rollouts, lift, corpus, matrix, and practice balance",
                    ),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL full-corpus matrix digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlResidualBurnDownReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_residual_burn_down|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL residual burn-down schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL residual burn-down report id drifted"),
            });
        }
        require_nonempty(
            self.source_full_corpus_report_id.as_str(),
            "source_full_corpus_report_id",
        )?;
        require_nonempty(
            self.source_full_corpus_report_digest.as_str(),
            "source_full_corpus_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
            || self.phase_eight_target_score_basis_points != QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL residual burn-down target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_eight_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL residual burn-down requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_residual_lift_basis_points)
            < self.phase_eight_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned residual burn-down lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.full_corpus_task_count < 1251
            || self.practice_area_count < 24
            || self.model_matrix_cell_count < 96
            || self.residual_cluster_budget > 24
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from(
                    "residual burn-down requires corpus, practice areas, matrix coverage, and residual budget",
                ),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("accepted residual rollouts must exceed quarantine budget"),
            });
        }
        if self.adversarial_holdout_run_count < 288 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("residual burn-down requires adversarial holdout task-runs"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-eight Harvey dry run allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 15 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("residual burn-down judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_residual_burn_down_plan_ref.as_str(),
            "blueprint_residual_burn_down_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("residual burn-down requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            require_nonempty(target.receipt_ref.as_str(), "target.receipt_ref")?;
            if target.planned_accepted_rollouts == 0
                || target.planned_residual_lift_basis_points == 0
                || target.full_corpus_task_count < 1251
                || target.model_matrix_cell_count < 96
                || target.residual_cluster_budget > 24
                || !target.practice_area_balance_required
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from(
                        "residual burn-down targets require rollouts, lift, corpus, matrix, residual budget, and practice balance",
                    ),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL residual burn-down digest drifted"),
            });
        }
        Ok(())
    }
}

impl QwenLegalRlFinalCampaignReport {
    /// Returns the stable digest over the report payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_qwen_legal_rl_final_campaign|", &clone)
    }

    fn validate(&self) -> Result<(), QwenLegalAdapterSftError> {
        if self.schema_version != QWEN_LEGAL_RL_FINAL_CAMPAIGN_SCHEMA_VERSION {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL final-campaign schema version drifted"),
            });
        }
        if self.report_id != QWEN_LEGAL_RL_FINAL_CAMPAIGN_REPORT_ID {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL final-campaign report id drifted"),
            });
        }
        require_nonempty(
            self.source_residual_burn_down_report_id.as_str(),
            "source_residual_burn_down_report_id",
        )?;
        require_nonempty(
            self.source_residual_burn_down_report_digest.as_str(),
            "source_residual_burn_down_report_digest",
        )?;
        require_nonempty(self.plan_id.as_str(), "plan_id")?;
        require_nonempty(self.plan_digest.as_str(), "plan_digest")?;
        if self.previous_target_score_basis_points != QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
            || self.phase_nine_target_score_basis_points != QWEN_LEGAL_PHASE_NINE_TARGET_SCORE_BPS
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL final-campaign target drifted"),
            });
        }
        if self.baseline_score_basis_points >= self.previous_target_score_basis_points
            || self.previous_target_score_basis_points >= self.phase_nine_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL final-campaign requires monotonic targets"),
            });
        }
        if self
            .baseline_score_basis_points
            .saturating_add(self.total_planned_final_lift_basis_points)
            < self.phase_nine_target_score_basis_points
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("planned final-campaign lift does not support target"),
            });
        }
        require_nonempty(
            self.retained_target_model_id.as_str(),
            "retained_target_model_id",
        )?;
        require_nonempty(self.rollout_window_id.as_str(), "rollout_window_id")?;
        if self.full_corpus_task_count < 1251
            || self.practice_area_count < 24
            || self.model_matrix_cell_count < 144
            || self.residual_cluster_budget > 12
            || self.final_adjudication_sample_count < 96
        {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from(
                    "final campaign requires corpus, practice areas, matrix coverage, residual budget, and adjudication sample",
                ),
            });
        }
        if self.accepted_rollout_minimum <= self.quarantined_rollout_budget {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from(
                    "accepted final-campaign rollouts must exceed quarantine budget",
                ),
            });
        }
        if self.adversarial_holdout_run_count < 432 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("final campaign requires adversarial holdout task-runs"),
            });
        }
        if self.holdout_max_regression_basis_points != 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("phase-nine Harvey dry run allows no holdout regression"),
            });
        }
        if self.calibrated_judge_disagreement_budget_basis_points > 10 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("final-campaign judge disagreement budget is too loose"),
            });
        }
        require_nonempty(
            self.blueprint_final_campaign_plan_ref.as_str(),
            "blueprint_final_campaign_plan_ref",
        )?;
        if self.targets.len() < 9 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("final campaign requires all frontier families"),
            });
        }
        for target in &self.targets {
            require_nonempty(target.failure_family.as_str(), "target.failure_family")?;
            require_nonempty(
                target.blueprint_module_slug.as_str(),
                "target.blueprint_module_slug",
            )?;
            require_nonempty(
                target.dataset_request_ref.as_str(),
                "target.dataset_request_ref",
            )?;
            require_nonempty(target.scorecard_ref.as_str(), "target.scorecard_ref")?;
            require_nonempty(target.receipt_ref.as_str(), "target.receipt_ref")?;
            if target.planned_accepted_rollouts == 0
                || target.planned_final_lift_basis_points == 0
                || target.full_corpus_task_count < 1251
                || target.model_matrix_cell_count < 144
                || target.residual_cluster_budget > 12
                || target.final_adjudication_sample_count < 96
                || !target.practice_area_balance_required
            {
                return Err(QwenLegalAdapterSftError::InvalidConfig {
                    detail: String::from(
                        "final-campaign targets require rollouts, lift, corpus, matrix, residual budget, adjudication, and practice balance",
                    ),
                });
            }
        }
        require_nonempty(self.benchmark_export_ref.as_str(), "benchmark_export_ref")?;
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("legal RL final-campaign digest drifted"),
            });
        }
        Ok(())
    }
}

/// Builds the offline benchmark projection for a legal RL plan.
pub fn qwen_legal_rl_benchmark_readiness_report(
    plan: &QwenLegalRlHillclimbPlan,
    baseline_score_basis_points: u16,
) -> Result<QwenLegalRlBenchmarkReadinessReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    if baseline_score_basis_points == 0 || baseline_score_basis_points > 10_000 {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("baseline score must be in basis points"),
        });
    }
    let total_target_lift_basis_points = plan
        .targets
        .iter()
        .map(|target| target.target_lift_basis_points)
        .fold(0_u16, u16::saturating_add);
    let unconstrained_projected_score_basis_points = baseline_score_basis_points
        .saturating_add(total_target_lift_basis_points)
        .min(10_000);
    let conservative_target_score_basis_points = QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
        .min(unconstrained_projected_score_basis_points)
        .max(baseline_score_basis_points.saturating_add(100));
    let count_method = |method| {
        plan.targets
            .iter()
            .filter(|target| target.optimizer_method == method)
            .count()
    };
    let mut report = QwenLegalRlBenchmarkReadinessReport {
        schema_version: String::from(QWEN_LEGAL_RL_BENCHMARK_REPORT_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_BENCHMARK_REPORT_ID),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points,
        conservative_target_score_basis_points,
        unconstrained_projected_score_basis_points,
        total_target_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        retained_slice_required: plan.rollout_policy.retained_slice_required,
        accepted_rollout_minimum: plan.rollout_policy.min_accepted_rollouts,
        quarantined_rollout_budget: plan.rollout_policy.max_quarantined_rollouts,
        grpo_target_count: count_method(QwenLegalRlOptimizerMethod::Grpo),
        gepa_target_count: count_method(QwenLegalRlOptimizerMethod::GepaTraceSelection),
        mipro_target_count: count_method(QwenLegalRlOptimizerMethod::MiproPromptSearch),
        supervised_refresh_target_count: count_method(
            QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh,
        ),
        dataset_request_count: plan.targets.len(),
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-002"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-three optimization window report from the RL plan and phase-two report.
pub fn qwen_legal_rl_phase_three_optimization_window_report(
    plan: &QwenLegalRlHillclimbPlan,
    readiness_report: &QwenLegalRlBenchmarkReadinessReport,
) -> Result<QwenLegalRlOptimizationWindowReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    readiness_report.validate()?;
    if readiness_report.plan_digest != plan.plan_digest
        || readiness_report.plan_id != plan.plan_id
        || readiness_report.conservative_target_score_basis_points
            != QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-three optimization window source report mismatch"),
        });
    }

    let targets = plan
        .targets
        .iter()
        .map(|target| {
            let planned_accepted_rollouts = match target.optimizer_method {
                QwenLegalRlOptimizerMethod::Grpo => 18,
                QwenLegalRlOptimizerMethod::GepaTraceSelection => 12,
                QwenLegalRlOptimizerMethod::MiproPromptSearch => 14,
                QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh => 10,
                QwenLegalRlOptimizerMethod::ManualReview => 6,
            };
            let planned_shadow_lift_basis_points = (target.target_lift_basis_points * 65) / 100;

            QwenLegalRlOptimizationWindowTarget {
                failure_family: target.failure_family.clone(),
                optimizer_method: target.optimizer_method,
                blueprint_module_slug: target.blueprint_module_slug.clone(),
                dataset_request_ref: target.dataset_request_ref.clone(),
                planned_accepted_rollouts,
                planned_shadow_lift_basis_points,
                scorecard_ref: format!(
                    "scorecard.psionic.qwen_legal.{}.phase_003.shadow_eval",
                    target.failure_family
                ),
            }
        })
        .collect::<Vec<_>>();
    let total_planned_shadow_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_shadow_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlOptimizationWindowReport {
        schema_version: String::from(QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_REPORT_ID),
        source_report_id: readiness_report.report_id.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        source_report_digest: readiness_report.report_digest.clone(),
        baseline_score_basis_points: readiness_report.baseline_score_basis_points,
        previous_target_score_basis_points: readiness_report.conservative_target_score_basis_points,
        phase_three_target_score_basis_points: QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS,
        total_planned_shadow_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-retained-20-task-window-003"),
        accepted_rollout_minimum,
        quarantined_rollout_budget: readiness_report.quarantined_rollout_budget + 4,
        holdout_max_regression_basis_points: 0,
        blueprint_shadow_eval_shortlist_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_SHADOW_EVAL_SHORTLIST_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-003"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-four perfect-score push report from the phase-three RL window.
pub fn qwen_legal_rl_phase_four_perfect_score_push_report(
    plan: &QwenLegalRlHillclimbPlan,
    window_report: &QwenLegalRlOptimizationWindowReport,
) -> Result<QwenLegalRlPerfectScorePushReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    window_report.validate()?;
    if window_report.plan_digest != plan.plan_digest
        || window_report.plan_id != plan.plan_id
        || window_report.phase_three_target_score_basis_points
            != QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-four perfect-score push source report mismatch"),
        });
    }

    let mut targets = window_report
        .targets
        .iter()
        .map(|target| QwenLegalRlPerfectScorePushTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 4,
            planned_shadow_lift_basis_points: target.planned_shadow_lift_basis_points,
            judge_adjudication_required: true,
            scorecard_ref: target
                .scorecard_ref
                .replace("phase_003.shadow_eval", "phase_004.perfect_push"),
        })
        .collect::<Vec<_>>();

    targets.extend([
        QwenLegalRlPerfectScorePushTarget {
            failure_family: String::from("deliverable_completeness"),
            optimizer_method: QwenLegalRlOptimizerMethod::MiproPromptSearch,
            blueprint_module_slug: String::from("harvey_legal.deliverable_outlining"),
            dataset_request_ref: String::from(
                "dataset_request.harvey_legal.deliverable_completeness.retained_001",
            ),
            planned_accepted_rollouts: 12,
            planned_shadow_lift_basis_points: 384,
            judge_adjudication_required: true,
            scorecard_ref: String::from(
                "scorecard.psionic.qwen_legal.deliverable_completeness.phase_004.perfect_push",
            ),
        },
        QwenLegalRlPerfectScorePushTarget {
            failure_family: String::from("fine_tune_data_selection"),
            optimizer_method: QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh,
            blueprint_module_slug: String::from("harvey_legal.candidate_improvement_planning"),
            dataset_request_ref: String::from(
                "dataset_request.harvey_legal.fine_tune_data_selection.retained_001",
            ),
            planned_accepted_rollouts: 12,
            planned_shadow_lift_basis_points: 312,
            judge_adjudication_required: true,
            scorecard_ref: String::from(
                "scorecard.psionic.qwen_legal.fine_tune_data_selection.phase_004.perfect_push",
            ),
        },
        QwenLegalRlPerfectScorePushTarget {
            failure_family: String::from("task_intake_routing"),
            optimizer_method: QwenLegalRlOptimizerMethod::ManualReview,
            blueprint_module_slug: String::from("harvey_legal.task_intake"),
            dataset_request_ref: String::from(
                "dataset_request.harvey_legal.task_intake_routing.retained_001",
            ),
            planned_accepted_rollouts: 8,
            planned_shadow_lift_basis_points: 192,
            judge_adjudication_required: true,
            scorecard_ref: String::from(
                "scorecard.psionic.qwen_legal.task_intake_routing.phase_004.perfect_push",
            ),
        },
    ]);

    let total_planned_shadow_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_shadow_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlPerfectScorePushReport {
        schema_version: String::from(QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_REPORT_ID),
        source_window_report_id: window_report.report_id.clone(),
        source_window_report_digest: window_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: window_report.baseline_score_basis_points,
        previous_target_score_basis_points: window_report.phase_three_target_score_basis_points,
        phase_four_target_score_basis_points: QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS,
        total_planned_shadow_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-retained-20-task-window-004"),
        accepted_rollout_minimum,
        quarantined_rollout_budget: window_report.quarantined_rollout_budget + 4,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 75,
        blueprint_perfect_score_push_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_PERFECT_SCORE_PUSH_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-004"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-five retained rehearsal report from the phase-four RL push.
pub fn qwen_legal_rl_phase_five_retained_rehearsal_report(
    plan: &QwenLegalRlHillclimbPlan,
    push_report: &QwenLegalRlPerfectScorePushReport,
) -> Result<QwenLegalRlRetainedRehearsalReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    push_report.validate()?;
    if push_report.plan_digest != plan.plan_digest
        || push_report.plan_id != plan.plan_id
        || push_report.phase_four_target_score_basis_points
            != QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-five retained rehearsal source report mismatch"),
        });
    }

    let targets = push_report
        .targets
        .iter()
        .map(|target| QwenLegalRlRetainedRehearsalTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 6,
            planned_rehearsal_lift_basis_points: target.planned_shadow_lift_basis_points,
            retained_replay_count: 3,
            judge_panel_required: true,
            scorecard_ref: target
                .scorecard_ref
                .replace("phase_004.perfect_push", "phase_005.retained_rehearsal"),
            receipt_ref: format!(
                "receipt.psionic.qwen_legal.{}.phase_005.retained_rehearsal",
                target.failure_family
            ),
        })
        .collect::<Vec<_>>();
    let total_planned_rehearsal_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_rehearsal_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlRetainedRehearsalReport {
        schema_version: String::from(QWEN_LEGAL_RL_RETAINED_REHEARSAL_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_RETAINED_REHEARSAL_REPORT_ID),
        source_push_report_id: push_report.report_id.clone(),
        source_push_report_digest: push_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: push_report.baseline_score_basis_points,
        previous_target_score_basis_points: push_report.phase_four_target_score_basis_points,
        phase_five_target_score_basis_points: QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS,
        total_planned_rehearsal_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-retained-20-task-window-005"),
        retained_rehearsal_run_count: 60,
        accepted_rollout_minimum,
        quarantined_rollout_budget: push_report.quarantined_rollout_budget + 4,
        adversarial_holdout_run_count: 36,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 50,
        blueprint_retained_rehearsal_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_RETAINED_REHEARSAL_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-005"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-six expanded corpus dry-run report from the phase-five rehearsal.
pub fn qwen_legal_rl_phase_six_expanded_corpus_report(
    plan: &QwenLegalRlHillclimbPlan,
    rehearsal_report: &QwenLegalRlRetainedRehearsalReport,
) -> Result<QwenLegalRlExpandedCorpusReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    rehearsal_report.validate()?;
    if rehearsal_report.plan_digest != plan.plan_digest
        || rehearsal_report.plan_id != plan.plan_id
        || rehearsal_report.phase_five_target_score_basis_points
            != QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-six expanded corpus source report mismatch"),
        });
    }

    let targets = rehearsal_report
        .targets
        .iter()
        .map(|target| QwenLegalRlExpandedCorpusTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 8,
            planned_expanded_lift_basis_points: ((target.planned_rehearsal_lift_basis_points
                as u32
                * 135)
                / 100) as u16,
            expanded_slice_task_count: 125,
            practice_area_balance_required: true,
            scorecard_ref: target
                .scorecard_ref
                .replace("phase_005.retained_rehearsal", "phase_006.expanded_corpus"),
            receipt_ref: format!(
                "receipt.psionic.qwen_legal.{}.phase_006.expanded_corpus",
                target.failure_family
            ),
        })
        .collect::<Vec<_>>();
    let total_planned_expanded_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_expanded_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlExpandedCorpusReport {
        schema_version: String::from(QWEN_LEGAL_RL_EXPANDED_CORPUS_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_EXPANDED_CORPUS_REPORT_ID),
        source_rehearsal_report_id: rehearsal_report.report_id.clone(),
        source_rehearsal_report_digest: rehearsal_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: rehearsal_report.baseline_score_basis_points,
        previous_target_score_basis_points: rehearsal_report.phase_five_target_score_basis_points,
        phase_six_target_score_basis_points: QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS,
        total_planned_expanded_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-expanded-125-task-window-006"),
        expanded_slice_task_count: 125,
        practice_area_count: 24,
        accepted_rollout_minimum,
        quarantined_rollout_budget: rehearsal_report.quarantined_rollout_budget + 6,
        adversarial_holdout_run_count: 72,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 35,
        blueprint_expanded_corpus_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_EXPANDED_CORPUS_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-006"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-seven full-corpus matrix dry-run report from the expanded corpus report.
pub fn qwen_legal_rl_phase_seven_full_corpus_matrix_report(
    plan: &QwenLegalRlHillclimbPlan,
    expanded_report: &QwenLegalRlExpandedCorpusReport,
) -> Result<QwenLegalRlFullCorpusMatrixReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    expanded_report.validate()?;
    if expanded_report.plan_digest != plan.plan_digest
        || expanded_report.plan_id != plan.plan_id
        || expanded_report.phase_six_target_score_basis_points
            != QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-seven full-corpus matrix source report mismatch"),
        });
    }

    let targets = expanded_report
        .targets
        .iter()
        .map(|target| QwenLegalRlFullCorpusMatrixTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 16,
            planned_full_corpus_lift_basis_points: ((target.planned_expanded_lift_basis_points
                as u32
                * 108)
                / 100) as u16,
            full_corpus_task_count: 1251,
            model_matrix_cell_count: 48,
            practice_area_balance_required: true,
            scorecard_ref: target
                .scorecard_ref
                .replace("phase_006.expanded_corpus", "phase_007.full_corpus_matrix"),
            receipt_ref: format!(
                "receipt.psionic.qwen_legal.{}.phase_007.full_corpus_matrix",
                target.failure_family
            ),
        })
        .collect::<Vec<_>>();
    let total_planned_full_corpus_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_full_corpus_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlFullCorpusMatrixReport {
        schema_version: String::from(QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_REPORT_ID),
        source_expanded_report_id: expanded_report.report_id.clone(),
        source_expanded_report_digest: expanded_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: expanded_report.baseline_score_basis_points,
        previous_target_score_basis_points: expanded_report.phase_six_target_score_basis_points,
        phase_seven_target_score_basis_points: QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS,
        total_planned_full_corpus_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-full-corpus-matrix-window-007"),
        full_corpus_task_count: 1251,
        practice_area_count: 24,
        model_matrix_cell_count: 48,
        accepted_rollout_minimum,
        quarantined_rollout_budget: expanded_report.quarantined_rollout_budget + 8,
        adversarial_holdout_run_count: 144,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 25,
        blueprint_full_corpus_matrix_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_FULL_CORPUS_MATRIX_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-007"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Builds the phase-eight residual burn-down dry-run report from the full-corpus matrix.
pub fn qwen_legal_rl_phase_eight_residual_burn_down_report(
    plan: &QwenLegalRlHillclimbPlan,
    full_corpus_report: &QwenLegalRlFullCorpusMatrixReport,
) -> Result<QwenLegalRlResidualBurnDownReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    full_corpus_report.validate()?;
    if full_corpus_report.plan_digest != plan.plan_digest
        || full_corpus_report.plan_id != plan.plan_id
        || full_corpus_report.phase_seven_target_score_basis_points
            != QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-eight residual burn-down source report mismatch"),
        });
    }

    let targets = full_corpus_report
        .targets
        .iter()
        .map(|target| QwenLegalRlResidualBurnDownTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 24,
            planned_residual_lift_basis_points: ((target.planned_full_corpus_lift_basis_points
                as u32
                * 115)
                / 100) as u16,
            full_corpus_task_count: 1251,
            model_matrix_cell_count: 96,
            residual_cluster_budget: 24,
            practice_area_balance_required: true,
            scorecard_ref: target.scorecard_ref.replace(
                "phase_007.full_corpus_matrix",
                "phase_008.residual_burn_down",
            ),
            receipt_ref: format!(
                "receipt.psionic.qwen_legal.{}.phase_008.residual_burn_down",
                target.failure_family
            ),
        })
        .collect::<Vec<_>>();
    let total_planned_residual_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_residual_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlResidualBurnDownReport {
        schema_version: String::from(QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_REPORT_ID),
        source_full_corpus_report_id: full_corpus_report.report_id.clone(),
        source_full_corpus_report_digest: full_corpus_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: full_corpus_report.baseline_score_basis_points,
        previous_target_score_basis_points: full_corpus_report
            .phase_seven_target_score_basis_points,
        phase_eight_target_score_basis_points: QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS,
        total_planned_residual_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-residual-burn-down-window-008"),
        full_corpus_task_count: 1251,
        practice_area_count: 24,
        model_matrix_cell_count: 96,
        residual_cluster_budget: 24,
        accepted_rollout_minimum,
        quarantined_rollout_budget: full_corpus_report.quarantined_rollout_budget + 10,
        adversarial_holdout_run_count: 288,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 15,
        blueprint_residual_burn_down_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_RESIDUAL_BURN_DOWN_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-008"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn qwen_legal_rl_phase_nine_final_campaign_report(
    plan: &QwenLegalRlHillclimbPlan,
    residual_report: &QwenLegalRlResidualBurnDownReport,
) -> Result<QwenLegalRlFinalCampaignReport, QwenLegalAdapterSftError> {
    plan.validate()?;
    residual_report.validate()?;
    if residual_report.plan_digest != plan.plan_digest
        || residual_report.plan_id != plan.plan_id
        || residual_report.phase_eight_target_score_basis_points
            != QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
    {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: String::from("phase-nine final-campaign source report mismatch"),
        });
    }

    let targets = residual_report
        .targets
        .iter()
        .map(|target| QwenLegalRlFinalCampaignTarget {
            failure_family: target.failure_family.clone(),
            optimizer_method: target.optimizer_method,
            blueprint_module_slug: target.blueprint_module_slug.clone(),
            dataset_request_ref: target.dataset_request_ref.clone(),
            planned_accepted_rollouts: target.planned_accepted_rollouts + 36,
            planned_final_lift_basis_points: ((target.planned_residual_lift_basis_points as u32
                * 108)
                / 100) as u16,
            full_corpus_task_count: 1251,
            model_matrix_cell_count: 144,
            residual_cluster_budget: 12,
            final_adjudication_sample_count: 96,
            practice_area_balance_required: true,
            scorecard_ref: target.scorecard_ref.replace(
                "phase_008.residual_burn_down",
                "phase_009.final_campaign_rehearsal",
            ),
            receipt_ref: format!(
                "receipt.psionic.qwen_legal.{}.phase_009.final_campaign_rehearsal",
                target.failure_family
            ),
        })
        .collect::<Vec<_>>();
    let total_planned_final_lift_basis_points = targets
        .iter()
        .map(|target| target.planned_final_lift_basis_points)
        .sum::<u16>();
    let accepted_rollout_minimum = targets
        .iter()
        .map(|target| target.planned_accepted_rollouts)
        .sum::<u16>();

    let mut report = QwenLegalRlFinalCampaignReport {
        schema_version: String::from(QWEN_LEGAL_RL_FINAL_CAMPAIGN_SCHEMA_VERSION),
        report_id: String::from(QWEN_LEGAL_RL_FINAL_CAMPAIGN_REPORT_ID),
        source_residual_burn_down_report_id: residual_report.report_id.clone(),
        source_residual_burn_down_report_digest: residual_report.report_digest.clone(),
        plan_id: plan.plan_id.clone(),
        plan_digest: plan.plan_digest.clone(),
        baseline_score_basis_points: residual_report.baseline_score_basis_points,
        previous_target_score_basis_points: residual_report.phase_eight_target_score_basis_points,
        phase_nine_target_score_basis_points: QWEN_LEGAL_PHASE_NINE_TARGET_SCORE_BPS,
        total_planned_final_lift_basis_points,
        retained_target_model_id: plan.retained_target_model_id.clone(),
        rollout_window_id: String::from("harvey-legal-final-campaign-window-009"),
        full_corpus_task_count: 1251,
        practice_area_count: 24,
        model_matrix_cell_count: 144,
        residual_cluster_budget: 12,
        final_adjudication_sample_count: 96,
        accepted_rollout_minimum,
        quarantined_rollout_budget: residual_report.quarantined_rollout_budget + 4,
        adversarial_holdout_run_count: 432,
        holdout_max_regression_basis_points: 0,
        calibrated_judge_disagreement_budget_basis_points: 10,
        blueprint_final_campaign_plan_ref: String::from(
            QWEN_LEGAL_BLUEPRINT_FINAL_CAMPAIGN_PLAN_REF,
        ),
        targets,
        benchmark_export_ref: String::from("autopilot4://benchmarks/harvey/progress/phase-009"),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

/// Full higher-level Qwen legal adapter smoke outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct QwenLegalAdapterSftRunOutcome {
    /// Step receipts emitted during the run.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted during the run.
    pub gradient_records: Vec<OpenAdapterGradientBatchRecord>,
    /// Summary and reproducibility metadata.
    pub summary: QwenLegalAdapterSftSummary,
    /// Initial adapter-only bundle receipt.
    pub initial_bundle_receipt: ModelIoArtifactReceipt,
    /// Final adapter-only bundle receipt.
    pub final_bundle_receipt: ModelIoArtifactReceipt,
    /// Typed adapter delta between the initial and final bundles.
    pub adapter_delta: ModelAdapterDelta,
    /// Typed exported artifact.
    pub exported_artifact: QwenLegalAdapterExportedArtifact,
    /// Final checkpoint snapshot.
    pub final_checkpoint: QwenLegalAdapterCheckpoint,
    /// Score/eval metadata for downstream import.
    pub score_import_bundle: QwenLegalScoreImportBundle,
    /// Next-phase RL hillclimb plan bound to this smoke run.
    pub rl_hillclimb_plan: QwenLegalRlHillclimbPlan,
    /// Offline benchmark projection for the RL hillclimb plan.
    pub rl_benchmark_report: QwenLegalRlBenchmarkReadinessReport,
    /// Phase-three optimization window report for the RL hillclimb plan.
    pub rl_optimization_window_report: QwenLegalRlOptimizationWindowReport,
    /// Phase-four perfect-score push report for the RL hillclimb plan.
    pub rl_perfect_score_push_report: QwenLegalRlPerfectScorePushReport,
    /// Phase-five retained rehearsal report for the RL hillclimb plan.
    pub rl_retained_rehearsal_report: QwenLegalRlRetainedRehearsalReport,
    /// Phase-six expanded corpus report for the RL hillclimb plan.
    pub rl_expanded_corpus_report: QwenLegalRlExpandedCorpusReport,
    /// Phase-seven full-corpus matrix report for the RL hillclimb plan.
    pub rl_full_corpus_matrix_report: QwenLegalRlFullCorpusMatrixReport,
    /// Phase-eight residual burn-down report for the RL hillclimb plan.
    pub rl_residual_burn_down_report: QwenLegalRlResidualBurnDownReport,
    /// Phase-nine final-campaign rehearsal report for the RL hillclimb plan.
    pub rl_final_campaign_report: QwenLegalRlFinalCampaignReport,
}

/// First honest Qwen legal adapter-SFT smoke trainer.
#[derive(Clone, Debug)]
pub struct QwenLegalAdapterSftTrainer {
    target_set: QwenLegalAdapterTargetSet,
    base_binding: QwenLegalServedBaseModelBinding,
    compatibility_digest: String,
    backend: OpenAdapterTrainingExecutionBackend,
}

impl QwenLegalAdapterSftTrainer {
    /// Builds the first bounded Qwen legal adapter trainer.
    pub fn new(
        config: QwenLegalAdapterSftConfig,
        target_set: QwenLegalAdapterTargetSet,
        base_binding: QwenLegalServedBaseModelBinding,
        samples: Vec<QwenLegalLmHeadSupervisionSample>,
    ) -> Result<Self, QwenLegalAdapterSftError> {
        config.validate()?;
        target_set.validate()?;
        base_binding.validate()?;
        let compatibility_digest = stable_compatibility_digest(&target_set, &base_binding);
        let open_samples = samples
            .into_iter()
            .map(QwenLegalLmHeadSupervisionSample::into_open_adapter_sample)
            .collect::<Result<Vec<_>, _>>()?;
        let backend = OpenAdapterTrainingExecutionBackend::new(
            crate::OpenAdapterExecutionConfig {
                run_id: config.run_id,
                checkpoint_family: String::from(QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY),
                execution_backend_label: String::from(OPEN_ADAPTER_QWEN35_LEGAL_CUDA_BACKEND_LABEL),
                admissible_model_family:
                    OpenAdapterAdmissibleModelFamily::Qwen35LegalDecoderLmHeadLora,
                budget: config.budget,
                batch_size: config.batch_size,
                precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
                model: OpenAdapterReferenceModel {
                    base_model_id: base_binding.public_model_id.clone(),
                    base_model_revision: base_binding.base_model_revision.clone(),
                    base_served_artifact_digest: base_binding.base_served_artifact_digest.clone(),
                    tokenizer: base_binding.tokenizer.clone(),
                    hidden_size: base_binding.hidden_size,
                    vocab_size: base_binding.vocab_size(),
                    target: OpenAdapterLmHeadTarget {
                        target_id: target_set.adapter_target_id.clone(),
                        lora_rank: target_set.lora_rank,
                        lora_alpha: target_set.lora_alpha,
                        optimizer: config.optimizer,
                        optimizer_residency_policy: config.optimizer_residency_policy,
                    },
                },
            },
            open_samples,
        )?;
        Ok(Self {
            target_set,
            base_binding,
            compatibility_digest,
            backend,
        })
    }

    /// Returns the explicit target set.
    #[must_use]
    pub fn target_set(&self) -> &QwenLegalAdapterTargetSet {
        &self.target_set
    }

    /// Returns the served-base binding.
    #[must_use]
    pub fn base_binding(&self) -> &QwenLegalServedBaseModelBinding {
        &self.base_binding
    }

    /// Returns the stable served-compatibility digest.
    #[must_use]
    pub fn compatibility_digest(&self) -> &str {
        self.compatibility_digest.as_str()
    }

    /// Returns the underlying reusable open-adapter backend.
    #[must_use]
    pub fn backend(&self) -> &OpenAdapterTrainingExecutionBackend {
        &self.backend
    }

    /// Creates a fresh training run.
    pub fn initialize_run(&self) -> Result<FixedBudgetTrainingRun, QwenLegalAdapterSftError> {
        Ok(self.backend.initialize_run()?)
    }

    /// Advances one run for up to `step_limit` additional steps.
    pub fn advance_run(
        &self,
        run: &mut FixedBudgetTrainingRun,
        step_limit: Option<u64>,
        started_at_ms: u64,
        step_duration_ms: u64,
    ) -> Result<QwenLegalAdapterRunProgress, QwenLegalAdapterSftError> {
        if step_duration_ms == 0 {
            return Err(QwenLegalAdapterSftError::InvalidConfig {
                detail: String::from("step_duration_ms must be greater than zero"),
            });
        }
        let remaining_steps = run
            .summary()
            .budget
            .max_steps
            .saturating_sub(run.summary().completed_steps);
        let allowed_steps = step_limit.unwrap_or(remaining_steps).min(remaining_steps);
        let mut step_receipts = Vec::new();
        let mut gradient_records = Vec::new();
        for step_offset in 0..allowed_steps {
            let batch_index = run.completed_steps() as usize % self.backend.batches().len().max(1);
            let step_started_at_ms = started_at_ms + step_offset.saturating_mul(step_duration_ms);
            let step_finished_at_ms = step_started_at_ms + step_duration_ms;
            let (step_input, gradient_record) = self.backend.produce_step_input(
                run,
                batch_index,
                step_started_at_ms,
                step_finished_at_ms,
            )?;
            gradient_records.push(gradient_record);
            step_receipts.push(run.apply_step(step_input)?);
        }
        Ok(QwenLegalAdapterRunProgress {
            step_receipts,
            gradient_records,
        })
    }

    /// Saves one exact run-state checkpoint for later continuation.
    pub fn save_checkpoint(
        &self,
        checkpoint_id: impl Into<String>,
        run: &FixedBudgetTrainingRun,
        dataset_binding: &QwenLegalDatasetBinding,
        eval_pack_binding: &QwenLegalEvalPackBinding,
        saved_at_ms: u64,
    ) -> Result<QwenLegalAdapterCheckpoint, QwenLegalAdapterSftError> {
        let checkpoint_id = checkpoint_id.into();
        dataset_binding.validate()?;
        eval_pack_binding.validate()?;
        self.backend.snapshot_training_groups(run)?;
        let mut checkpoint = QwenLegalAdapterCheckpoint {
            schema_version: String::from(QWEN_LEGAL_ADAPTER_CHECKPOINT_SCHEMA_VERSION),
            checkpoint_id,
            lane_id: String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID),
            compatibility_digest: self.compatibility_digest.clone(),
            target_set_id: self.target_set.target_set_id.clone(),
            base_served_artifact_digest: self.base_binding.base_served_artifact_digest.clone(),
            tokenizer_contract_digest: self.base_binding.tokenizer.stable_digest(),
            prompt_template_digest: self.base_binding.prompt_template_digest.clone(),
            dataset_digest: dataset_binding.dataset_digest.clone(),
            eval_pack_digest: eval_pack_binding.eval_pack_digest.clone(),
            saved_at_ms,
            run: run.clone(),
            checkpoint_digest: String::new(),
        };
        checkpoint.checkpoint_digest = checkpoint.stable_digest();
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Restores one previously saved exact run-state checkpoint.
    pub fn restore_run(
        &self,
        checkpoint: &QwenLegalAdapterCheckpoint,
    ) -> Result<FixedBudgetTrainingRun, QwenLegalAdapterSftError> {
        checkpoint.validate()?;
        if checkpoint.compatibility_digest != self.compatibility_digest
            || checkpoint.target_set_id != self.target_set.target_set_id
            || checkpoint.base_served_artifact_digest
                != self.base_binding.base_served_artifact_digest
            || checkpoint.tokenizer_contract_digest != self.base_binding.tokenizer.stable_digest()
            || checkpoint.prompt_template_digest != self.base_binding.prompt_template_digest
        {
            return Err(QwenLegalAdapterSftError::InvalidCheckpoint {
                detail: String::from("checkpoint compatibility drifted from active Qwen lane"),
            });
        }
        self.backend.snapshot_training_groups(&checkpoint.run)?;
        Ok(checkpoint.run.clone())
    }

    /// Runs the full cold-start Qwen legal adapter smoke lane.
    pub fn run_sft(
        &self,
        request: &QwenLegalAdapterSftRunRequest,
    ) -> Result<QwenLegalAdapterSftRunOutcome, QwenLegalAdapterSftError> {
        let run = self.initialize_run()?;
        self.run_from_existing_run(run, request, None)
    }

    /// Restores one checkpoint and continues until the fixed budget is reached.
    pub fn run_sft_from_checkpoint(
        &self,
        checkpoint: &QwenLegalAdapterCheckpoint,
        request: &QwenLegalAdapterSftRunRequest,
    ) -> Result<QwenLegalAdapterSftRunOutcome, QwenLegalAdapterSftError> {
        let run = self.restore_run(checkpoint)?;
        self.run_from_existing_run(run, request, Some(checkpoint.checkpoint_id.as_str()))
    }

    fn run_from_existing_run(
        &self,
        mut run: FixedBudgetTrainingRun,
        request: &QwenLegalAdapterSftRunRequest,
        resumed_from_checkpoint_id: Option<&str>,
    ) -> Result<QwenLegalAdapterSftRunOutcome, QwenLegalAdapterSftError> {
        request.validate()?;
        let initial_groups = self.backend.snapshot_training_groups(&run)?;
        let initial_bundle = self.bundle_from_groups(
            initial_groups.as_slice(),
            format!("checkpoint://{}/initial", self.backend.config().run_id),
        )?;
        let (_, initial_bundle_receipt) = initial_bundle.export_safetensors()?;
        let progress = self.advance_run(
            &mut run,
            None,
            request.started_at_ms,
            request.step_duration_ms,
        )?;
        let final_groups = self.backend.snapshot_training_groups(&run)?;
        let final_bundle = self.bundle_from_groups(
            final_groups.as_slice(),
            format!("checkpoint://{}/final", self.backend.config().run_id),
        )?;
        let (_, final_bundle_receipt) = final_bundle.export_safetensors()?;
        let adapter_delta = crate::PortableModelStateDict::derive_adapter_delta(
            &initial_bundle.state_dict,
            &final_bundle.state_dict,
            request.adapter_id.clone(),
        )?;
        let exported = self.backend.export_run_artifact(
            &run,
            &OpenAdapterArtifactExportRequest::new(
                request.dataset_binding.dataset_ref.clone(),
                request.validator_policy_ref.clone(),
                request.adapter_id.clone(),
                request.adapter_revision.clone(),
            ),
        )?;
        let exported_artifact = QwenLegalAdapterExportedArtifact {
            compatibility_digest: self.compatibility_digest.clone(),
            dataset_digest: request.dataset_binding.dataset_digest.clone(),
            eval_pack_digest: request.eval_pack_binding.eval_pack_digest.clone(),
            adapter_identity: exported.adapter_identity,
            adapter_identity_digest: exported.adapter_identity_digest,
            adapter_artifact_digest: exported.adapter_artifact_digest,
            adapter_alpha: self.target_set.lora_alpha,
            adapter_bytes: exported.adapter_bytes,
        };
        exported_artifact.load_lm_head_lora_artifact()?;
        let checkpoint_id = checkpoint_id_for(
            request.adapter_id.as_str(),
            request.adapter_revision.as_str(),
            resumed_from_checkpoint_id,
        );
        let final_checkpoint = self.save_checkpoint(
            checkpoint_id,
            &run,
            &request.dataset_binding,
            &request.eval_pack_binding,
            progress
                .step_receipts
                .last()
                .map(|receipt| receipt.timing.finished_at_ms)
                .unwrap_or(request.started_at_ms),
        )?;
        let run_summary = run.summary();
        let run_summary_digest = stable_digest(b"psionic_qwen_legal_run_summary|", &run_summary);
        let mut score_import_bundle = QwenLegalScoreImportBundle {
            schema_version: String::from(QWEN_LEGAL_SCORE_IMPORT_BUNDLE_SCHEMA_VERSION),
            bundle_id: format!(
                "{}-{}-score-import",
                request.adapter_id, request.adapter_revision
            ),
            lane_id: String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID),
            benchmark_suite_id: request.eval_pack_binding.benchmark_suite_id.clone(),
            retained_slice_id: request.eval_pack_binding.retained_slice_id.clone(),
            dataset_digest: request.dataset_binding.dataset_digest.clone(),
            eval_pack_digest: request.eval_pack_binding.eval_pack_digest.clone(),
            base_served_artifact_digest: self.base_binding.base_served_artifact_digest.clone(),
            prompt_template_digest: self.base_binding.prompt_template_digest.clone(),
            adapter_artifact_digest: exported_artifact.adapter_artifact_digest.clone(),
            adapter_identity_digest: exported_artifact.adapter_identity_digest.clone(),
            checkpoint_ref: format!("checkpoint://{}", final_checkpoint.checkpoint_id),
            run_summary_digest,
            import_target: request.eval_pack_binding.import_target.clone(),
            bundle_digest: String::new(),
        };
        score_import_bundle.bundle_digest = score_import_bundle.stable_digest();
        let rl_hillclimb_plan = canonical_qwen_legal_rl_hillclimb_plan(
            request.dataset_binding.clone(),
            request.eval_pack_binding.clone(),
            score_import_bundle.bundle_id.clone(),
        )?;
        let rl_benchmark_report =
            qwen_legal_rl_benchmark_readiness_report(&rl_hillclimb_plan, 5_260)?;
        let rl_optimization_window_report = qwen_legal_rl_phase_three_optimization_window_report(
            &rl_hillclimb_plan,
            &rl_benchmark_report,
        )?;
        let rl_perfect_score_push_report = qwen_legal_rl_phase_four_perfect_score_push_report(
            &rl_hillclimb_plan,
            &rl_optimization_window_report,
        )?;
        let rl_retained_rehearsal_report = qwen_legal_rl_phase_five_retained_rehearsal_report(
            &rl_hillclimb_plan,
            &rl_perfect_score_push_report,
        )?;
        let rl_expanded_corpus_report = qwen_legal_rl_phase_six_expanded_corpus_report(
            &rl_hillclimb_plan,
            &rl_retained_rehearsal_report,
        )?;
        let rl_full_corpus_matrix_report = qwen_legal_rl_phase_seven_full_corpus_matrix_report(
            &rl_hillclimb_plan,
            &rl_expanded_corpus_report,
        )?;
        let rl_residual_burn_down_report = qwen_legal_rl_phase_eight_residual_burn_down_report(
            &rl_hillclimb_plan,
            &rl_full_corpus_matrix_report,
        )?;
        let rl_final_campaign_report = qwen_legal_rl_phase_nine_final_campaign_report(
            &rl_hillclimb_plan,
            &rl_residual_burn_down_report,
        )?;
        let summary = QwenLegalAdapterSftSummary {
            run_summary,
            lane_id: String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID),
            public_model_id: self.base_binding.public_model_id.clone(),
            served_model_id: self.base_binding.served_model_id.clone(),
            model_family_acceptance_label: self.base_binding.model_family_acceptance_label.clone(),
            compatibility_digest: self.compatibility_digest.clone(),
            target_set_id: self.target_set.target_set_id.clone(),
            dataset_ref: request.dataset_binding.dataset_ref.clone(),
            dataset_digest: request.dataset_binding.dataset_digest.clone(),
            eval_pack_id: request.eval_pack_binding.eval_pack_id.clone(),
            eval_pack_digest: request.eval_pack_binding.eval_pack_digest.clone(),
            benchmark_suite_id: request.eval_pack_binding.benchmark_suite_id.clone(),
            retained_slice_id: request.eval_pack_binding.retained_slice_id.clone(),
            validator_policy_ref: request.validator_policy_ref.clone(),
            base_served_artifact_digest: self.base_binding.base_served_artifact_digest.clone(),
            tokenizer_contract_digest: self.base_binding.tokenizer.stable_digest(),
            prompt_template_digest: self.base_binding.prompt_template_digest.clone(),
            adapter_artifact_digest: exported_artifact.adapter_artifact_digest.clone(),
            adapter_identity_digest: exported_artifact.adapter_identity_digest.clone(),
            initial_state_dict_digest: initial_bundle_receipt.state_dict_digest.clone(),
            final_state_dict_digest: final_bundle_receipt.state_dict_digest.clone(),
            final_checkpoint_id: final_checkpoint.checkpoint_id.clone(),
        };
        Ok(QwenLegalAdapterSftRunOutcome {
            step_receipts: progress.step_receipts,
            gradient_records: progress.gradient_records,
            summary,
            initial_bundle_receipt,
            final_bundle_receipt,
            adapter_delta,
            exported_artifact,
            final_checkpoint,
            score_import_bundle,
            rl_hillclimb_plan,
            rl_benchmark_report,
            rl_optimization_window_report,
            rl_perfect_score_push_report,
            rl_retained_rehearsal_report,
            rl_expanded_corpus_report,
            rl_full_corpus_matrix_report,
            rl_residual_burn_down_report,
            rl_final_campaign_report,
        })
    }

    fn bundle_from_groups(
        &self,
        groups: &[crate::TrainingParameterGroupState],
        checkpoint_ref: String,
    ) -> Result<PortableModelBundle, QwenLegalAdapterSftError> {
        Ok(PortableModelBundle::from_training_groups(
            OPEN_ADAPTER_QWEN35_LEGAL_ADAPTER_FAMILY,
            self.base_binding.base_model_revision.clone(),
            QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY,
            Some(checkpoint_ref),
            groups,
            PortableTokenizerBinding::new(
                self.base_binding.tokenizer.clone(),
                PortableTokenizerAssetFormat::PsionicDigest,
                self.base_binding.base_model_ref(),
            ),
            Some(self.base_binding.prompt_template_digest.clone()),
        )?)
    }
}

/// Progress emitted while advancing one Qwen legal run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalAdapterRunProgress {
    /// Step receipts emitted while advancing the run.
    pub step_receipts: Vec<TrainingStepReceipt>,
    /// Gradient-production records emitted while advancing the run.
    pub gradient_records: Vec<OpenAdapterGradientBatchRecord>,
}

/// Error surfaced by the higher-level Qwen legal adapter smoke lane.
#[derive(Debug, Error)]
pub enum QwenLegalAdapterSftError {
    #[error("Qwen legal adapter config is invalid: {detail}")]
    InvalidConfig { detail: String },
    #[error("Qwen legal served compatibility mismatch: {detail}")]
    Compatibility { detail: String },
    #[error("Qwen legal target set is invalid: {detail}")]
    InvalidTargetSet { detail: String },
    #[error("Qwen legal dataset binding is invalid: {detail}")]
    DatasetDrift { detail: String },
    #[error("Qwen legal checkpoint is invalid: {detail}")]
    InvalidCheckpoint { detail: String },
    #[error(transparent)]
    TrainingExecution(#[from] OpenAdapterTrainingExecutionError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    OpenAdapterSft(#[from] OpenAdapterSftError),
    #[error(transparent)]
    ModelIo(#[from] ModelIoError),
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
}

fn checkpoint_id_for(
    adapter_id: &str,
    adapter_revision: &str,
    resumed_from_checkpoint_id: Option<&str>,
) -> String {
    match resumed_from_checkpoint_id {
        Some(previous) => format!("{adapter_id}-{adapter_revision}-continued-from-{previous}"),
        None => format!("{adapter_id}-{adapter_revision}-final"),
    }
}

fn stable_compatibility_digest(
    target_set: &QwenLegalAdapterTargetSet,
    base_binding: &QwenLegalServedBaseModelBinding,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_qwen_legal_adapter_compatibility|");
    hasher.update(target_set.target_set_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.adapter_target_id.as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.lora_rank.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(target_set.lora_alpha.to_bits().to_le_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.public_model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.served_model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.model_family_acceptance_label.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.base_model_revision.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.base_served_artifact_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.prompt_template_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.hidden_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.context_window_tokens.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(base_binding.tokenizer.stable_digest().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).expect("payload should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn require_nonempty(value: &str, field: &'static str) -> Result<(), QwenLegalAdapterSftError> {
    if value.trim().is_empty() {
        return Err(QwenLegalAdapterSftError::InvalidConfig {
            detail: format!("{field} must be present"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_base_binding() -> QwenLegalServedBaseModelBinding {
        let template_digest = "sha256:qwen35-legal-template-smoke";
        QwenLegalServedBaseModelBinding {
            public_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_MODEL_ID),
            served_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID),
            model_family_acceptance_label: String::from(QWEN35_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL),
            base_model_revision: String::from("qwen3.5-4b-smoke-revision"),
            base_served_artifact_digest: String::from(
                QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST,
            ),
            artifact_path: None,
            artifact_mode: QwenLegalBaseArtifactMode::SyntheticHiddenStateSmoke,
            tokenizer: TokenizerDigest::new(
                TokenizerFamily::BytePairEncoding,
                "sha256:qwen35-legal-tokenizer-smoke",
                256,
            )
            .with_template_digest(template_digest),
            prompt_template_digest: String::from(template_digest),
            hidden_size: 4,
            context_window_tokens: 128,
        }
    }

    fn sample_config() -> QwenLegalAdapterSftConfig {
        QwenLegalAdapterSftConfig {
            run_id: String::from("qwen-legal-adapter-run"),
            budget: TrainingLoopBudget::new(4, 1, 1).expect("budget"),
            batch_size: 2,
            optimizer: TrainingOptimizerConfig::adamw(0.12, 0.9, 0.99, 1e-8)
                .with_gradient_clip_norm(1.0),
            optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
        }
    }

    fn sample_supervision() -> Vec<QwenLegalLmHeadSupervisionSample> {
        vec![
            QwenLegalLmHeadSupervisionSample::new(
                "legal-a",
                vec![1.0, 0.0, 0.0, 0.0],
                12,
                41,
                "legal-record-a",
            ),
            QwenLegalLmHeadSupervisionSample::new(
                "legal-b",
                vec![0.0, 1.0, 0.0, 0.0],
                35,
                39,
                "legal-record-b",
            ),
            QwenLegalLmHeadSupervisionSample::new(
                "legal-c",
                vec![0.0, 0.0, 1.0, 0.0],
                62,
                47,
                "legal-record-c",
            ),
            QwenLegalLmHeadSupervisionSample::new(
                "legal-d",
                vec![0.0, 0.0, 0.0, 1.0],
                90,
                44,
                "legal-record-d",
            ),
        ]
    }

    fn sample_dataset_binding() -> QwenLegalDatasetBinding {
        QwenLegalDatasetBinding {
            dataset_ref: String::from("dataset://openagents/legal-benchmark/harvey-smoke@v1"),
            dataset_digest: String::from("sha256:legal-training-record-bundle-smoke"),
            training_record_schema_version: String::from(
                LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION,
            ),
            train_split_ref: String::from("split://legal-benchmark/harvey-smoke/train"),
            validation_split_ref: String::from("split://legal-benchmark/harvey-smoke/validation"),
            hidden_criterion_policy_ref: String::from(
                "policy://legal-benchmark/hidden-criteria/exclude-visible@v1",
            ),
        }
    }

    fn sample_eval_pack_binding() -> QwenLegalEvalPackBinding {
        QwenLegalEvalPackBinding {
            eval_pack_id: String::from("legal-benchmark-retained-smoke"),
            eval_pack_digest: String::from("sha256:legal-retained-smoke-eval-pack"),
            benchmark_suite_id: String::from("harvey-legal-benchmark"),
            retained_slice_id: String::from("retained-smoke"),
            scorer_version: String::from("psionic-legal-scorer.v1"),
            import_target: String::from("autopilot4://legal-benchmark/runs"),
        }
    }

    fn sample_request() -> QwenLegalAdapterSftRunRequest {
        QwenLegalAdapterSftRunRequest {
            dataset_binding: sample_dataset_binding(),
            eval_pack_binding: sample_eval_pack_binding(),
            validator_policy_ref: String::from("policy://validator/legal-benchmark/qwen-smoke"),
            adapter_id: String::from("qwen35-4b-legal-smoke"),
            adapter_revision: String::from("r1"),
            started_at_ms: 1_000,
            step_duration_ms: 20,
        }
    }

    fn sample_trainer() -> QwenLegalAdapterSftTrainer {
        QwenLegalAdapterSftTrainer::new(
            sample_config(),
            canonical_qwen_legal_adapter_target_set(),
            sample_base_binding(),
            sample_supervision(),
        )
        .expect("trainer")
    }

    #[test]
    fn qwen_legal_adapter_smoke_exports_artifact_checkpoint_and_import_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = sample_trainer();
        let outcome = trainer.run_sft(&sample_request())?;
        assert_eq!(outcome.step_receipts.len(), 4);
        assert_eq!(
            outcome.summary.run_summary.checkpoint_family,
            QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY
        );
        assert_eq!(
            outcome.exported_artifact.adapter_identity.base_model_id,
            QWEN35_4B_LEGAL_SMOKE_MODEL_ID
        );
        assert_eq!(
            trainer.backend().provenance().adapter_family,
            OPEN_ADAPTER_QWEN35_LEGAL_ADAPTER_FAMILY
        );
        assert_eq!(
            outcome.final_checkpoint.dataset_digest,
            sample_dataset_binding().dataset_digest
        );
        assert_eq!(
            outcome.score_import_bundle.import_target,
            "autopilot4://legal-benchmark/runs"
        );
        assert_eq!(
            outcome.score_import_bundle.bundle_digest,
            outcome.score_import_bundle.stable_digest()
        );
        assert_eq!(
            outcome.rl_hillclimb_plan.score_import_bundle_ref,
            outcome.score_import_bundle.bundle_id
        );
        assert_eq!(
            outcome.rl_hillclimb_plan.retained_target_model_id,
            QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID
        );
        assert_eq!(
            outcome.rl_hillclimb_plan.plan_digest,
            outcome.rl_hillclimb_plan.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_benchmark_report
                .conservative_target_score_basis_points,
            QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_benchmark_report.report_digest,
            outcome.rl_benchmark_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_optimization_window_report
                .phase_three_target_score_basis_points,
            QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_optimization_window_report.report_digest,
            outcome.rl_optimization_window_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_perfect_score_push_report
                .phase_four_target_score_basis_points,
            QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_perfect_score_push_report.report_digest,
            outcome.rl_perfect_score_push_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_retained_rehearsal_report
                .phase_five_target_score_basis_points,
            QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_retained_rehearsal_report.report_digest,
            outcome.rl_retained_rehearsal_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_expanded_corpus_report
                .phase_six_target_score_basis_points,
            QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_expanded_corpus_report.report_digest,
            outcome.rl_expanded_corpus_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_full_corpus_matrix_report
                .phase_seven_target_score_basis_points,
            QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_full_corpus_matrix_report.report_digest,
            outcome.rl_full_corpus_matrix_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_residual_burn_down_report
                .phase_eight_target_score_basis_points,
            QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_residual_burn_down_report.report_digest,
            outcome.rl_residual_burn_down_report.stable_digest()
        );
        assert_eq!(
            outcome
                .rl_final_campaign_report
                .phase_nine_target_score_basis_points,
            QWEN_LEGAL_PHASE_NINE_TARGET_SCORE_BPS
        );
        assert_eq!(
            outcome.rl_final_campaign_report.report_digest,
            outcome.rl_final_campaign_report.stable_digest()
        );
        let loaded = outcome.exported_artifact.load_lm_head_lora_artifact()?;
        assert_eq!(loaded.hidden_size, 4);
        assert_eq!(loaded.rank, QWEN_LEGAL_ADAPTER_LORA_RANK);
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_hillclimb_plan_covers_gepa_mipro_and_grpo_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let methods = plan
            .targets
            .iter()
            .map(|target| target.optimizer_method)
            .collect::<Vec<_>>();
        let failure_families = plan
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();
        let expected_lift = plan
            .targets
            .iter()
            .map(|target| target.target_lift_basis_points)
            .sum::<u16>();

        assert_eq!(
            plan.schema_version,
            QWEN_LEGAL_RL_HILLCLIMB_PLAN_SCHEMA_VERSION
        );
        assert_eq!(plan.base_lane_id, QWEN_LEGAL_ADAPTER_SFT_LANE_ID);
        assert_eq!(
            plan.smoke_served_model_id,
            QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID
        );
        assert_eq!(
            plan.retained_target_served_model_id,
            QWEN36_35B_A3B_LEGAL_RETAINED_SERVED_MODEL_ID
        );
        assert!(plan.rollout_policy.retained_slice_required);
        assert!(
            plan.rollout_policy.min_accepted_rollouts
                > plan.rollout_policy.max_quarantined_rollouts
        );
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::Grpo));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::GepaTraceSelection));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::MiproPromptSearch));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh));
        assert!(expected_lift >= 3_000);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
        ] {
            assert!(failure_families.contains(&family), "missing {family}");
        }
        assert!(
            plan.blueprint_optimizer_frontier_ref
                .contains("harvey_legal_qwen_optimizer_frontier")
        );
        assert_eq!(plan.plan_digest, plan.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_benchmark_report_projects_phase_two_retained_target()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;

        assert_eq!(
            report.schema_version,
            QWEN_LEGAL_RL_BENCHMARK_REPORT_SCHEMA_VERSION
        );
        assert_eq!(report.plan_id, QWEN_LEGAL_RL_HILLCLIMB_PLAN_ID);
        assert_eq!(report.plan_digest, plan.plan_digest);
        assert_eq!(report.baseline_score_basis_points, 5_260);
        assert_eq!(
            report.conservative_target_score_basis_points,
            QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
        );
        assert!(report.unconstrained_projected_score_basis_points > 9_000);
        assert!(report.total_target_lift_basis_points >= 4_000);
        assert_eq!(
            report.retained_target_model_id,
            QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID
        );
        assert!(report.retained_slice_required);
        assert!(report.accepted_rollout_minimum > report.quarantined_rollout_budget);
        assert!(report.grpo_target_count >= 2);
        assert!(report.gepa_target_count >= 2);
        assert!(report.mipro_target_count >= 1);
        assert!(report.supervised_refresh_target_count >= 1);
        assert_eq!(report.dataset_request_count, plan.targets.len());
        assert!(report.benchmark_export_ref.contains("autopilot4"));
        assert_eq!(report.report_digest, report.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_three_window_targets_shadow_eval_shortlist()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let methods = window
            .targets
            .iter()
            .map(|target| target.optimizer_method)
            .collect::<Vec<_>>();
        let families = window
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            window.schema_version,
            QWEN_LEGAL_RL_OPTIMIZATION_WINDOW_SCHEMA_VERSION
        );
        assert_eq!(window.plan_digest, plan.plan_digest);
        assert_eq!(window.source_report_digest, readiness_report.report_digest);
        assert_eq!(window.baseline_score_basis_points, 5_260);
        assert_eq!(
            window.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_TWO_TARGET_SCORE_BPS
        );
        assert_eq!(
            window.phase_three_target_score_basis_points,
            QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
        );
        assert!(
            window
                .baseline_score_basis_points
                .saturating_add(window.total_planned_shadow_lift_basis_points)
                >= window.phase_three_target_score_basis_points
        );
        assert_eq!(window.accepted_rollout_minimum, 84);
        assert_eq!(window.quarantined_rollout_budget, 16);
        assert_eq!(window.holdout_max_regression_basis_points, 0);
        assert!(
            window
                .blueprint_shadow_eval_shortlist_ref
                .contains("phase_three_shadow_eval_shortlist")
        );
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::Grpo));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::GepaTraceSelection));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::MiproPromptSearch));
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(window.targets.iter().all(|target| {
            target.planned_accepted_rollouts > 0
                && target.planned_shadow_lift_basis_points > 0
                && target.scorecard_ref.contains("phase_003.shadow_eval")
        }));
        assert!(window.benchmark_export_ref.ends_with("phase-003"));
        assert_eq!(window.report_digest, window.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_four_push_covers_all_frontier_families()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let methods = push
            .targets
            .iter()
            .map(|target| target.optimizer_method)
            .collect::<Vec<_>>();
        let families = push
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            push.schema_version,
            QWEN_LEGAL_RL_PERFECT_SCORE_PUSH_SCHEMA_VERSION
        );
        assert_eq!(push.plan_digest, plan.plan_digest);
        assert_eq!(push.source_window_report_digest, window.report_digest);
        assert_eq!(
            push.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_THREE_TARGET_SCORE_BPS
        );
        assert_eq!(
            push.phase_four_target_score_basis_points,
            QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
        );
        assert!(
            push.baseline_score_basis_points
                .saturating_add(push.total_planned_shadow_lift_basis_points)
                >= push.phase_four_target_score_basis_points
        );
        assert_eq!(push.accepted_rollout_minimum, 140);
        assert_eq!(push.quarantined_rollout_budget, 20);
        assert_eq!(push.holdout_max_regression_basis_points, 0);
        assert!(push.calibrated_judge_disagreement_budget_basis_points <= 75);
        assert!(
            push.blueprint_perfect_score_push_plan_ref
                .contains("phase_four_perfect_score_push_plan")
        );
        assert_eq!(push.targets.len(), 9);
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::Grpo));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::GepaTraceSelection));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::MiproPromptSearch));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::SupervisedFineTuneRefresh));
        assert!(methods.contains(&QwenLegalRlOptimizerMethod::ManualReview));
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(push.targets.iter().all(|target| {
            target.judge_adjudication_required
                && target.planned_accepted_rollouts > 0
                && target.planned_shadow_lift_basis_points > 0
                && target.scorecard_ref.contains("phase_004.perfect_push")
        }));
        assert!(push.benchmark_export_ref.ends_with("phase-004"));
        assert_eq!(push.report_digest, push.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_five_rehearsal_targets_ninety_percent()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let rehearsal = qwen_legal_rl_phase_five_retained_rehearsal_report(&plan, &push)?;
        let families = rehearsal
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            rehearsal.schema_version,
            QWEN_LEGAL_RL_RETAINED_REHEARSAL_SCHEMA_VERSION
        );
        assert_eq!(rehearsal.plan_digest, plan.plan_digest);
        assert_eq!(rehearsal.source_push_report_digest, push.report_digest);
        assert_eq!(
            rehearsal.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_FOUR_TARGET_SCORE_BPS
        );
        assert_eq!(
            rehearsal.phase_five_target_score_basis_points,
            QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
        );
        assert!(
            rehearsal
                .baseline_score_basis_points
                .saturating_add(rehearsal.total_planned_rehearsal_lift_basis_points)
                >= rehearsal.phase_five_target_score_basis_points
        );
        assert_eq!(rehearsal.retained_rehearsal_run_count, 60);
        assert_eq!(rehearsal.accepted_rollout_minimum, 194);
        assert_eq!(rehearsal.quarantined_rollout_budget, 24);
        assert_eq!(rehearsal.adversarial_holdout_run_count, 36);
        assert_eq!(rehearsal.holdout_max_regression_basis_points, 0);
        assert!(rehearsal.calibrated_judge_disagreement_budget_basis_points <= 50);
        assert!(
            rehearsal
                .blueprint_retained_rehearsal_plan_ref
                .contains("phase_five_retained_rehearsal_plan")
        );
        assert_eq!(rehearsal.targets.len(), 9);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(rehearsal.targets.iter().all(|target| {
            target.judge_panel_required
                && target.retained_replay_count == 3
                && target.planned_accepted_rollouts > 0
                && target.planned_rehearsal_lift_basis_points > 0
                && target
                    .scorecard_ref
                    .contains("phase_005.retained_rehearsal")
                && target.receipt_ref.contains("phase_005.retained_rehearsal")
        }));
        assert!(rehearsal.benchmark_export_ref.ends_with("phase-005"));
        assert_eq!(rehearsal.report_digest, rehearsal.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_six_expanded_corpus_targets_95_percent()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let rehearsal = qwen_legal_rl_phase_five_retained_rehearsal_report(&plan, &push)?;
        let expanded = qwen_legal_rl_phase_six_expanded_corpus_report(&plan, &rehearsal)?;
        let families = expanded
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            expanded.schema_version,
            QWEN_LEGAL_RL_EXPANDED_CORPUS_SCHEMA_VERSION
        );
        assert_eq!(expanded.plan_digest, plan.plan_digest);
        assert_eq!(
            expanded.source_rehearsal_report_digest,
            rehearsal.report_digest
        );
        assert_eq!(
            expanded.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_FIVE_TARGET_SCORE_BPS
        );
        assert_eq!(
            expanded.phase_six_target_score_basis_points,
            QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
        );
        assert!(
            expanded
                .baseline_score_basis_points
                .saturating_add(expanded.total_planned_expanded_lift_basis_points)
                >= expanded.phase_six_target_score_basis_points
        );
        assert_eq!(expanded.expanded_slice_task_count, 125);
        assert_eq!(expanded.practice_area_count, 24);
        assert_eq!(expanded.accepted_rollout_minimum, 266);
        assert_eq!(expanded.quarantined_rollout_budget, 30);
        assert_eq!(expanded.adversarial_holdout_run_count, 72);
        assert_eq!(expanded.holdout_max_regression_basis_points, 0);
        assert!(expanded.calibrated_judge_disagreement_budget_basis_points <= 35);
        assert!(
            expanded
                .blueprint_expanded_corpus_plan_ref
                .contains("phase_six_expanded_corpus_plan")
        );
        assert_eq!(expanded.targets.len(), 9);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(expanded.targets.iter().all(|target| {
            target.practice_area_balance_required
                && target.expanded_slice_task_count == 125
                && target.planned_accepted_rollouts > 0
                && target.planned_expanded_lift_basis_points > 0
                && target.scorecard_ref.contains("phase_006.expanded_corpus")
                && target.receipt_ref.contains("phase_006.expanded_corpus")
        }));
        assert!(expanded.benchmark_export_ref.ends_with("phase-006"));
        assert_eq!(expanded.report_digest, expanded.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_seven_full_corpus_matrix_targets_98_percent()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let rehearsal = qwen_legal_rl_phase_five_retained_rehearsal_report(&plan, &push)?;
        let expanded = qwen_legal_rl_phase_six_expanded_corpus_report(&plan, &rehearsal)?;
        let full_corpus = qwen_legal_rl_phase_seven_full_corpus_matrix_report(&plan, &expanded)?;
        let families = full_corpus
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            full_corpus.schema_version,
            QWEN_LEGAL_RL_FULL_CORPUS_MATRIX_SCHEMA_VERSION
        );
        assert_eq!(full_corpus.plan_digest, plan.plan_digest);
        assert_eq!(
            full_corpus.source_expanded_report_digest,
            expanded.report_digest
        );
        assert_eq!(
            full_corpus.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_SIX_TARGET_SCORE_BPS
        );
        assert_eq!(
            full_corpus.phase_seven_target_score_basis_points,
            QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
        );
        assert!(
            full_corpus
                .baseline_score_basis_points
                .saturating_add(full_corpus.total_planned_full_corpus_lift_basis_points)
                >= full_corpus.phase_seven_target_score_basis_points
        );
        assert_eq!(full_corpus.full_corpus_task_count, 1251);
        assert_eq!(full_corpus.practice_area_count, 24);
        assert_eq!(full_corpus.model_matrix_cell_count, 48);
        assert_eq!(full_corpus.accepted_rollout_minimum, 410);
        assert_eq!(full_corpus.quarantined_rollout_budget, 38);
        assert_eq!(full_corpus.adversarial_holdout_run_count, 144);
        assert_eq!(full_corpus.holdout_max_regression_basis_points, 0);
        assert!(full_corpus.calibrated_judge_disagreement_budget_basis_points <= 25);
        assert!(
            full_corpus
                .blueprint_full_corpus_matrix_plan_ref
                .contains("phase_seven_full_corpus_matrix_plan")
        );
        assert_eq!(full_corpus.targets.len(), 9);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(full_corpus.targets.iter().all(|target| {
            target.practice_area_balance_required
                && target.full_corpus_task_count == 1251
                && target.model_matrix_cell_count == 48
                && target.planned_accepted_rollouts > 0
                && target.planned_full_corpus_lift_basis_points > 0
                && target
                    .scorecard_ref
                    .contains("phase_007.full_corpus_matrix")
                && target.receipt_ref.contains("phase_007.full_corpus_matrix")
        }));
        assert!(full_corpus.benchmark_export_ref.ends_with("phase-007"));
        assert_eq!(full_corpus.report_digest, full_corpus.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_eight_residual_burn_down_targets_99_percent()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let rehearsal = qwen_legal_rl_phase_five_retained_rehearsal_report(&plan, &push)?;
        let expanded = qwen_legal_rl_phase_six_expanded_corpus_report(&plan, &rehearsal)?;
        let full_corpus = qwen_legal_rl_phase_seven_full_corpus_matrix_report(&plan, &expanded)?;
        let residual = qwen_legal_rl_phase_eight_residual_burn_down_report(&plan, &full_corpus)?;
        let families = residual
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            residual.schema_version,
            QWEN_LEGAL_RL_RESIDUAL_BURN_DOWN_SCHEMA_VERSION
        );
        assert_eq!(residual.plan_digest, plan.plan_digest);
        assert_eq!(
            residual.source_full_corpus_report_digest,
            full_corpus.report_digest
        );
        assert_eq!(
            residual.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_SEVEN_TARGET_SCORE_BPS
        );
        assert_eq!(
            residual.phase_eight_target_score_basis_points,
            QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
        );
        assert!(
            residual
                .baseline_score_basis_points
                .saturating_add(residual.total_planned_residual_lift_basis_points)
                >= residual.phase_eight_target_score_basis_points
        );
        assert_eq!(residual.full_corpus_task_count, 1251);
        assert_eq!(residual.practice_area_count, 24);
        assert_eq!(residual.model_matrix_cell_count, 96);
        assert_eq!(residual.residual_cluster_budget, 24);
        assert_eq!(residual.accepted_rollout_minimum, 626);
        assert_eq!(residual.quarantined_rollout_budget, 48);
        assert_eq!(residual.adversarial_holdout_run_count, 288);
        assert_eq!(residual.holdout_max_regression_basis_points, 0);
        assert!(residual.calibrated_judge_disagreement_budget_basis_points <= 15);
        assert!(
            residual
                .blueprint_residual_burn_down_plan_ref
                .contains("phase_eight_residual_burn_down_plan")
        );
        assert_eq!(residual.targets.len(), 9);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(residual.targets.iter().all(|target| {
            target.practice_area_balance_required
                && target.full_corpus_task_count == 1251
                && target.model_matrix_cell_count == 96
                && target.residual_cluster_budget == 24
                && target.planned_accepted_rollouts > 0
                && target.planned_residual_lift_basis_points > 0
                && target
                    .scorecard_ref
                    .contains("phase_008.residual_burn_down")
                && target.receipt_ref.contains("phase_008.residual_burn_down")
        }));
        assert!(residual.benchmark_export_ref.ends_with("phase-008"));
        assert_eq!(residual.report_digest, residual.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_rl_phase_nine_final_campaign_targets_995_percent()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = canonical_qwen_legal_rl_hillclimb_plan(
            sample_dataset_binding(),
            sample_eval_pack_binding(),
            "qwen35-4b-legal-smoke-r1-score-import",
        )?;
        let readiness_report = qwen_legal_rl_benchmark_readiness_report(&plan, 5_260)?;
        let window =
            qwen_legal_rl_phase_three_optimization_window_report(&plan, &readiness_report)?;
        let push = qwen_legal_rl_phase_four_perfect_score_push_report(&plan, &window)?;
        let rehearsal = qwen_legal_rl_phase_five_retained_rehearsal_report(&plan, &push)?;
        let expanded = qwen_legal_rl_phase_six_expanded_corpus_report(&plan, &rehearsal)?;
        let full_corpus = qwen_legal_rl_phase_seven_full_corpus_matrix_report(&plan, &expanded)?;
        let residual = qwen_legal_rl_phase_eight_residual_burn_down_report(&plan, &full_corpus)?;
        let final_campaign = qwen_legal_rl_phase_nine_final_campaign_report(&plan, &residual)?;
        let families = final_campaign
            .targets
            .iter()
            .map(|target| target.failure_family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            final_campaign.schema_version,
            QWEN_LEGAL_RL_FINAL_CAMPAIGN_SCHEMA_VERSION
        );
        assert_eq!(final_campaign.plan_digest, plan.plan_digest);
        assert_eq!(
            final_campaign.source_residual_burn_down_report_digest,
            residual.report_digest
        );
        assert_eq!(
            final_campaign.previous_target_score_basis_points,
            QWEN_LEGAL_PHASE_EIGHT_TARGET_SCORE_BPS
        );
        assert_eq!(
            final_campaign.phase_nine_target_score_basis_points,
            QWEN_LEGAL_PHASE_NINE_TARGET_SCORE_BPS
        );
        assert!(
            final_campaign
                .baseline_score_basis_points
                .saturating_add(final_campaign.total_planned_final_lift_basis_points)
                >= final_campaign.phase_nine_target_score_basis_points
        );
        assert_eq!(final_campaign.full_corpus_task_count, 1251);
        assert_eq!(final_campaign.practice_area_count, 24);
        assert_eq!(final_campaign.model_matrix_cell_count, 144);
        assert_eq!(final_campaign.residual_cluster_budget, 12);
        assert_eq!(final_campaign.final_adjudication_sample_count, 96);
        assert_eq!(final_campaign.accepted_rollout_minimum, 950);
        assert_eq!(final_campaign.quarantined_rollout_budget, 52);
        assert_eq!(final_campaign.adversarial_holdout_run_count, 432);
        assert_eq!(final_campaign.holdout_max_regression_basis_points, 0);
        assert!(final_campaign.calibrated_judge_disagreement_budget_basis_points <= 10);
        assert!(
            final_campaign
                .blueprint_final_campaign_plan_ref
                .contains("phase_nine_final_campaign_plan")
        );
        assert_eq!(final_campaign.targets.len(), 9);
        for family in [
            "document_coverage",
            "citation_evidence",
            "legal_reasoning",
            "spreadsheet_reasoning",
            "missing_fact",
            "pre_submit_self_check",
            "deliverable_completeness",
            "fine_tune_data_selection",
            "task_intake_routing",
        ] {
            assert!(families.contains(&family), "missing {family}");
        }
        assert!(final_campaign.targets.iter().all(|target| {
            target.practice_area_balance_required
                && target.full_corpus_task_count == 1251
                && target.model_matrix_cell_count == 144
                && target.residual_cluster_budget == 12
                && target.final_adjudication_sample_count == 96
                && target.planned_accepted_rollouts > 0
                && target.planned_final_lift_basis_points > 0
                && target
                    .scorecard_ref
                    .contains("phase_009.final_campaign_rehearsal")
                && target
                    .receipt_ref
                    .contains("phase_009.final_campaign_rehearsal")
        }));
        assert!(final_campaign.benchmark_export_ref.ends_with("phase-009"));
        assert_eq!(final_campaign.report_digest, final_campaign.stable_digest());
        Ok(())
    }

    #[test]
    fn qwen_legal_adapter_refuses_template_and_dataset_schema_drift() {
        let mut base = sample_base_binding();
        base.prompt_template_digest = String::from("sha256:drifted-template");
        let template_error = QwenLegalAdapterSftTrainer::new(
            sample_config(),
            canonical_qwen_legal_adapter_target_set(),
            base,
            sample_supervision(),
        )
        .expect_err("template drift must refuse");
        assert!(template_error.to_string().contains("template digest"));

        let trainer = sample_trainer();
        let mut request = sample_request();
        request.dataset_binding.training_record_schema_version =
            String::from("psionic.legal_benchmark_training_record.v0");
        let dataset_error = trainer
            .run_sft(&request)
            .expect_err("dataset schema drift must refuse");
        assert!(
            dataset_error
                .to_string()
                .contains("legal training record schema")
        );
    }

    #[test]
    fn qwen_legal_adapter_gates_real_artifact_execution() {
        let mut real_binding = sample_base_binding();
        real_binding.artifact_mode = QwenLegalBaseArtifactMode::RealArtifactRequired;
        real_binding.base_served_artifact_digest = String::from("sha256:real-qwen35-4b-artifact");
        real_binding.artifact_path = None;
        let error = QwenLegalAdapterSftTrainer::new(
            sample_config(),
            canonical_qwen_legal_adapter_target_set(),
            real_binding,
            sample_supervision(),
        )
        .expect_err("real artifact mode must require a path");
        assert!(error.to_string().contains("artifact path"));
    }

    #[test]
    fn qwen_legal_adapter_checkpoint_restores_exact_run_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let trainer = sample_trainer();
        let mut run = trainer.initialize_run()?;
        let progress = trainer.advance_run(&mut run, Some(2), 1_000, 20)?;
        assert_eq!(progress.step_receipts.len(), 2);
        let checkpoint = trainer.save_checkpoint(
            "qwen-legal-midpoint",
            &run,
            &sample_dataset_binding(),
            &sample_eval_pack_binding(),
            1_040,
        )?;
        let restored = trainer.restore_run(&checkpoint)?;
        assert_eq!(restored.summary().completed_steps, 2);
        let resumed = trainer.run_sft_from_checkpoint(&checkpoint, &sample_request())?;
        assert_eq!(resumed.summary.run_summary.completed_steps, 4);
        assert_eq!(
            resumed.summary.run_summary.last_receipt_id.as_deref(),
            Some("qwen-legal-adapter-run-step-4")
        );
        Ok(())
    }
}
