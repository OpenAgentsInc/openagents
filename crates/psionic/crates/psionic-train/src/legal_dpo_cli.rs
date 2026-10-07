use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
};
use psionic_core::{QuantizationMode, TensorData};
use psionic_data::{
    LEGAL_DPO_DATASET_SCHEMA_VERSION, LegalDpoMessage, LegalDpoPreferencePair, TokenizerDigest,
    TokenizerFamily, load_legal_dpo_dataset,
};
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

/// Config schema accepted by `psionic-train dpo --config`.
pub const PSIONIC_LEGAL_DPO_CONFIG_SCHEMA_VERSION: &str = "psionic.legal_dpo_config.v1";
/// Receipt schema emitted by `psionic-train dpo`.
pub const PSIONIC_LEGAL_DPO_RECEIPT_SCHEMA_VERSION: &str = "psionic.legal_dpo_training_receipt.v1";
/// Checkpoint summary schema emitted by `psionic-train dpo`.
pub const PSIONIC_LEGAL_DPO_CHECKPOINT_SCHEMA_VERSION: &str =
    "psionic.legal_dpo_checkpoint_summary.v1";
/// Loss/eval curve schema emitted by `psionic-train dpo`.
pub const PSIONIC_LEGAL_DPO_LOSS_CURVE_SCHEMA_VERSION: &str = "psionic.legal_dpo_loss_curve.v1";

const QWEN36_LEGAL_DPO_CHECKPOINT_FAMILY: &str = "psionic.qwen36.legal_adapter_dpo";
const DEFAULT_QWEN36_BASE_DIGEST: &str = "sha256:synthetic-qwen36-legal-smoke";
const DEFAULT_QWEN36_TEMPLATE_DIGEST: &str = "sha256:qwen36-chat-template-v1-smoke";
const DEFAULT_QWEN36_TOKENIZER_DIGEST: &str = "sha256:qwen36-tokenizer-smoke";

/// Config for one Rust-only legal DPO run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoConfig {
    /// Schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Train family, expected to be `dpo`.
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
    /// Pair batch size.
    pub batch_size: usize,
    /// DPO beta parameter.
    pub beta: f32,
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
    /// DPO JSONL dataset path.
    pub dataset_path: String,
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

impl PsionicLegalDpoConfig {
    fn validate(&self) -> Result<(), PsionicLegalDpoError> {
        if self.schema_version != PSIONIC_LEGAL_DPO_CONFIG_SCHEMA_VERSION {
            return invalid_config("schema_version is not psionic.legal_dpo_config.v1");
        }
        require_nonempty(self.run_id.as_str(), "run_id")?;
        if self.train_type != "dpo" {
            return invalid_config("train_type must be `dpo`");
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
        if !self.beta.is_finite() || self.beta <= 0.0 {
            return invalid_config("beta must be positive and finite");
        }
        if !self.assistant_only_loss {
            return invalid_config("DPO smoke requires assistant_only_loss = true");
        }
        if !self.ignore_empty_think_loss {
            return invalid_config("DPO smoke requires ignore_empty_think_loss = true");
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
        require_nonempty(self.dataset_path.as_str(), "dataset_path")?;
        require_nonempty(self.dataset_ref.as_str(), "dataset_ref")?;
        require_nonempty(self.validator_policy_ref.as_str(), "validator_policy_ref")?;
        require_nonempty(self.eval_suite_id.as_str(), "eval_suite_id")?;
        require_nonempty(self.adapter_id.as_str(), "adapter_id")?;
        require_nonempty(self.adapter_revision.as_str(), "adapter_revision")?;
        require_nonempty(self.output_dir.as_str(), "output_dir")?;
        Ok(())
    }
}

/// One feature derived from a prompt/chosen/rejected DPO pair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoPairFeature {
    /// Pair id from the DPO JSONL.
    pub pair_id: String,
    /// Pair family parsed from the pair id.
    pub pair_family: String,
    /// Normalized failure class.
    pub reason: String,
    /// Qwen3.6 prompt receipt.
    pub prompt_receipt: Qwen36PromptReceipt,
    /// Synthetic hidden state for the prompt.
    pub hidden_state: Vec<f32>,
    /// Token id representing the chosen completion.
    pub chosen_target_token_id: u32,
    /// Token id representing the rejected completion.
    pub rejected_target_token_id: u32,
    /// Approximate prompt token count when no tokenizer file is bound.
    pub source_token_count: u32,
}

/// One loss/eval row emitted by the DPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoLossPoint {
    /// Global step.
    pub step: u64,
    /// Mean weighted DPO loss from the weighted-target batch.
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
    pub preference_accuracy: f32,
    /// Average chosen-minus-rejected logprob margin after the step.
    pub average_preference_margin: f32,
}

/// Loss and eval curve artifact emitted by the DPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoLossCurve {
    /// Schema version.
    pub schema_version: String,
    /// Run id.
    pub run_id: String,
    /// Initial eval before DPO updates.
    pub initial_eval: PsionicLegalDpoEvalPoint,
    /// Per-step loss/eval points.
    pub points: Vec<PsionicLegalDpoLossPoint>,
}

/// Preference eval point over the DPO pairs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoEvalPoint {
    /// Pair count.
    pub pair_count: usize,
    /// Fraction of pairs where chosen logprob exceeds rejected logprob.
    pub preference_accuracy: f32,
    /// Average chosen-minus-rejected logprob margin.
    pub average_preference_margin: f32,
}

/// Deterministic checkpoint summary emitted by the DPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoCheckpointSummary {
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
    /// Weighted DPO batch digests.
    pub weighted_batch_digests: Vec<String>,
    /// Stable checkpoint digest.
    pub checkpoint_digest: String,
}

impl PsionicLegalDpoCheckpointSummary {
    fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.checkpoint_digest.clear();
        stable_json_digest(b"psionic_legal_dpo_checkpoint|", &clone)
    }
}

/// Machine-readable receipt emitted by one `psionic-train dpo` run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoTrainingReceipt {
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
    /// DPO beta parameter.
    pub beta: f32,
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
    /// DPO dataset path.
    pub dataset_path: String,
    /// DPO dataset digest.
    pub dataset_digest: String,
    /// DPO pair count.
    pub pair_count: usize,
    /// Eval suite id.
    pub eval_suite_id: String,
    /// Completed steps.
    pub completed_steps: u64,
    /// Initial preference accuracy.
    pub initial_preference_accuracy: f32,
    /// Final preference accuracy.
    pub final_preference_accuracy: f32,
    /// Initial average preference margin.
    pub initial_average_preference_margin: f32,
    /// Final average preference margin.
    pub final_average_preference_margin: f32,
    /// Whether final margin improved over the parent SFT adapter.
    pub preference_margin_improved: bool,
    /// Adapter path.
    pub adapter_artifact_path: String,
    /// Adapter digest.
    pub adapter_artifact_digest: String,
    /// Adapter identity digest.
    pub adapter_identity_digest: String,
    /// Loss curve path.
    pub loss_curve_path: String,
    /// Checkpoint summary path.
    pub checkpoint_summary_path: String,
    /// No Python was invoked.
    pub python_invoked: bool,
    /// Plain limitation statement for the current smoke.
    pub claim_boundary: String,
    /// Receipt digest.
    pub receipt_digest: String,
}

impl PsionicLegalDpoTrainingReceipt {
    /// Stable receipt digest with the digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(b"psionic_legal_dpo_training_receipt|", &clone)
    }
}

/// Paths and receipt returned after running the DPO command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionicLegalDpoRunArtifacts {
    /// Adapter path.
    pub adapter_artifact_path: String,
    /// Receipt path.
    pub receipt_path: String,
    /// Loss curve path.
    pub loss_curve_path: String,
    /// Checkpoint summary path.
    pub checkpoint_summary_path: String,
    /// Receipt payload.
    pub receipt: PsionicLegalDpoTrainingReceipt,
}

/// Error returned by the legal DPO command.
#[derive(Debug, Error)]
pub enum PsionicLegalDpoError {
    #[error("invalid legal DPO config: {detail}")]
    InvalidConfig { detail: String },
    #[error("legal DPO I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("legal DPO JSON failed at `{path}`: {message}")]
    Json { path: String, message: String },
    #[error("legal DPO serialization failed: {message}")]
    Serialization { message: String },
    #[error("legal DPO trainer failed: {0}")]
    Trainer(#[from] OpenAdapterTrainingExecutionError),
    #[error("legal DPO export failed: {0}")]
    OpenAdapter(#[from] OpenAdapterSftError),
    #[error("legal DPO core failed: {0}")]
    Core(#[from] TrainingCoreError),
    #[error("legal DPO parent SFT failed: {0}")]
    ParentSft(#[from] PsionicLegalSftError),
    #[error("legal DPO adapter load failed: {0}")]
    AdapterLoad(#[from] LmHeadLoraLoadError),
    #[error("legal DPO Qwen3.6 prompt render failed: {0}")]
    Qwen36Template(#[from] Qwen36TemplateError),
    #[error("legal DPO dataset failed: {0}")]
    DpoDataset(String),
}

/// Runs `psionic-train dpo --config <path>` args and returns the receipt.
pub fn run_psionic_legal_dpo_cli(
    args: &[String],
) -> Result<PsionicLegalDpoTrainingReceipt, PsionicLegalDpoError> {
    let config_path = parse_config_path(args)?;
    let artifacts = run_psionic_legal_dpo_config_path(config_path)?;
    Ok(artifacts.receipt)
}

/// Runs one DPO config from a JSON file.
pub fn run_psionic_legal_dpo_config_path(
    path: impl AsRef<Path>,
) -> Result<PsionicLegalDpoRunArtifacts, PsionicLegalDpoError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| PsionicLegalDpoError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let config: PsionicLegalDpoConfig =
        serde_json::from_slice(bytes.as_slice()).map_err(|error| PsionicLegalDpoError::Json {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    run_psionic_legal_dpo_config(&config)
}

/// Runs one parsed DPO config.
pub fn run_psionic_legal_dpo_config(
    config: &PsionicLegalDpoConfig,
) -> Result<PsionicLegalDpoRunArtifacts, PsionicLegalDpoError> {
    config.validate()?;
    ensure_parent_sft(config)?;
    let output_dir = PathBuf::from(&config.output_dir);
    fs::create_dir_all(&output_dir).map_err(|error| PsionicLegalDpoError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let parent_receipt = read_parent_sft_receipt(config)?;
    let parent_receipt_digest = sha256_file(config.parent_sft_receipt_path.as_str())?;
    let parent_adapter_bytes =
        fs::read(&config.parent_sft_adapter_path).map_err(|error| PsionicLegalDpoError::Io {
            path: config.parent_sft_adapter_path.clone(),
            message: error.to_string(),
        })?;
    let parent_adapter_digest = sha256_hex(parent_adapter_bytes.as_slice());
    let parent_adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        parent_adapter_bytes.as_slice(),
        parent_adapter_identity(config, &parent_receipt, parent_adapter_digest.as_str()),
        config.lora_alpha,
    )?;

    let pairs = load_legal_dpo_dataset(&config.dataset_path)
        .map_err(|error| PsionicLegalDpoError::DpoDataset(error.to_string()))?;
    let dataset_digest = sha256_file(config.dataset_path.as_str())?;
    let features = dpo_pair_features(config, pairs.as_slice())?;

    let backend = OpenAdapterTrainingExecutionBackend::new(
        OpenAdapterExecutionConfig {
            run_id: config.run_id.clone(),
            checkpoint_family: String::from(QWEN36_LEGAL_DPO_CHECKPOINT_FAMILY),
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
                    feature.pair_id.clone(),
                    feature.hidden_state.clone(),
                    feature.chosen_target_token_id,
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
        let batch = dpo_weighted_batch(config, features.as_slice(), step_index)?;
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
        loss_points.push(PsionicLegalDpoLossPoint {
            step: step_receipt.schedule.global_step,
            loss: weighted_record.mean_weighted_loss,
            raw_loss: weighted_record.mean_raw_loss,
            mean_loss_weight: weighted_record.mean_loss_weight,
            batch_id: weighted_record.batch_id.clone(),
            receipt_digest: step_receipt.receipt_digest.clone(),
            preference_accuracy: eval.preference_accuracy,
            average_preference_margin: eval.average_preference_margin,
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
        PsionicLegalDpoError::Io {
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
    let loss_curve = PsionicLegalDpoLossCurve {
        schema_version: String::from(PSIONIC_LEGAL_DPO_LOSS_CURVE_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        initial_eval: initial_eval.clone(),
        points: loss_points,
    };
    let loss_curve_path = output_dir.join("loss_curve.json");
    write_json(&loss_curve_path, &loss_curve)?;

    let checkpoint = checkpoint_summary(
        config,
        &run.summary(),
        parent_adapter_digest.as_str(),
        exported.adapter_artifact_digest.as_str(),
        exported.adapter_identity_digest.as_str(),
        step_receipts.as_slice(),
        weighted_records.as_slice(),
    );
    let checkpoint_path = output_dir.join("checkpoint_summary.json");
    write_json(&checkpoint_path, &checkpoint)?;

    let mut receipt = PsionicLegalDpoTrainingReceipt {
        schema_version: String::from(PSIONIC_LEGAL_DPO_RECEIPT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        trainer: String::from("psionic.open_adapter.qwen36_legal_lm_head_lora_dpo.v1"),
        train_type: config.train_type.clone(),
        base_model: config.base_model.clone(),
        served_model_id: config.served_model_id.clone(),
        base_model_revision: config.base_model_revision.clone(),
        base_artifact_mode: config.base_artifact_mode,
        base_served_artifact_digest: config.base_served_artifact_digest.clone(),
        tokenizer_digest: config.tokenizer_digest.clone(),
        prompt_template_digest: config.prompt_template_digest.clone(),
        qwen36_reasoning_mode: config.qwen36_reasoning_mode,
        beta: config.beta,
        assistant_only_loss: config.assistant_only_loss,
        ignore_empty_think_loss: config.ignore_empty_think_loss,
        parent_sft_adapter_path: config.parent_sft_adapter_path.clone(),
        parent_sft_receipt_path: config.parent_sft_receipt_path.clone(),
        parent_sft_receipt_digest: parent_receipt_digest,
        parent_sft_adapter_digest: parent_adapter_digest,
        dataset_path: config.dataset_path.clone(),
        dataset_digest,
        pair_count: features.len(),
        eval_suite_id: config.eval_suite_id.clone(),
        completed_steps: run.summary().completed_steps,
        initial_preference_accuracy: initial_eval.preference_accuracy,
        final_preference_accuracy: final_eval.preference_accuracy,
        initial_average_preference_margin: initial_eval.average_preference_margin,
        final_average_preference_margin: final_eval.average_preference_margin,
        preference_margin_improved: final_eval.average_preference_margin
            > initial_eval.average_preference_margin,
        adapter_artifact_path: adapter_path.display().to_string(),
        adapter_artifact_digest: exported.adapter_artifact_digest.clone(),
        adapter_identity_digest: exported.adapter_identity_digest.clone(),
        loss_curve_path: loss_curve_path.display().to_string(),
        checkpoint_summary_path: checkpoint_path.display().to_string(),
        python_invoked: false,
        claim_boundary: String::from(
            "This is a Rust-only adapter DPO smoke over synthetic Qwen3.6 hidden states derived from legal DPO pairs. It proves parent SFT adapter loading, Qwen3.6 prompt rendering, chosen/rejected weighted updates, checkpoint receipts, and adapter export. It does not claim full dense Qwen3.6 training or improvement on private Harvey benchmark tasks.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    let receipt_path = output_dir.join("training_receipt.json");
    write_json(&receipt_path, &receipt)?;

    Ok(PsionicLegalDpoRunArtifacts {
        adapter_artifact_path: adapter_path.display().to_string(),
        receipt_path: receipt_path.display().to_string(),
        loss_curve_path: loss_curve_path.display().to_string(),
        checkpoint_summary_path: checkpoint_path.display().to_string(),
        receipt,
    })
}

fn ensure_parent_sft(config: &PsionicLegalDpoConfig) -> Result<(), PsionicLegalDpoError> {
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

fn dpo_pair_features(
    config: &PsionicLegalDpoConfig,
    pairs: &[LegalDpoPreferencePair],
) -> Result<Vec<PsionicLegalDpoPairFeature>, PsionicLegalDpoError> {
    if pairs.is_empty() {
        return invalid_config("DPO dataset must contain at least one pair");
    }
    let renderer = Qwen36PromptRenderer::without_tokenizer();
    let options = Qwen36PromptOptions {
        reasoning_mode: config.qwen36_reasoning_mode,
        add_generation_prompt: true,
        emit_empty_think_block: config.emit_empty_think_block,
    };
    let mut features = Vec::with_capacity(pairs.len());
    for pair in pairs {
        validate_pair(pair)?;
        let rendered = renderer.render(&prompt_messages(pair.prompt.as_slice()), &options)?;
        let prompt_receipt = Qwen36PromptReceipt::from(&rendered);
        let hidden_state =
            hidden_state_from_digest(rendered.prompt_hash.as_str(), config.hidden_size);
        let chosen_target_token_id =
            token_id_from_text("chosen", pair.chosen.as_str(), config.vocab_size);
        let mut rejected_target_token_id =
            token_id_from_text("rejected", pair.rejected.as_str(), config.vocab_size);
        if rejected_target_token_id == chosen_target_token_id {
            rejected_target_token_id = (rejected_target_token_id + 1)
                % u32::try_from(config.vocab_size).unwrap_or(u32::MAX);
        }
        features.push(PsionicLegalDpoPairFeature {
            pair_id: pair.pair_id.clone(),
            pair_family: pair_family(pair.pair_id.as_str()),
            reason: pair.reason.clone(),
            prompt_receipt,
            hidden_state,
            chosen_target_token_id,
            rejected_target_token_id,
            source_token_count: approximate_token_count(rendered.text.as_str()),
        });
    }
    Ok(features)
}

fn validate_pair(pair: &LegalDpoPreferencePair) -> Result<(), PsionicLegalDpoError> {
    if pair.schema_version != LEGAL_DPO_DATASET_SCHEMA_VERSION {
        return invalid_config("DPO pair schema_version drifted");
    }
    require_nonempty(pair.pair_id.as_str(), "pair_id")?;
    if pair.prompt.is_empty() {
        return invalid_config("DPO pair prompt must not be empty");
    }
    require_nonempty(pair.chosen.as_str(), "chosen")?;
    require_nonempty(pair.rejected.as_str(), "rejected")?;
    if matches!(
        pair.visibility.as_str(),
        "hidden" | "hidden_training" | "private" | "private_training"
    ) {
        return invalid_config("hidden/private DPO pairs are not trainable");
    }
    Ok(())
}

fn dpo_weighted_batch(
    config: &PsionicLegalDpoConfig,
    features: &[PsionicLegalDpoPairFeature],
    step_index: u64,
) -> Result<OpenAdapterWeightedTargetBatchRequest, PsionicLegalDpoError> {
    let start = (step_index as usize * config.batch_size) % features.len();
    let mut targets = Vec::with_capacity(config.batch_size.saturating_mul(2));
    for offset in 0..config.batch_size {
        let feature = &features[(start + offset) % features.len()];
        targets.push(OpenAdapterWeightedTokenTarget::new(
            format!("{}.chosen", feature.pair_id),
            feature.hidden_state.clone(),
            feature.chosen_target_token_id,
            config.beta,
        ));
        targets.push(OpenAdapterWeightedTokenTarget::new(
            format!("{}.rejected", feature.pair_id),
            feature.hidden_state.clone(),
            feature.rejected_target_token_id,
            -config.beta,
        ));
    }
    Ok(OpenAdapterWeightedTargetBatchRequest::new(
        format!("legal-dpo-batch-{}", step_index + 1),
        targets,
        0.0,
    ))
}

fn evaluate_features(
    config: &PsionicLegalDpoConfig,
    adapter: &LmHeadLoraAdapterArtifact,
    features: &[PsionicLegalDpoPairFeature],
) -> PsionicLegalDpoEvalPoint {
    evaluate_lora_values(config, adapter.lora_a(), adapter.lora_b(), features)
}

fn evaluate_group_features(
    config: &PsionicLegalDpoConfig,
    groups: &[TrainingParameterGroupState],
    features: &[PsionicLegalDpoPairFeature],
) -> Result<PsionicLegalDpoEvalPoint, PsionicLegalDpoError> {
    if groups.len() != 2 {
        return invalid_config("expected two LoRA parameter groups");
    }
    let lora_a = group_values(&groups[0])?;
    let lora_b = group_values(&groups[1])?;
    Ok(evaluate_lora_values(config, lora_a, lora_b, features))
}

fn evaluate_lora_values(
    config: &PsionicLegalDpoConfig,
    lora_a: &[f32],
    lora_b: &[f32],
    features: &[PsionicLegalDpoPairFeature],
) -> PsionicLegalDpoEvalPoint {
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
    let mut correct = 0usize;
    let mut total_margin = 0.0_f32;
    for feature in features {
        let chosen = logprob_for_target(
            config,
            base_projection.as_slice(),
            lora_a,
            lora_b,
            feature.hidden_state.as_slice(),
            feature.chosen_target_token_id,
        );
        let rejected = logprob_for_target(
            config,
            base_projection.as_slice(),
            lora_a,
            lora_b,
            feature.hidden_state.as_slice(),
            feature.rejected_target_token_id,
        );
        let margin = chosen - rejected;
        if margin > 0.0 {
            correct += 1;
        }
        total_margin += margin;
    }
    let pair_count = features.len();
    let denominator = pair_count.max(1) as f32;
    PsionicLegalDpoEvalPoint {
        pair_count,
        preference_accuracy: correct as f32 / denominator,
        average_preference_margin: total_margin / denominator,
    }
}

fn logprob_for_target(
    config: &PsionicLegalDpoConfig,
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
    config: &PsionicLegalDpoConfig,
    run_summary: &TrainingRunSummary,
    parent_adapter_digest: &str,
    final_adapter_digest: &str,
    final_adapter_identity_digest: &str,
    step_receipts: &[TrainingStepReceipt],
    weighted_records: &[OpenAdapterWeightedTargetBatchRecord],
) -> PsionicLegalDpoCheckpointSummary {
    let mut checkpoint = PsionicLegalDpoCheckpointSummary {
        schema_version: String::from(PSIONIC_LEGAL_DPO_CHECKPOINT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        checkpoint_family: String::from(QWEN36_LEGAL_DPO_CHECKPOINT_FAMILY),
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
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest = checkpoint.stable_digest();
    checkpoint
}

fn parent_adapter_identity(
    config: &PsionicLegalDpoConfig,
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
    config: &PsionicLegalDpoConfig,
) -> Result<PsionicTrainingReceipt, PsionicLegalDpoError> {
    let bytes =
        fs::read(&config.parent_sft_receipt_path).map_err(|error| PsionicLegalDpoError::Io {
            path: config.parent_sft_receipt_path.clone(),
            message: error.to_string(),
        })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| PsionicLegalDpoError::Json {
        path: config.parent_sft_receipt_path.clone(),
        message: error.to_string(),
    })
}

fn group_values(group: &TrainingParameterGroupState) -> Result<&[f32], PsionicLegalDpoError> {
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        _ => invalid_config("DPO smoke expected f32 LoRA parameter groups"),
    }
}

fn prompt_messages(messages: &[LegalDpoMessage]) -> Vec<PromptMessage> {
    messages
        .iter()
        .map(|message| {
            PromptMessage::new(
                match message.role.as_str() {
                    "system" => PromptMessageRole::System,
                    "developer" => PromptMessageRole::Developer,
                    "assistant" => PromptMessageRole::Assistant,
                    "tool" => PromptMessageRole::Tool,
                    _ => PromptMessageRole::User,
                },
                message.content.clone(),
            )
        })
        .collect()
}

fn pair_family(pair_id: &str) -> String {
    pair_id
        .rsplit_once('.')
        .and_then(|(prefix, _)| prefix.rsplit_once('.').map(|(_, family)| family))
        .unwrap_or("unknown")
        .to_string()
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

fn parse_config_path(args: &[String]) -> Result<&Path, PsionicLegalDpoError> {
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
                return invalid_config("usage: psionic-train dpo --config <path>");
            }
            other => {
                return invalid_config(format!("unsupported dpo argument `{other}`"));
            }
        }
    }
    invalid_config("missing --config <path>")
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), PsionicLegalDpoError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| PsionicLegalDpoError::Serialization {
            message: error.to_string(),
        })?;
    fs::write(path, bytes).map_err(|error| PsionicLegalDpoError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn require_nonempty(value: &str, field: &str) -> Result<(), PsionicLegalDpoError> {
    if value.trim().is_empty() {
        return invalid_config(format!("{field} must be present"));
    }
    Ok(())
}

fn invalid_config<T>(detail: impl Into<String>) -> Result<T, PsionicLegalDpoError> {
    Err(PsionicLegalDpoError::InvalidConfig {
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

fn sha256_file(path: &str) -> Result<String, PsionicLegalDpoError> {
    let bytes = fs::read(path).map_err(|error| PsionicLegalDpoError::Io {
        path: path.to_string(),
        message: error.to_string(),
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
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

/// Returns the default synthetic Qwen3.6 legal DPO smoke config.
#[must_use]
pub fn default_qwen36_legal_dpo_smoke_config(
    output_dir: impl Into<String>,
) -> PsionicLegalDpoConfig {
    PsionicLegalDpoConfig {
        schema_version: String::from(PSIONIC_LEGAL_DPO_CONFIG_SCHEMA_VERSION),
        run_id: String::from("qwen36-legal-dpo-smoke"),
        train_type: String::from("dpo"),
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
        learning_rate: 0.08,
        max_steps: 6,
        batch_size: 4,
        beta: 0.25,
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
        dataset_path: String::from("fixtures/legal_benchmark/dpo_smoke/legal-dpo-v1.jsonl"),
        dataset_ref: String::from("dataset://openagents/legal-benchmark/legal-dpo-v1@smoke"),
        validator_policy_ref: String::from("policy://validator/legal-benchmark/qwen36-dpo-smoke"),
        eval_suite_id: String::from("harvey_public_three_deterministic_replay_v1"),
        adapter_id: String::from("qwen36-27b-legal-dpo-smoke"),
        adapter_revision: String::from("r1"),
        output_dir: output_dir.into(),
        started_at_ms: 2_000,
        step_duration_ms: 20,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_data::{LEGAL_DPO_DATASET_SCHEMA_VERSION, LegalDpoMessage, LegalDpoPreferencePair};

    #[test]
    fn legal_qwen36_dpo_smoke_prefers_file_write_over_chat_only()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let sft_dir = temp.path().join("sft");
        let dpo_dir = temp.path().join("dpo");
        let dataset = temp.path().join("legal-dpo-v1.jsonl");
        write_test_pair(&dataset)?;
        let mut config = default_qwen36_legal_dpo_smoke_config(dpo_dir.display().to_string());
        config.parent_sft_adapter_path = sft_dir.join("adapter.safetensors").display().to_string();
        config.parent_sft_receipt_path =
            sft_dir.join("training_receipt.json").display().to_string();
        config.dataset_path = dataset.display().to_string();
        config.parent_sft_config_path = None;
        config.bootstrap_parent_sft_if_missing = false;

        let sft_config =
            crate::default_qwen36_legal_sft_smoke_config(sft_dir.display().to_string());
        crate::run_psionic_legal_sft_config(&sft_config)?;
        let artifacts = run_psionic_legal_dpo_config(&config)?;

        assert!(Path::new(&artifacts.adapter_artifact_path).is_file());
        assert!(Path::new(&artifacts.receipt_path).is_file());
        assert!(Path::new(&artifacts.loss_curve_path).is_file());
        assert!(Path::new(&artifacts.checkpoint_summary_path).is_file());
        assert!(!artifacts.receipt.python_invoked);
        assert_eq!(artifacts.receipt.pair_count, 1);
        assert!(artifacts.receipt.preference_margin_improved);
        assert!(
            artifacts.receipt.final_average_preference_margin
                > artifacts.receipt.initial_average_preference_margin
        );
        assert_eq!(
            artifacts.receipt.receipt_digest,
            artifacts.receipt.stable_digest()
        );
        Ok(())
    }

    fn write_test_pair(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let pair = LegalDpoPreferencePair {
            schema_version: String::from(LEGAL_DPO_DATASET_SCHEMA_VERSION),
            pair_id: String::from("run.good.run.bad.file_discipline.1"),
            prompt: vec![
                LegalDpoMessage {
                    role: String::from("system"),
                    content: String::from("You are a legal benchmark agent."),
                },
                LegalDpoMessage {
                    role: String::from("user"),
                    content: String::from("Write memo.md."),
                },
            ],
            chosen: String::from(
                "I will write memo.md through the output tool.\n\nmemo.md:\nThe answer is concise and source-grounded.",
            ),
            rejected: String::from("I can answer in chat only and skip the file."),
            reason: String::from("DidNotWriteRequiredFile"),
            source_run_ids: vec![String::from("run.good"), String::from("run.bad")],
            visibility: String::from("public_training"),
            exclusion_flags: Vec::new(),
        };
        fs::write(path, format!("{}\n", serde_json::to_string(&pair)?))?;
        Ok(())
    }
}
