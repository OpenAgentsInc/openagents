use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
};
use psionic_core::{QuantizationMode, TensorData};
use psionic_data::{TokenizerDigest, TokenizerFamily};
use psionic_eval::{LegalReward, LegalRewardWeights};
use psionic_models::{
    PromptMessage, PromptMessageRole, Qwen36PromptOptions, Qwen36PromptReceipt,
    Qwen36PromptRenderer, Qwen36ReasoningMode, Qwen36TemplateError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    OPEN_ADAPTER_QWEN36_LEGAL_CUDA_BACKEND_LABEL, OpenAdapterAdmissibleModelFamily,
    OpenAdapterArtifactExportRequest, OpenAdapterExecutionConfig, OpenAdapterHiddenStateSample,
    OpenAdapterLmHeadTarget, OpenAdapterPrecisionPolicy, OpenAdapterReferenceModel,
    OpenAdapterSftError, OpenAdapterTrainingExecutionBackend, OpenAdapterTrainingExecutionError,
    OpenAdapterWeightedTargetBatchRecord, OpenAdapterWeightedTargetBatchRequest,
    OpenAdapterWeightedTokenTarget, PsionicLegalSftBaseArtifactMode, PsionicLegalSftError,
    PsionicTrainingReceipt, TrainingCoreError, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterGroupState, TrainingRunSummary,
    TrainingStepReceipt, run_psionic_legal_sft_config_path,
};

/// Config schema accepted by `psionic-train grpo --config`.
pub const PSIONIC_LEGAL_GRPO_CONFIG_SCHEMA_VERSION: &str = "psionic.legal_grpo_config.v1";
/// Receipt schema emitted by `psionic-train grpo`.
pub const PSIONIC_LEGAL_GRPO_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.legal_grpo_training_receipt.v1";
/// Checkpoint summary schema emitted by `psionic-train grpo`.
pub const PSIONIC_LEGAL_GRPO_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.legal_grpo_checkpoint_summary.v1";
/// Loss/eval curve schema emitted by `psionic-train grpo`.
pub const PSIONIC_LEGAL_GRPO_LOSS_CURVE_SCHEMA_VERSION: &str = "psionic.legal_grpo_loss_curve.v1";
/// Rollout trace schema emitted by `psionic-train grpo`.
pub const PSIONIC_LEGAL_GRPO_ROLLOUT_TRACE_SCHEMA_VERSION: &str =
    "psionic.legal_grpo_rollout_trace.v1";

const QWEN36_LEGAL_GRPO_CHECKPOINT_FAMILY: &str = "psionic.qwen36.legal_adapter_grpo";
const DEFAULT_QWEN36_BASE_DIGEST: &str = "sha256:synthetic-qwen36-legal-smoke";
const DEFAULT_QWEN36_TEMPLATE_DIGEST: &str = "sha256:qwen36-chat-template-v1-smoke";
const DEFAULT_QWEN36_TOKENIZER_DIGEST: &str = "sha256:qwen36-tokenizer-smoke";
const ANSWER_LENGTH_MAX_BYTES: u64 = 100_000;

/// Config for one Rust-only legal GRPO smoke run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoConfig {
    /// Schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Train family, expected to be `grpo`.
    pub train_type: String,
    /// Public base model id.
    pub base_model: String,
    /// Served model id used by eval metadata.
    pub served_model_id: String,
    /// Base model revision or fixture revision.
    pub base_model_revision: String,
    /// Synthetic smoke or real artifact mode.
    pub base_artifact_mode: PsionicLegalSftBaseArtifactMode,
    /// Digest of the frozen base artifact or synthetic fixture.
    pub base_served_artifact_digest: String,
    /// Tokenizer digest used by the Qwen3.6 prompt template.
    #[serde(default = "default_tokenizer_digest")]
    pub tokenizer_digest: String,
    /// Prompt template digest.
    #[serde(default = "default_template_digest")]
    pub prompt_template_digest: String,
    /// Synthetic fixture hidden width.
    pub hidden_size: usize,
    /// Synthetic fixture vocabulary width.
    pub vocab_size: usize,
    /// Adapter target used by the current smoke trainer.
    #[serde(default = "default_adapter_target_id")]
    pub adapter_target_id: String,
    /// LoRA rank.
    pub lora_rank: usize,
    /// LoRA alpha.
    pub lora_alpha: f32,
    /// Learning rate.
    pub learning_rate: f32,
    /// Fixed step count for the smoke trainer.
    pub max_steps: u64,
    /// Maximum weighted targets consumed per update.
    pub batch_size: usize,
    /// Number of sampled completions per prompt group.
    pub group_size: usize,
    /// Scalar applied to normalized rewards before adapter updates.
    pub reward_scale: f32,
    /// Reference KL penalty reserved for the full online trainer.
    pub kl_reference_penalty: f32,
    /// Teacher target blend passed to the open-adapter backend.
    pub teacher_target_blend: f32,
    /// Assistant-only loss flag.
    pub assistant_only_loss: bool,
    /// Whether empty Qwen think blocks are masked out.
    pub ignore_empty_think_loss: bool,
    /// Qwen3.6 reasoning mode used for prompt rendering.
    pub qwen36_reasoning_mode: Qwen36ReasoningMode,
    /// Whether direct-answer prompt rendering emits an empty think block.
    pub emit_empty_think_block: bool,
    /// Parent SFT adapter artifact path.
    pub parent_sft_adapter_path: String,
    /// Parent SFT training receipt path.
    pub parent_sft_receipt_path: String,
    /// Parent SFT adapter id.
    pub parent_sft_adapter_id: String,
    /// Parent SFT adapter revision.
    pub parent_sft_adapter_revision: String,
    /// Optional parent SFT config path used to bootstrap the parent if missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_sft_config_path: Option<String>,
    /// Whether to run the parent SFT config if the parent artifact is absent.
    #[serde(default)]
    pub bootstrap_parent_sft_if_missing: bool,
    /// Prompt groups used for deterministic local group sampling.
    pub prompt_groups: Vec<PsionicLegalGrpoPromptGroup>,
    /// Stable dataset ref carried into export lineage.
    pub dataset_ref: String,
    /// Validator policy ref carried into export lineage.
    pub validator_policy_ref: String,
    /// Eval suite id recorded in the receipt.
    pub eval_suite_id: String,
    /// Output adapter id.
    pub adapter_id: String,
    /// Output adapter revision.
    pub adapter_revision: String,
    /// Output directory.
    pub output_dir: String,
    /// Logical start timestamp.
    pub started_at_ms: u64,
    /// Logical duration per step.
    pub step_duration_ms: u64,
}

impl PsionicLegalGrpoConfig {
    fn validate(&self) -> Result<(), PsionicLegalGrpoError> {
        if self.schema_version != PSIONIC_LEGAL_GRPO_CONFIG_SCHEMA_VERSION {
            return invalid_config("schema_version is not psionic.legal_grpo_config.v1");
        }
        require_nonempty(self.run_id.as_str(), "run_id")?;
        if self.train_type != "grpo" {
            return invalid_config("train_type must be `grpo`");
        }
        require_nonempty(self.base_model.as_str(), "base_model")?;
        require_nonempty(self.served_model_id.as_str(), "served_model_id")?;
        require_nonempty(self.base_model_revision.as_str(), "base_model_revision")?;
        require_nonempty(
            self.base_served_artifact_digest.as_str(),
            "base_served_artifact_digest",
        )?;
        require_nonempty(self.tokenizer_digest.as_str(), "tokenizer_digest")?;
        require_nonempty(
            self.prompt_template_digest.as_str(),
            "prompt_template_digest",
        )?;
        if self.hidden_size == 0 || self.vocab_size == 0 {
            return invalid_config("hidden_size and vocab_size must be non-zero");
        }
        require_nonempty(self.adapter_target_id.as_str(), "adapter_target_id")?;
        if self.lora_rank == 0 || !self.lora_alpha.is_finite() || self.lora_alpha <= 0.0 {
            return invalid_config("lora_rank and lora_alpha must be positive");
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return invalid_config("learning_rate must be positive and finite");
        }
        if self.max_steps == 0 || self.batch_size == 0 || self.step_duration_ms == 0 {
            return invalid_config("max_steps, batch_size, and step_duration_ms must be non-zero");
        }
        if self.group_size < 2 {
            return invalid_config("group_size must be at least two");
        }
        if !self.reward_scale.is_finite() || self.reward_scale <= 0.0 {
            return invalid_config("reward_scale must be positive and finite");
        }
        if !self.kl_reference_penalty.is_finite() || self.kl_reference_penalty < 0.0 {
            return invalid_config("kl_reference_penalty must be non-negative and finite");
        }
        if !self.teacher_target_blend.is_finite() || self.teacher_target_blend < 0.0 {
            return invalid_config("teacher_target_blend must be non-negative and finite");
        }
        if !self.assistant_only_loss {
            return invalid_config("GRPO smoke requires assistant_only_loss = true");
        }
        if !self.ignore_empty_think_loss {
            return invalid_config("GRPO smoke requires ignore_empty_think_loss = true");
        }
        require_nonempty(
            self.parent_sft_adapter_path.as_str(),
            "parent_sft_adapter_path",
        )?;
        require_nonempty(
            self.parent_sft_receipt_path.as_str(),
            "parent_sft_receipt_path",
        )?;
        require_nonempty(self.parent_sft_adapter_id.as_str(), "parent_sft_adapter_id")?;
        require_nonempty(
            self.parent_sft_adapter_revision.as_str(),
            "parent_sft_adapter_revision",
        )?;
        if self.prompt_groups.is_empty() {
            return invalid_config("prompt_groups must not be empty");
        }
        for prompt_group in &self.prompt_groups {
            validate_prompt_group(prompt_group, self.group_size)?;
        }
        require_nonempty(self.dataset_ref.as_str(), "dataset_ref")?;
        require_nonempty(self.validator_policy_ref.as_str(), "validator_policy_ref")?;
        require_nonempty(self.eval_suite_id.as_str(), "eval_suite_id")?;
        require_nonempty(self.adapter_id.as_str(), "adapter_id")?;
        require_nonempty(self.adapter_revision.as_str(), "adapter_revision")?;
        require_nonempty(self.output_dir.as_str(), "output_dir")?;
        Ok(())
    }
}

/// One prompt group for deterministic local GRPO sampling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoPromptGroup {
    /// Stable prompt id.
    pub prompt_id: String,
    /// Legal benchmark task id represented by this prompt.
    pub task_id: String,
    /// Legal benchmark task version represented by this prompt.
    pub task_version: String,
    /// Required answer path for the represented task.
    pub required_answer_path: String,
    /// Whether the task requires source-document usage.
    pub source_required: bool,
    /// System prompt rendered through the Qwen3.6 chat template.
    pub system_prompt: String,
    /// User prompt rendered through the Qwen3.6 chat template.
    pub user_prompt: String,
    /// Deterministic sampled completions for this smoke group.
    pub sampled_completions: Vec<PsionicLegalGrpoCompletionSample>,
}

/// One sampled completion and the observable workflow behavior it produced.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoCompletionSample {
    /// Stable completion id within its prompt group.
    pub completion_id: String,
    /// Completion text preserved for future data building.
    pub completion_text: String,
    /// Whether the completion wrote the required answer file.
    pub wrote_required_file: bool,
    /// Optional path written by the completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    /// Answer byte size observed after the completion.
    pub answer_byte_size: u64,
    /// Whether the completion used task sources.
    pub source_used: bool,
    /// Whether the completion submitted the run.
    pub submitted: bool,
    /// Whether answer-integrity checks passed.
    pub integrity_valid: bool,
    /// Whether the completion saw hidden/private scoring data.
    #[serde(default)]
    pub hidden_leakage: bool,
    /// Public score delta in basis points, if known.
    #[serde(default)]
    pub public_score_delta_bps: i32,
}

/// One rollout feature used by the open-adapter update.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoRolloutFeature {
    /// Stable prompt id.
    pub prompt_id: String,
    /// Stable rollout id.
    pub rollout_id: String,
    /// Qwen3.6 prompt receipt.
    pub prompt_receipt: Qwen36PromptReceipt,
    /// Synthetic hidden state for this prompt.
    pub hidden_state: Vec<f32>,
    /// Token id representing this sampled completion.
    pub target_token_id: u32,
    /// Approximate prompt and completion token count.
    pub source_token_count: u32,
    /// Raw total reward from the verifier-style plugin.
    pub total_reward: f32,
    /// Group-normalized reward used as the GRPO-style advantage.
    pub normalized_advantage: f32,
    /// Whether this rollout is kept as a bad completion.
    pub bad_completion: bool,
    /// Reward trace for this rollout.
    pub reward_trace: PsionicLegalGrpoRewardTrace,
}

/// One reward trace emitted by the GRPO smoke trainer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoRewardTrace {
    /// Schema version.
    pub schema_version: String,
    /// Stable trace id.
    pub trace_id: String,
    /// Stable run id.
    pub run_id: String,
    /// Prompt group id.
    pub prompt_id: String,
    /// Completion id.
    pub completion_id: String,
    /// Legal task id represented by the rollout.
    pub task_id: String,
    /// Legal task version represented by the rollout.
    pub task_version: String,
    /// Required answer path.
    pub required_answer_path: String,
    /// Optional observed output path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_output_path: Option<String>,
    /// Reward components.
    pub components: LegalReward,
    /// Reward weights.
    pub weights: LegalRewardWeights,
    /// Workflow reward before public score delta.
    pub workflow_reward: f32,
    /// Public score delta reward.
    pub public_score_delta_reward: f32,
    /// Total reward used for the group.
    pub total_reward: f32,
    /// Group-normalized advantage.
    pub normalized_advantage: f32,
    /// Whether this rollout is excluded from positive learning.
    pub fatal_excluded: bool,
    /// Human-readable exclusion reasons.
    pub exclusion_reasons: Vec<String>,
    /// Completion text preserved for later preference/RL data generation.
    pub completion_text: String,
    /// Stable digest over the trace.
    pub trace_digest: String,
}

impl PsionicLegalGrpoRewardTrace {
    fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.trace_digest.clear();
        stable_json_digest(b"psionic_legal_grpo_reward_trace|", &clone)
    }
}

/// Reward plugin trait used by the GRPO smoke trainer.
pub trait PsionicLegalGrpoRewardFunction {
    /// Scores one sampled completion.
    fn score(
        &self,
        run_id: &str,
        prompt_group: &PsionicLegalGrpoPromptGroup,
        completion: &PsionicLegalGrpoCompletionSample,
    ) -> PsionicLegalGrpoRewardTrace;
}

/// Verifier-style legal workflow reward used by the first GRPO smoke trainer.
#[derive(Clone, Debug)]
pub struct PsionicLegalWorkflowRewardPlugin {
    weights: LegalRewardWeights,
}

impl Default for PsionicLegalWorkflowRewardPlugin {
    fn default() -> Self {
        Self {
            weights: LegalRewardWeights::default(),
        }
    }
}

impl PsionicLegalGrpoRewardFunction for PsionicLegalWorkflowRewardPlugin {
    fn score(
        &self,
        run_id: &str,
        prompt_group: &PsionicLegalGrpoPromptGroup,
        completion: &PsionicLegalGrpoCompletionSample,
    ) -> PsionicLegalGrpoRewardTrace {
        let correct_path = completion
            .output_path
            .as_deref()
            .is_some_and(|path| path == prompt_group.required_answer_path);
        let non_empty_answer = completion.answer_byte_size > 0;
        let answer_length_ok = completion.answer_byte_size > 0
            && completion.answer_byte_size <= ANSWER_LENGTH_MAX_BYTES;
        let source_usage_ok = !prompt_group.source_required || completion.source_used;
        let components = LegalReward {
            wrote_required_file: bool_reward(completion.wrote_required_file),
            correct_path: bool_reward(completion.wrote_required_file && correct_path),
            non_empty_answer: bool_reward(completion.wrote_required_file && non_empty_answer),
            answer_length_ok: bool_reward(completion.wrote_required_file && answer_length_ok),
            source_usage_ok: bool_reward(source_usage_ok),
            submitted_ok: bool_reward(completion.submitted),
            integrity_valid: bool_reward(completion.integrity_valid),
            public_score_delta: completion.public_score_delta_bps as f32 / 10_000.0,
            no_hidden_leakage: bool_reward(!completion.hidden_leakage),
        };
        let workflow_reward = components.wrote_required_file * self.weights.wrote_required_file
            + components.correct_path * self.weights.correct_path
            + components.non_empty_answer * self.weights.non_empty_answer
            + components.answer_length_ok * self.weights.answer_length_ok
            + components.source_usage_ok * self.weights.source_usage_ok
            + components.submitted_ok * self.weights.submitted_ok
            + components.integrity_valid * self.weights.integrity_valid;
        let mut exclusion_reasons = Vec::new();
        if completion.hidden_leakage {
            exclusion_reasons.push(String::from("hidden_or_private_scoring_leakage"));
        }
        if completion.wrote_required_file && !completion.integrity_valid {
            exclusion_reasons.push(String::from("answer_integrity_failed"));
        }
        let fatal_excluded = completion.hidden_leakage;
        let public_score_delta_reward =
            components.public_score_delta * self.weights.public_score_delta;
        let total_reward = if fatal_excluded {
            0.0
        } else {
            workflow_reward + public_score_delta_reward
        };
        let mut trace = PsionicLegalGrpoRewardTrace {
            schema_version: String::from(PSIONIC_LEGAL_GRPO_ROLLOUT_TRACE_SCHEMA_VERSION),
            trace_id: format!(
                "grpo.{}.{}.{}",
                run_id, prompt_group.prompt_id, completion.completion_id
            ),
            run_id: run_id.to_string(),
            prompt_id: prompt_group.prompt_id.clone(),
            completion_id: completion.completion_id.clone(),
            task_id: prompt_group.task_id.clone(),
            task_version: prompt_group.task_version.clone(),
            required_answer_path: prompt_group.required_answer_path.clone(),
            observed_output_path: completion.output_path.clone(),
            components,
            weights: self.weights.clone(),
            workflow_reward,
            public_score_delta_reward,
            total_reward,
            normalized_advantage: 0.0,
            fatal_excluded,
            exclusion_reasons,
            completion_text: completion.completion_text.clone(),
            trace_digest: String::new(),
        };
        trace.trace_digest = trace.stable_digest();
        trace
    }
}

/// One loss/eval row emitted by the GRPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoLossPoint {
    /// Global step.
    pub step: u64,
    /// Mean weighted loss from the weighted-target batch.
    pub loss: f32,
    /// Mean raw chosen-token loss before weighting.
    pub raw_loss: f32,
    /// Mean loss weight.
    pub mean_loss_weight: f32,
    /// Batch id.
    pub batch_id: String,
    /// Step receipt digest.
    pub receipt_digest: String,
    /// Preference accuracy after the step.
    pub file_write_preference_accuracy: f32,
    /// Average best-minus-worst reward logprob margin after the step.
    pub average_reward_margin: f32,
}

/// Loss and eval curve artifact emitted by the GRPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoLossCurve {
    /// Schema version.
    pub schema_version: String,
    /// Run id.
    pub run_id: String,
    /// Initial eval before GRPO updates.
    pub initial_eval: PsionicLegalGrpoEvalPoint,
    /// Per-step loss/eval points.
    pub points: Vec<PsionicLegalGrpoLossPoint>,
}

/// Reward preference eval point over the sampled rollout groups.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoEvalPoint {
    /// Prompt group count.
    pub group_count: usize,
    /// Fraction of groups where best-reward completion beats worst-reward completion.
    pub file_write_preference_accuracy: f32,
    /// Average best-minus-worst logprob margin.
    pub average_reward_margin: f32,
}

/// Deterministic checkpoint summary emitted by the GRPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoCheckpointSummary {
    /// Schema version.
    pub schema_version: String,
    /// Run id.
    pub run_id: String,
    /// Checkpoint family.
    pub checkpoint_family: String,
    /// Final run summary.
    pub run_summary: TrainingRunSummary,
    /// Parent adapter digest.
    pub parent_adapter_digest: String,
    /// Final adapter digest.
    pub final_adapter_digest: String,
    /// Final adapter identity digest.
    pub final_adapter_identity_digest: String,
    /// Step receipt digests.
    pub step_receipt_digests: Vec<String>,
    /// Weighted GRPO batch digests.
    pub weighted_batch_digests: Vec<String>,
    /// Reward trace digests.
    pub reward_trace_digests: Vec<String>,
    /// Stable checkpoint digest.
    pub checkpoint_digest: String,
}

impl PsionicLegalGrpoCheckpointSummary {
    fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.checkpoint_digest.clear();
        stable_json_digest(b"psionic_legal_grpo_checkpoint|", &clone)
    }
}

/// Machine-readable receipt emitted by one `psionic-train grpo` run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoTrainingReceipt {
    /// Schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Trainer implementation id.
    pub trainer: String,
    /// Train type.
    pub train_type: String,
    /// Base model id.
    pub base_model: String,
    /// Served model id.
    pub served_model_id: String,
    /// Base revision.
    pub base_model_revision: String,
    /// Base artifact mode.
    pub base_artifact_mode: PsionicLegalSftBaseArtifactMode,
    /// Base artifact digest.
    pub base_served_artifact_digest: String,
    /// Tokenizer digest.
    pub tokenizer_digest: String,
    /// Prompt template digest.
    pub prompt_template_digest: String,
    /// Qwen3.6 reasoning mode used for rendering.
    pub qwen36_reasoning_mode: Qwen36ReasoningMode,
    /// Group size.
    pub group_size: usize,
    /// Reward scale.
    pub reward_scale: f32,
    /// Reference KL penalty for this smoke run.
    pub kl_reference_penalty: f32,
    /// Assistant-only loss.
    pub assistant_only_loss: bool,
    /// Empty think-block loss ignored.
    pub ignore_empty_think_loss: bool,
    /// Parent SFT adapter path.
    pub parent_sft_adapter_path: String,
    /// Parent SFT receipt path.
    pub parent_sft_receipt_path: String,
    /// Parent SFT receipt digest.
    pub parent_sft_receipt_digest: String,
    /// Parent SFT adapter digest.
    pub parent_sft_adapter_digest: String,
    /// Dataset reference.
    pub dataset_ref: String,
    /// Prompt group count.
    pub prompt_group_count: usize,
    /// Rollout count.
    pub rollout_count: usize,
    /// Bad completion count preserved in traces.
    pub bad_completion_count: usize,
    /// Reward trace count.
    pub reward_trace_count: usize,
    /// Eval suite id.
    pub eval_suite_id: String,
    /// Completed steps.
    pub completed_steps: u64,
    /// Initial file-write preference accuracy.
    pub initial_file_write_preference_accuracy: f32,
    /// Final file-write preference accuracy.
    pub final_file_write_preference_accuracy: f32,
    /// Initial best-minus-worst reward margin.
    pub initial_average_reward_margin: f32,
    /// Final best-minus-worst reward margin.
    pub final_average_reward_margin: f32,
    /// Whether final reward margin improved over the parent SFT adapter.
    pub reward_margin_improved: bool,
    /// Adapter path.
    pub adapter_artifact_path: String,
    /// Adapter digest.
    pub adapter_artifact_digest: String,
    /// Adapter identity digest.
    pub adapter_identity_digest: String,
    /// Loss curve path.
    pub loss_curve_path: String,
    /// Reward trace JSONL path.
    pub reward_trace_path: String,
    /// Checkpoint summary path.
    pub checkpoint_summary_path: String,
    /// No Python was invoked.
    pub python_invoked: bool,
    /// Current sampling backend.
    pub sampling_backend: String,
    /// Whether distributed sampling is part of the next production step.
    pub distributed_sampling_supported_later: bool,
    /// Plain limitation statement for the current smoke.
    pub claim_boundary: String,
    /// Receipt digest.
    pub receipt_digest: String,
}

impl PsionicLegalGrpoTrainingReceipt {
    /// Stable receipt digest with the digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(b"psionic_legal_grpo_training_receipt|", &clone)
    }
}

/// Paths and receipt returned after running the GRPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalGrpoRunArtifacts {
    /// Adapter path.
    pub adapter_artifact_path: String,
    /// Receipt path.
    pub receipt_path: String,
    /// Loss curve path.
    pub loss_curve_path: String,
    /// Reward trace path.
    pub reward_trace_path: String,
    /// Checkpoint summary path.
    pub checkpoint_summary_path: String,
    /// Receipt payload.
    pub receipt: PsionicLegalGrpoTrainingReceipt,
}

/// Error returned by the legal GRPO command.
#[derive(Debug, Error)]
pub enum PsionicLegalGrpoError {
    /// Invalid config.
    #[error("invalid legal GRPO config: {detail}")]
    InvalidConfig { detail: String },
    /// I/O failure.
    #[error("legal GRPO I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    /// JSON failure.
    #[error("legal GRPO JSON failed at `{path}`: {message}")]
    Json { path: String, message: String },
    /// Serialization failure.
    #[error("legal GRPO serialization failed: {message}")]
    Serialization { message: String },
    /// Open-adapter trainer failure.
    #[error("legal GRPO trainer failed: {0}")]
    Trainer(#[from] OpenAdapterTrainingExecutionError),
    /// Open-adapter export failure.
    #[error("legal GRPO export failed: {0}")]
    OpenAdapter(#[from] OpenAdapterSftError),
    /// Core trainer failure.
    #[error("legal GRPO core failed: {0}")]
    Core(#[from] TrainingCoreError),
    /// Parent SFT failure.
    #[error("legal GRPO parent SFT failed: {0}")]
    ParentSft(#[from] PsionicLegalSftError),
    /// Adapter load failure.
    #[error("legal GRPO adapter load failed: {0}")]
    AdapterLoad(#[from] LmHeadLoraLoadError),
    /// Qwen prompt rendering failure.
    #[error("legal GRPO Qwen3.6 prompt render failed: {0}")]
    Qwen36Template(#[from] Qwen36TemplateError),
}

/// Runs `psionic-train grpo --config <path>` args and returns the receipt.
pub fn run_psionic_legal_grpo_cli(
    args: &[String],
) -> Result<PsionicLegalGrpoTrainingReceipt, PsionicLegalGrpoError> {
    let config_path = parse_config_path(args)?;
    let artifacts = run_psionic_legal_grpo_config_path(config_path)?;
    Ok(artifacts.receipt)
}

/// Runs one GRPO config from a JSON file.
pub fn run_psionic_legal_grpo_config_path(
    path: impl AsRef<Path>,
) -> Result<PsionicLegalGrpoRunArtifacts, PsionicLegalGrpoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| PsionicLegalGrpoError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let config: PsionicLegalGrpoConfig =
        serde_json::from_slice(bytes.as_slice()).map_err(|error| PsionicLegalGrpoError::Json {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    run_psionic_legal_grpo_config(&config)
}

/// Runs one parsed GRPO config.
pub fn run_psionic_legal_grpo_config(
    config: &PsionicLegalGrpoConfig,
) -> Result<PsionicLegalGrpoRunArtifacts, PsionicLegalGrpoError> {
    config.validate()?;
    ensure_parent_sft(config)?;
    let output_dir = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_dir).map_err(|error| PsionicLegalGrpoError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let parent_receipt = read_parent_sft_receipt(config)?;
    let parent_receipt_digest = sha256_file(config.parent_sft_receipt_path.as_str())?;
    let parent_adapter_bytes =
        fs::read(&config.parent_sft_adapter_path).map_err(|error| PsionicLegalGrpoError::Io {
            path: config.parent_sft_adapter_path.clone(),
            message: error.to_string(),
        })?;
    let parent_adapter_digest = sha256_hex(parent_adapter_bytes.as_slice());
    let parent_adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        parent_adapter_bytes.as_slice(),
        parent_adapter_identity(config, &parent_receipt, parent_adapter_digest.as_str()),
        config.lora_alpha,
    )?;

    let mut features = grpo_rollout_features(config, &PsionicLegalWorkflowRewardPlugin::default())?;
    normalize_rollout_advantages(features.as_mut_slice());
    let reward_traces = features
        .iter()
        .map(|feature| feature.reward_trace.clone())
        .collect::<Vec<_>>();

    let backend = OpenAdapterTrainingExecutionBackend::new(
        OpenAdapterExecutionConfig {
            run_id: config.run_id.clone(),
            checkpoint_family: String::from(QWEN36_LEGAL_GRPO_CHECKPOINT_FAMILY),
            execution_backend_label: String::from(OPEN_ADAPTER_QWEN36_LEGAL_CUDA_BACKEND_LABEL),
            admissible_model_family: OpenAdapterAdmissibleModelFamily::Qwen36LegalDecoderLmHeadLora,
            budget: TrainingLoopBudget::new(config.max_steps, 1, 1)?,
            batch_size: config.batch_size,
            precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
            model: OpenAdapterReferenceModel {
                base_model_id: config.base_model.clone(),
                base_model_revision: config.base_model_revision.clone(),
                base_served_artifact_digest: config.base_served_artifact_digest.clone(),
                tokenizer: TokenizerDigest::new(
                    TokenizerFamily::BytePairEncoding,
                    config.tokenizer_digest.clone(),
                    config.vocab_size as u32,
                )
                .with_template_digest(config.prompt_template_digest.as_str()),
                hidden_size: config.hidden_size,
                vocab_size: config.vocab_size,
                target: OpenAdapterLmHeadTarget {
                    target_id: config.adapter_target_id.clone(),
                    lora_rank: config.lora_rank,
                    lora_alpha: config.lora_alpha,
                    optimizer: TrainingOptimizerConfig::adamw(
                        config.learning_rate,
                        0.9,
                        0.99,
                        1e-8,
                    ),
                    optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
                },
            },
        },
        features
            .iter()
            .map(|feature| {
                OpenAdapterHiddenStateSample::new(
                    feature.rollout_id.clone(),
                    feature.hidden_state.clone(),
                    feature.target_token_id,
                    feature.source_token_count,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    let mut run = backend.initialize_run_from_loaded_adapter(&parent_adapter)?;
    let initial_eval = evaluate_features(config, &parent_adapter, features.as_slice());
    let mut step_receipts = Vec::new();
    let mut weighted_records = Vec::new();
    let mut loss_points = Vec::new();
    for step_index in 0..config.max_steps {
        let batch = grpo_weighted_batch(config, features.as_slice(), step_index);
        let started_at_ms =
            config.started_at_ms + step_index.saturating_mul(config.step_duration_ms);
        let finished_at_ms = started_at_ms + config.step_duration_ms;
        let (step_input, weighted_record) = backend.produce_weighted_target_step_input(
            &run,
            &batch,
            started_at_ms,
            finished_at_ms,
        )?;
        let step_receipt = run.apply_step(step_input)?;
        let groups = backend.snapshot_training_groups(&run)?;
        let eval = evaluate_group_features(config, groups.as_slice(), features.as_slice())?;
        loss_points.push(PsionicLegalGrpoLossPoint {
            step: step_receipt.schedule.global_step,
            loss: weighted_record.mean_weighted_loss,
            raw_loss: weighted_record.mean_raw_loss,
            mean_loss_weight: weighted_record.mean_loss_weight,
            batch_id: weighted_record.batch_id.clone(),
            receipt_digest: step_receipt.receipt_digest.clone(),
            file_write_preference_accuracy: eval.file_write_preference_accuracy,
            average_reward_margin: eval.average_reward_margin,
        });
        step_receipts.push(step_receipt);
        weighted_records.push(weighted_record);
    }

    let exported = backend.export_run_artifact(
        &run,
        &OpenAdapterArtifactExportRequest::new(
            config.dataset_ref.clone(),
            config.validator_policy_ref.clone(),
            config.adapter_id.clone(),
            config.adapter_revision.clone(),
        ),
    )?;
    let adapter_path = output_dir.join("adapter.safetensors");
    fs::write(&adapter_path, exported.adapter_bytes.as_slice()).map_err(|error| {
        PsionicLegalGrpoError::Io {
            path: adapter_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let final_adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        exported.adapter_bytes.as_slice(),
        exported.adapter_identity.clone(),
        exported.adapter_alpha,
    )?;
    let final_eval = evaluate_features(config, &final_adapter, features.as_slice());
    let loss_curve = PsionicLegalGrpoLossCurve {
        schema_version: String::from(PSIONIC_LEGAL_GRPO_LOSS_CURVE_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        initial_eval: initial_eval.clone(),
        points: loss_points,
    };
    let loss_curve_path = output_dir.join("loss_curve.json");
    write_json(&loss_curve_path, &loss_curve)?;

    let reward_trace_path = output_dir.join("reward_traces.jsonl");
    write_jsonl(&reward_trace_path, reward_traces.as_slice())?;

    let checkpoint = checkpoint_summary(
        config,
        &run.summary(),
        parent_adapter_digest.as_str(),
        exported.adapter_artifact_digest.as_str(),
        exported.adapter_identity_digest.as_str(),
        step_receipts.as_slice(),
        weighted_records.as_slice(),
        reward_traces.as_slice(),
    );
    let checkpoint_path = output_dir.join("checkpoint_summary.json");
    write_json(&checkpoint_path, &checkpoint)?;

    let bad_completion_count = features
        .iter()
        .filter(|feature| feature.bad_completion)
        .count();
    let mut receipt = PsionicLegalGrpoTrainingReceipt {
        schema_version: String::from(PSIONIC_LEGAL_GRPO_RECEIPT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        trainer: String::from("psionic.open_adapter.qwen36_legal_lm_head_lora_grpo.v1"),
        train_type: config.train_type.clone(),
        base_model: config.base_model.clone(),
        served_model_id: config.served_model_id.clone(),
        base_model_revision: config.base_model_revision.clone(),
        base_artifact_mode: config.base_artifact_mode,
        base_served_artifact_digest: config.base_served_artifact_digest.clone(),
        tokenizer_digest: config.tokenizer_digest.clone(),
        prompt_template_digest: config.prompt_template_digest.clone(),
        qwen36_reasoning_mode: config.qwen36_reasoning_mode,
        group_size: config.group_size,
        reward_scale: config.reward_scale,
        kl_reference_penalty: config.kl_reference_penalty,
        assistant_only_loss: config.assistant_only_loss,
        ignore_empty_think_loss: config.ignore_empty_think_loss,
        parent_sft_adapter_path: config.parent_sft_adapter_path.clone(),
        parent_sft_receipt_path: config.parent_sft_receipt_path.clone(),
        parent_sft_receipt_digest: parent_receipt_digest,
        parent_sft_adapter_digest: parent_adapter_digest,
        dataset_ref: config.dataset_ref.clone(),
        prompt_group_count: config.prompt_groups.len(),
        rollout_count: features.len(),
        bad_completion_count,
        reward_trace_count: reward_traces.len(),
        eval_suite_id: config.eval_suite_id.clone(),
        completed_steps: run.summary().completed_steps,
        initial_file_write_preference_accuracy: initial_eval.file_write_preference_accuracy,
        final_file_write_preference_accuracy: final_eval.file_write_preference_accuracy,
        initial_average_reward_margin: initial_eval.average_reward_margin,
        final_average_reward_margin: final_eval.average_reward_margin,
        reward_margin_improved: final_eval.average_reward_margin
            > initial_eval.average_reward_margin,
        adapter_artifact_path: adapter_path.display().to_string(),
        adapter_artifact_digest: exported.adapter_artifact_digest.clone(),
        adapter_identity_digest: exported.adapter_identity_digest.clone(),
        loss_curve_path: loss_curve_path.display().to_string(),
        reward_trace_path: reward_trace_path.display().to_string(),
        checkpoint_summary_path: checkpoint_path.display().to_string(),
        python_invoked: false,
        sampling_backend: String::from("deterministic_local_group_sampler"),
        distributed_sampling_supported_later: true,
        claim_boundary: String::from(
            "This is a Rust-only adapter GRPO smoke over deterministic local completion groups and synthetic Qwen3.6 hidden states. It proves group sampling, verifier-style reward traces, group-normalized reward updates, bad-completion preservation, checkpoint receipts, and adapter export. It does not claim full dense Qwen3.6 RL, distributed Pylon sampling, or improvement on private Harvey benchmark tasks.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    let receipt_path = output_dir.join("training_receipt.json");
    write_json(&receipt_path, &receipt)?;

    Ok(PsionicLegalGrpoRunArtifacts {
        adapter_artifact_path: adapter_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        loss_curve_path: loss_curve_path.display().to_string(),
        reward_trace_path: reward_trace_path.display().to_string(),
        checkpoint_summary_path: checkpoint_path.display().to_string(),
        receipt,
    })
}

fn ensure_parent_sft(config: &PsionicLegalGrpoConfig) -> Result<(), PsionicLegalGrpoError> {
    if Path::new(&config.parent_sft_adapter_path).is_file()
        && Path::new(&config.parent_sft_receipt_path).is_file()
    {
        return Ok(());
    }
    if !config.bootstrap_parent_sft_if_missing {
        return invalid_config(
            "parent SFT adapter or receipt is missing and bootstrap_parent_sft_if_missing is false",
        );
    }
    let Some(path) = config.parent_sft_config_path.as_deref() else {
        return invalid_config("parent_sft_config_path is required when bootstrapping parent SFT");
    };
    run_psionic_legal_sft_config_path(path)?;
    if !Path::new(&config.parent_sft_adapter_path).is_file()
        || !Path::new(&config.parent_sft_receipt_path).is_file()
    {
        return invalid_config("parent SFT bootstrap completed but expected artifacts are missing");
    }
    Ok(())
}

fn grpo_rollout_features(
    config: &PsionicLegalGrpoConfig,
    reward_fn: &impl PsionicLegalGrpoRewardFunction,
) -> Result<Vec<PsionicLegalGrpoRolloutFeature>, PsionicLegalGrpoError> {
    let renderer = Qwen36PromptRenderer::without_tokenizer();
    let options = Qwen36PromptOptions {
        reasoning_mode: config.qwen36_reasoning_mode,
        add_generation_prompt: true,
        emit_empty_think_block: config.emit_empty_think_block,
    };
    let mut features = Vec::new();
    for prompt_group in &config.prompt_groups {
        let messages = vec![
            PromptMessage::new(
                PromptMessageRole::System,
                prompt_group.system_prompt.clone(),
            ),
            PromptMessage::new(PromptMessageRole::User, prompt_group.user_prompt.clone()),
        ];
        let rendered = renderer.render(&messages, &options)?;
        let prompt_receipt = Qwen36PromptReceipt::from(&rendered);
        let hidden_state =
            hidden_state_from_digest(rendered.prompt_hash.as_str(), config.hidden_size);
        let max_reward = prompt_group
            .sampled_completions
            .iter()
            .map(|completion| reward_fn.score(config.run_id.as_str(), prompt_group, completion))
            .map(|trace| trace.total_reward)
            .fold(f32::NEG_INFINITY, f32::max);
        for completion in &prompt_group.sampled_completions {
            let reward_trace = reward_fn.score(config.run_id.as_str(), prompt_group, completion);
            let rollout_id = format!("{}.{}", prompt_group.prompt_id, completion.completion_id);
            features.push(PsionicLegalGrpoRolloutFeature {
                prompt_id: prompt_group.prompt_id.clone(),
                rollout_id: rollout_id.clone(),
                prompt_receipt: prompt_receipt.clone(),
                hidden_state: hidden_state.clone(),
                target_token_id: token_id_from_text(
                    "grpo_completion",
                    completion.completion_text.as_str(),
                    config.vocab_size,
                ),
                source_token_count: approximate_token_count(
                    format!("{}\n{}", rendered.text, completion.completion_text).as_str(),
                ),
                total_reward: reward_trace.total_reward,
                normalized_advantage: 0.0,
                bad_completion: reward_trace.total_reward < max_reward,
                reward_trace,
            });
        }
    }
    ensure_unique_targets(config, features.as_mut_slice());
    Ok(features)
}

fn normalize_rollout_advantages(features: &mut [PsionicLegalGrpoRolloutFeature]) {
    let mut by_prompt = BTreeMap::<String, Vec<usize>>::new();
    for (index, feature) in features.iter().enumerate() {
        by_prompt
            .entry(feature.prompt_id.clone())
            .or_default()
            .push(index);
    }
    for indexes in by_prompt.values() {
        let mean = indexes
            .iter()
            .map(|index| features[*index].total_reward)
            .sum::<f32>()
            / indexes.len().max(1) as f32;
        let variance = indexes
            .iter()
            .map(|index| {
                let delta = features[*index].total_reward - mean;
                delta * delta
            })
            .sum::<f32>()
            / indexes.len().max(1) as f32;
        let stddev = variance.sqrt().max(1e-6);
        for index in indexes {
            let advantage = (features[*index].total_reward - mean) / stddev;
            features[*index].normalized_advantage = advantage;
            features[*index].reward_trace.normalized_advantage = advantage;
            features[*index].reward_trace.trace_digest =
                features[*index].reward_trace.stable_digest();
        }
    }
}

fn grpo_weighted_batch(
    config: &PsionicLegalGrpoConfig,
    features: &[PsionicLegalGrpoRolloutFeature],
    step_index: u64,
) -> OpenAdapterWeightedTargetBatchRequest {
    let start = (step_index as usize * config.batch_size) % features.len();
    let mut targets = Vec::with_capacity(config.batch_size);
    for offset in 0..config.batch_size {
        let feature = &features[(start + offset) % features.len()];
        targets.push(
            OpenAdapterWeightedTokenTarget::new(
                feature.rollout_id.clone(),
                feature.hidden_state.clone(),
                feature.target_token_id,
                feature.normalized_advantage * config.reward_scale,
            )
            .with_teacher_target_logprob(-feature.total_reward.abs()),
        );
    }
    OpenAdapterWeightedTargetBatchRequest::new(
        format!("legal-grpo-batch-{}", step_index + 1),
        targets,
        config.teacher_target_blend,
    )
}

fn evaluate_features(
    config: &PsionicLegalGrpoConfig,
    adapter: &LmHeadLoraAdapterArtifact,
    features: &[PsionicLegalGrpoRolloutFeature],
) -> PsionicLegalGrpoEvalPoint {
    evaluate_lora_values(config, adapter.lora_a(), adapter.lora_b(), features)
}

fn evaluate_group_features(
    config: &PsionicLegalGrpoConfig,
    groups: &[TrainingParameterGroupState],
    features: &[PsionicLegalGrpoRolloutFeature],
) -> Result<PsionicLegalGrpoEvalPoint, PsionicLegalGrpoError> {
    if groups.len() != 2 {
        return invalid_config("expected two LoRA parameter groups");
    }
    let lora_a = group_values(&groups[0])?;
    let lora_b = group_values(&groups[1])?;
    Ok(evaluate_lora_values(config, lora_a, lora_b, features))
}

fn evaluate_lora_values(
    config: &PsionicLegalGrpoConfig,
    lora_a: &[f32],
    lora_b: &[f32],
    features: &[PsionicLegalGrpoRolloutFeature],
) -> PsionicLegalGrpoEvalPoint {
    let base_projection = seeded_matrix(
        format!(
            "{}|{}|base_projection|{}x{}",
            config.base_model, config.base_model_revision, config.vocab_size, config.hidden_size
        )
        .as_str(),
        config.vocab_size,
        config.hidden_size,
        0.04,
    );
    let mut by_prompt = BTreeMap::<String, Vec<&PsionicLegalGrpoRolloutFeature>>::new();
    for feature in features {
        by_prompt
            .entry(feature.prompt_id.clone())
            .or_default()
            .push(feature);
    }
    let mut correct = 0usize;
    let mut total_margin = 0.0_f32;
    for group in by_prompt.values() {
        let Some(best) = group.iter().max_by(|left, right| {
            left.total_reward
                .partial_cmp(&right.total_reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            continue;
        };
        let Some(worst) = group.iter().min_by(|left, right| {
            left.total_reward
                .partial_cmp(&right.total_reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            continue;
        };
        let best_logprob = logprob_for_target(
            config,
            base_projection.as_slice(),
            lora_a,
            lora_b,
            best.hidden_state.as_slice(),
            best.target_token_id,
        );
        let worst_logprob = logprob_for_target(
            config,
            base_projection.as_slice(),
            lora_a,
            lora_b,
            worst.hidden_state.as_slice(),
            worst.target_token_id,
        );
        let margin = best_logprob - worst_logprob;
        if margin > 0.0 {
            correct += 1;
        }
        total_margin += margin;
    }
    let group_count = by_prompt.len();
    let denominator = group_count.max(1) as f32;
    PsionicLegalGrpoEvalPoint {
        group_count,
        file_write_preference_accuracy: correct as f32 / denominator,
        average_reward_margin: total_margin / denominator,
    }
}

fn logprob_for_target(
    config: &PsionicLegalGrpoConfig,
    base_projection: &[f32],
    lora_a: &[f32],
    lora_b: &[f32],
    hidden: &[f32],
    target_token_id: u32,
) -> f32 {
    let mut logits = mat_vec(
        base_projection,
        config.vocab_size,
        config.hidden_size,
        hidden,
    );
    let intermediate = mat_vec(lora_a, config.lora_rank, config.hidden_size, hidden);
    let adapter_logits = mat_vec(
        lora_b,
        config.vocab_size,
        config.lora_rank,
        intermediate.as_slice(),
    );
    add_scaled(
        logits.as_mut_slice(),
        adapter_logits.as_slice(),
        config.lora_alpha / config.lora_rank.max(1) as f32,
    );
    let distribution = softmax(logits.as_slice());
    distribution[target_token_id as usize]
        .max(f32::EPSILON)
        .ln()
}

fn checkpoint_summary(
    config: &PsionicLegalGrpoConfig,
    run_summary: &TrainingRunSummary,
    parent_adapter_digest: &str,
    final_adapter_digest: &str,
    final_adapter_identity_digest: &str,
    step_receipts: &[TrainingStepReceipt],
    weighted_records: &[OpenAdapterWeightedTargetBatchRecord],
    reward_traces: &[PsionicLegalGrpoRewardTrace],
) -> PsionicLegalGrpoCheckpointSummary {
    let mut checkpoint = PsionicLegalGrpoCheckpointSummary {
        schema_version: String::from(PSIONIC_LEGAL_GRPO_CHECKPOINT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        checkpoint_family: String::from(QWEN36_LEGAL_GRPO_CHECKPOINT_FAMILY),
        run_summary: run_summary.clone(),
        parent_adapter_digest: parent_adapter_digest.to_string(),
        final_adapter_digest: final_adapter_digest.to_string(),
        final_adapter_identity_digest: final_adapter_identity_digest.to_string(),
        step_receipt_digests: step_receipts
            .iter()
            .map(|receipt| receipt.receipt_digest.clone())
            .collect(),
        weighted_batch_digests: weighted_records
            .iter()
            .map(|record| record.execution_digest.clone())
            .collect(),
        reward_trace_digests: reward_traces
            .iter()
            .map(|trace| trace.trace_digest.clone())
            .collect(),
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest = checkpoint.stable_digest();
    checkpoint
}

fn parent_adapter_identity(
    config: &PsionicLegalGrpoConfig,
    receipt: &PsionicTrainingReceipt,
    parent_adapter_digest: &str,
) -> AdapterArtifactIdentity {
    AdapterArtifactIdentity::new(
        config.parent_sft_adapter_id.clone(),
        config.parent_sft_adapter_revision.clone(),
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        config.base_model.clone(),
        config.base_model_revision.clone(),
        config.base_served_artifact_digest.clone(),
        parent_adapter_digest.to_string(),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        u64::try_from(
            config
                .lora_rank
                .saturating_mul(config.hidden_size + config.vocab_size),
        )
        .unwrap_or(u64::MAX),
    )
    .with_provenance_digest(receipt.receipt_digest.clone())
}

fn read_parent_sft_receipt(
    config: &PsionicLegalGrpoConfig,
) -> Result<PsionicTrainingReceipt, PsionicLegalGrpoError> {
    let bytes =
        fs::read(&config.parent_sft_receipt_path).map_err(|error| PsionicLegalGrpoError::Io {
            path: config.parent_sft_receipt_path.clone(),
            message: error.to_string(),
        })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| PsionicLegalGrpoError::Json {
        path: config.parent_sft_receipt_path.clone(),
        message: error.to_string(),
    })
}

fn group_values(group: &TrainingParameterGroupState) -> Result<&[f32], PsionicLegalGrpoError> {
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        _ => invalid_config("GRPO smoke expected f32 LoRA parameter groups"),
    }
}

fn validate_prompt_group(
    prompt_group: &PsionicLegalGrpoPromptGroup,
    group_size: usize,
) -> Result<(), PsionicLegalGrpoError> {
    require_nonempty(prompt_group.prompt_id.as_str(), "prompt_id")?;
    require_nonempty(prompt_group.task_id.as_str(), "task_id")?;
    require_nonempty(prompt_group.task_version.as_str(), "task_version")?;
    require_nonempty(
        prompt_group.required_answer_path.as_str(),
        "required_answer_path",
    )?;
    require_nonempty(prompt_group.system_prompt.as_str(), "system_prompt")?;
    require_nonempty(prompt_group.user_prompt.as_str(), "user_prompt")?;
    if prompt_group.sampled_completions.len() != group_size {
        return invalid_config(format!(
            "prompt_group `{}` must contain exactly group_size completions",
            prompt_group.prompt_id
        ));
    }
    let mut saw_good = false;
    let mut saw_bad = false;
    for completion in &prompt_group.sampled_completions {
        require_nonempty(completion.completion_id.as_str(), "completion_id")?;
        require_nonempty(completion.completion_text.as_str(), "completion_text")?;
        if completion.hidden_leakage {
            return invalid_config(
                "GRPO smoke completions must not use hidden/private scoring data",
            );
        }
        let correct_path = completion
            .output_path
            .as_deref()
            .is_some_and(|path| path == prompt_group.required_answer_path);
        let good = completion.wrote_required_file
            && correct_path
            && completion.answer_byte_size > 0
            && completion.submitted
            && completion.integrity_valid
            && (!prompt_group.source_required || completion.source_used);
        saw_good |= good;
        saw_bad |= !good;
    }
    if !saw_good || !saw_bad {
        return invalid_config(format!(
            "prompt_group `{}` must contain at least one good and one bad completion",
            prompt_group.prompt_id
        ));
    }
    Ok(())
}

fn ensure_unique_targets(
    config: &PsionicLegalGrpoConfig,
    features: &mut [PsionicLegalGrpoRolloutFeature],
) {
    let mut seen = BTreeMap::<u32, usize>::new();
    for feature in features {
        let count = seen.entry(feature.target_token_id).or_default();
        if *count > 0 {
            feature.target_token_id = (feature.target_token_id
                + u32::try_from(*count).unwrap_or(1))
                % u32::try_from(config.vocab_size).unwrap_or(u32::MAX);
        }
        *count += 1;
    }
}

fn hidden_state_from_digest(seed: &str, hidden_size: usize) -> Vec<f32> {
    (0..hidden_size)
        .map(|index| {
            let digest = Sha256::digest(format!("{seed}|hidden|{index}").as_bytes());
            let raw = u16::from_le_bytes([digest[0], digest[1]]) as f32 / u16::MAX as f32;
            ((raw * 2.0) - 1.0).clamp(-1.0, 1.0)
        })
        .collect()
}

fn token_id_from_text(prefix: &str, text: &str, vocab_size: usize) -> u32 {
    let digest = Sha256::digest(format!("{prefix}|{text}").as_bytes());
    let value = u64::from_le_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (value % vocab_size.max(1) as u64) as u32
}

fn approximate_token_count(text: &str) -> u32 {
    u32::try_from((text.len() / 4).max(1)).unwrap_or(u32::MAX)
}

fn mat_vec(matrix: &[f32], rows: usize, cols: usize, vector: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0_f32; rows];
    for row in 0..rows {
        let mut total = 0.0_f32;
        for col in 0..cols {
            total += matrix[row * cols + col] * vector[col];
        }
        out[row] = total;
    }
    out
}

fn add_scaled(dst: &mut [f32], src: &[f32], scale: f32) {
    for (left, right) in dst.iter_mut().zip(src) {
        *left += right * scale;
    }
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp = logits
        .iter()
        .map(|value| (*value - max).exp())
        .collect::<Vec<_>>();
    let sum = exp.iter().sum::<f32>().max(f32::EPSILON);
    exp.into_iter().map(|value| value / sum).collect()
}

fn seeded_matrix(seed: &str, rows: usize, cols: usize, scale: f32) -> Vec<f32> {
    (0..rows * cols)
        .map(|index| {
            let digest = Sha256::digest(format!("{seed}|{index}").as_bytes());
            let raw = u16::from_le_bytes([digest[0], digest[1]]) as f32 / u16::MAX as f32;
            ((raw * 2.0) - 1.0) * scale
        })
        .collect()
}

fn parse_config_path(args: &[String]) -> Result<&Path, PsionicLegalGrpoError> {
    let index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(path) = args.get(index + 1) else {
                    return invalid_config("--config requires a path");
                };
                return Ok(Path::new(path));
            }
            "--help" | "-h" => {
                return invalid_config("usage: psionic-train grpo --config <path>");
            }
            other => {
                return invalid_config(format!("unsupported grpo argument `{other}`"));
            }
        }
    }
    invalid_config("missing --config <path>")
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), PsionicLegalGrpoError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| PsionicLegalGrpoError::Serialization {
            message: error.to_string(),
        })?;
    fs::write(path, bytes).map_err(|error| PsionicLegalGrpoError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_jsonl(
    path: &Path,
    traces: &[PsionicLegalGrpoRewardTrace],
) -> Result<(), PsionicLegalGrpoError> {
    let mut jsonl = String::new();
    for trace in traces {
        let line =
            serde_json::to_string(trace).map_err(|error| PsionicLegalGrpoError::Serialization {
                message: error.to_string(),
            })?;
        jsonl.push_str(line.as_str());
        jsonl.push('\n');
    }
    fs::write(path, jsonl.as_bytes()).map_err(|error| PsionicLegalGrpoError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn require_nonempty(value: &str, field: &str) -> Result<(), PsionicLegalGrpoError> {
    if value.trim().is_empty() {
        return invalid_config(format!("{field} must be present"));
    }
    Ok(())
}

fn invalid_config<T>(detail: impl Into<String>) -> Result<T, PsionicLegalGrpoError> {
    Err(PsionicLegalGrpoError::InvalidConfig {
        detail: detail.into(),
    })
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sha256_file(path: &str) -> Result<String, PsionicLegalGrpoError> {
    let bytes = fs::read(path).map_err(|error| PsionicLegalGrpoError::Io {
        path: path.to_string(),
        message: error.to_string(),
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn bool_reward(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}

fn default_tokenizer_digest() -> String {
    String::from(DEFAULT_QWEN36_TOKENIZER_DIGEST)
}

fn default_template_digest() -> String {
    String::from(DEFAULT_QWEN36_TEMPLATE_DIGEST)
}

fn default_adapter_target_id() -> String {
    String::from("lm_head")
}

/// Returns the default synthetic Qwen3.6 legal GRPO smoke config.
#[must_use]
pub fn default_qwen36_legal_grpo_smoke_config(
    output_dir: impl Into<String>,
) -> PsionicLegalGrpoConfig {
    PsionicLegalGrpoConfig {
        schema_version: String::from(PSIONIC_LEGAL_GRPO_CONFIG_SCHEMA_VERSION),
        run_id: String::from("qwen36-legal-grpo-smoke"),
        train_type: String::from("grpo"),
        base_model: String::from("Qwen/Qwen3.6-27B"),
        served_model_id: String::from("qwen3.6-27b"),
        base_model_revision: String::from("qwen3.6-27b-smoke-revision"),
        base_artifact_mode: PsionicLegalSftBaseArtifactMode::SyntheticHiddenStateSmoke,
        base_served_artifact_digest: String::from(DEFAULT_QWEN36_BASE_DIGEST),
        tokenizer_digest: default_tokenizer_digest(),
        prompt_template_digest: default_template_digest(),
        hidden_size: 4,
        vocab_size: 256,
        adapter_target_id: default_adapter_target_id(),
        lora_rank: 16,
        lora_alpha: 32.0,
        learning_rate: 0.06,
        max_steps: 8,
        batch_size: 6,
        group_size: 3,
        reward_scale: 0.35,
        kl_reference_penalty: 0.0,
        teacher_target_blend: 0.0,
        assistant_only_loss: true,
        ignore_empty_think_loss: true,
        qwen36_reasoning_mode: Qwen36ReasoningMode::DirectAnswer,
        emit_empty_think_block: false,
        parent_sft_adapter_path: String::from("target/legal/qwen36_sft_smoke/adapter.safetensors"),
        parent_sft_receipt_path: String::from(
            "target/legal/qwen36_sft_smoke/training_receipt.json",
        ),
        parent_sft_adapter_id: String::from("qwen36-27b-legal-smoke"),
        parent_sft_adapter_revision: String::from("r1"),
        parent_sft_config_path: Some(String::from("configs/legal/qwen36_sft_smoke.json")),
        bootstrap_parent_sft_if_missing: true,
        prompt_groups: default_grpo_prompt_groups(),
        dataset_ref: String::from("dataset://openagents/legal-benchmark/legal-grpo-v1@smoke"),
        validator_policy_ref: String::from("policy://validator/legal-benchmark/qwen36-grpo-smoke"),
        eval_suite_id: String::from("harvey_public_three_deterministic_replay_v1"),
        adapter_id: String::from("qwen36-27b-legal-grpo-smoke"),
        adapter_revision: String::from("r1"),
        output_dir: output_dir.into(),
        started_at_ms: 3_000,
        step_duration_ms: 20,
    }
}

fn default_grpo_prompt_groups() -> Vec<PsionicLegalGrpoPromptGroup> {
    vec![
        PsionicLegalGrpoPromptGroup {
            prompt_id: String::from("mfn.file_discipline"),
            task_id: String::from("harvey.funds-asset-management.analyze_mfn_waterfall"),
            task_version: String::from("public-smoke"),
            required_answer_path: String::from("memo.md"),
            source_required: true,
            system_prompt: String::from(
                "You are a legal benchmark agent. Use sources and write the required file.",
            ),
            user_prompt: String::from(
                "Review the provided MFN materials and write memo.md. Submit only after the file is written.",
            ),
            sampled_completions: vec![
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("writes_sources_submit"),
                    completion_text: String::from(
                        "Read the documents, write memo.md with source-grounded analysis, validate the file, and submit.",
                    ),
                    wrote_required_file: true,
                    output_path: Some(String::from("memo.md")),
                    answer_byte_size: 2_800,
                    source_used: true,
                    submitted: true,
                    integrity_valid: true,
                    hidden_leakage: false,
                    public_score_delta_bps: 1000,
                },
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("chat_only"),
                    completion_text: String::from(
                        "Answer in chat and explain that no separate file is necessary.",
                    ),
                    wrote_required_file: false,
                    output_path: None,
                    answer_byte_size: 0,
                    source_used: true,
                    submitted: false,
                    integrity_valid: false,
                    hidden_leakage: false,
                    public_score_delta_bps: -1500,
                },
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("wrong_path"),
                    completion_text: String::from(
                        "Write the analysis to notes.md and submit without checking the required path.",
                    ),
                    wrote_required_file: true,
                    output_path: Some(String::from("notes.md")),
                    answer_byte_size: 1_100,
                    source_used: true,
                    submitted: true,
                    integrity_valid: true,
                    hidden_leakage: false,
                    public_score_delta_bps: -500,
                },
            ],
        },
        PsionicLegalGrpoPromptGroup {
            prompt_id: String::from("lease.source_use"),
            task_id: String::from("harvey.real-estate.summarize_lease_risks"),
            task_version: String::from("public-smoke"),
            required_answer_path: String::from("answer.md"),
            source_required: true,
            system_prompt: String::from(
                "You are a legal benchmark agent. Do the work in files, not only chat.",
            ),
            user_prompt: String::from(
                "Read the lease packet, identify legal risks, write answer.md, and submit.",
            ),
            sampled_completions: vec![
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("source_grounded_file"),
                    completion_text: String::from(
                        "Inspect the lease, write answer.md with risk headings and citations, validate it, then submit.",
                    ),
                    wrote_required_file: true,
                    output_path: Some(String::from("answer.md")),
                    answer_byte_size: 3_400,
                    source_used: true,
                    submitted: true,
                    integrity_valid: true,
                    hidden_leakage: false,
                    public_score_delta_bps: 1200,
                },
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("no_source_guess"),
                    completion_text: String::from(
                        "Write answer.md from memory without opening the lease packet.",
                    ),
                    wrote_required_file: true,
                    output_path: Some(String::from("answer.md")),
                    answer_byte_size: 1_600,
                    source_used: false,
                    submitted: true,
                    integrity_valid: true,
                    hidden_leakage: false,
                    public_score_delta_bps: -900,
                },
                PsionicLegalGrpoCompletionSample {
                    completion_id: String::from("empty_file"),
                    completion_text: String::from(
                        "Create answer.md as an empty placeholder and submit.",
                    ),
                    wrote_required_file: true,
                    output_path: Some(String::from("answer.md")),
                    answer_byte_size: 0,
                    source_used: false,
                    submitted: true,
                    integrity_valid: false,
                    hidden_leakage: false,
                    public_score_delta_bps: -1200,
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_qwen36_grpo_smoke_improves_file_write_reward_margin()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let sft_dir = temp.path().join("sft");
        let grpo_dir = temp.path().join("grpo");
        let mut config = default_qwen36_legal_grpo_smoke_config(grpo_dir.display().to_string());
        config.parent_sft_adapter_path = sft_dir.join("adapter.safetensors").display().to_string();
        config.parent_sft_receipt_path =
            sft_dir.join("training_receipt.json").display().to_string();
        config.parent_sft_config_path = None;
        config.bootstrap_parent_sft_if_missing = false;

        let sft_config =
            crate::default_qwen36_legal_sft_smoke_config(sft_dir.display().to_string());
        crate::run_psionic_legal_sft_config(&sft_config)?;
        let artifacts = run_psionic_legal_grpo_config(&config)?;

        assert!(Path::new(&artifacts.adapter_artifact_path).is_file());
        assert!(Path::new(&artifacts.receipt_path).is_file());
        assert!(Path::new(&artifacts.loss_curve_path).is_file());
        assert!(Path::new(&artifacts.reward_trace_path).is_file());
        assert!(Path::new(&artifacts.checkpoint_summary_path).is_file());
        assert!(!artifacts.receipt.python_invoked);
        assert_eq!(artifacts.receipt.prompt_group_count, 2);
        assert_eq!(artifacts.receipt.reward_trace_count, 6);
        assert!(artifacts.receipt.bad_completion_count > 0);
        assert!(artifacts.receipt.reward_margin_improved);
        assert!(
            artifacts.receipt.final_average_reward_margin
                > artifacts.receipt.initial_average_reward_margin
        );
        assert_eq!(
            artifacts.receipt.receipt_digest,
            artifacts.receipt.stable_digest()
        );
        Ok(())
    }

    #[test]
    fn legal_qwen36_grpo_reward_plugin_prefers_valid_file_output() {
        let config = default_qwen36_legal_grpo_smoke_config("target/tmp");
        let prompt = &config.prompt_groups[0];
        let reward = PsionicLegalWorkflowRewardPlugin::default();
        let good = reward.score(
            config.run_id.as_str(),
            prompt,
            &prompt.sampled_completions[0],
        );
        let bad = reward.score(
            config.run_id.as_str(),
            prompt,
            &prompt.sampled_completions[1],
        );

        assert!(good.total_reward > bad.total_reward);
        assert_eq!(good.components.wrote_required_file, 1.0);
        assert_eq!(bad.components.wrote_required_file, 0.0);
    }
}
