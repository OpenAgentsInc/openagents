use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    first_swarm_run_contract, first_swarm_tokenizer_digest, OpenAdapterAdmissibleModelFamily,
    OpenAdapterExecutionConfig, OpenAdapterHiddenStateSample, OpenAdapterLmHeadTarget,
    OpenAdapterPrecisionPolicy, OpenAdapterReferenceModel, OpenAdapterSftRunOutcome,
    OpenAdapterSftRunRequest, OpenAdapterTrainingExecutionBackend, PortableTokenizerAssetFormat,
    PortableTokenizerBinding, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy,
};

/// Stable contract identifier for the first swarm open-adapter receipt language.
pub const FIRST_SWARM_OPEN_ADAPTER_RECEIPT_CONTRACT_ID: &str =
    "swarm.open_adapter.receipt_contract.v1";
/// Stable fixture path for the first swarm open-adapter receipt contract.
pub const FIRST_SWARM_OPEN_ADAPTER_RECEIPT_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/swarm/first_swarm_open_adapter_receipt_contract_v1.json";
/// Stable contributor receipt schema version for the first swarm lane.
pub const FIRST_SWARM_OPEN_ADAPTER_CONTRIBUTOR_RECEIPT_SCHEMA_VERSION: &str =
    "swarm.open_adapter.contributor_receipt.v1";
/// Frozen base model id for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_ID: &str = "gpt-oss-20b";
/// Frozen base model revision for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_REVISION: &str = "swarm-local-v1";
/// Frozen base served-artifact digest for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_BASE_SERVED_ARTIFACT_DIGEST: &str =
    "sha256:swarm-open-adapter-base";
/// Frozen hidden width for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_HIDDEN_SIZE: usize = 4;
/// Frozen vocabulary width for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_VOCAB_SIZE: usize = 4;
/// Frozen LoRA rank for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_LORA_RANK: usize = 2;
/// Frozen LoRA alpha for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_LORA_ALPHA: f32 = 8.0;
/// Frozen batch size for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_BATCH_SIZE: usize = 2;
/// Frozen sample count for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_SAMPLE_COUNT: usize = 4;
/// Frozen deterministic probe target token id for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_PROBE_TARGET_TOKEN_ID: usize = 2;
/// Frozen precision policy for the first swarm open-adapter lane.
pub const FIRST_SWARM_OPEN_ADAPTER_PRECISION_POLICY: &str = "f32_reference";

/// Canonical cross-backend receipt contract for the first swarm open-adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmOpenAdapterReceiptContract {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable first swarm run family id.
    pub run_family_id: String,
    /// Stable first swarm contract digest.
    pub swarm_contract_digest: String,
    /// Stable dataset ref.
    pub dataset_ref: String,
    /// Stable dataset-manifest digest.
    pub dataset_manifest_digest: String,
    /// Stable validator policy id.
    pub validator_policy_id: String,
    /// Stable aggregation policy id.
    pub aggregation_policy_id: String,
    /// Stable replay policy id.
    pub replay_policy_id: String,
    /// Stable adapter family.
    pub adapter_family: String,
    /// Stable adapter format.
    pub adapter_format: String,
    /// Stable admissible model family.
    pub admissible_model_family: String,
    /// Stable precision policy.
    pub precision_policy: String,
    /// Stable base model id.
    pub base_model_id: String,
    /// Stable base model revision.
    pub base_model_revision: String,
    /// Stable base served-artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer digest.
    pub tokenizer_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Frozen hidden-state width.
    pub hidden_size: usize,
    /// Frozen vocabulary width.
    pub vocab_size: usize,
    /// Frozen LoRA rank.
    pub lora_rank: usize,
    /// Frozen LoRA alpha.
    pub lora_alpha: f32,
    /// Frozen batch size.
    pub batch_size: usize,
    /// Frozen sample count.
    pub sample_count: usize,
    /// Stable deterministic probe target token id.
    pub deterministic_probe_target_token_id: usize,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl FirstSwarmOpenAdapterReceiptContract {
    /// Returns the stable digest over the contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(
            b"psionic_first_swarm_open_adapter_receipt_contract|",
            &clone,
        )
    }
}

/// Backend-tagged manifest emitted by one first-swarm contributor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmOpenAdapterContributorManifest {
    /// Stable receipt-contract digest shared by every contributor.
    pub receipt_contract_digest: String,
    /// Stable run family id.
    pub run_family_id: String,
    /// Stable first swarm contract digest.
    pub swarm_contract_digest: String,
    /// Stable dataset ref.
    pub dataset_ref: String,
    /// Stable dataset-manifest digest.
    pub dataset_manifest_digest: String,
    /// Stable validator policy id.
    pub validator_policy_id: String,
    /// Stable aggregation policy id.
    pub aggregation_policy_id: String,
    /// Stable replay policy id.
    pub replay_policy_id: String,
    /// Stable contributor role id.
    pub contributor_role_id: String,
    /// Stable backend label.
    pub execution_backend_label: String,
    /// Stable logical-device kind.
    pub logical_device_kind: String,
    /// Stable logical-device label.
    pub logical_device_label: String,
    /// Stable admissible model family.
    pub admissible_model_family: String,
    /// Stable adapter family.
    pub adapter_family: String,
    /// Stable adapter format.
    pub adapter_format: String,
    /// Stable precision policy.
    pub precision_policy: String,
    /// Stable base model id.
    pub base_model_id: String,
    /// Stable base model revision.
    pub base_model_revision: String,
    /// Stable base served-artifact digest.
    pub base_served_artifact_digest: String,
    /// Stable tokenizer digest.
    pub tokenizer_digest: String,
    /// Stable tokenizer contract digest.
    pub tokenizer_contract_digest: String,
    /// Frozen hidden-state width.
    pub hidden_size: usize,
    /// Frozen vocabulary width.
    pub vocab_size: usize,
    /// Frozen LoRA rank.
    pub lora_rank: usize,
    /// Frozen LoRA alpha.
    pub lora_alpha: f32,
    /// Frozen batch size.
    pub batch_size: usize,
    /// Frozen sample count.
    pub sample_count: usize,
    /// Stable replay identity digest across backends.
    pub shared_replay_identity_digest: String,
    /// Stable manifest digest.
    pub manifest_digest: String,
}

impl FirstSwarmOpenAdapterContributorManifest {
    /// Returns the stable digest over the contributor manifest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.manifest_digest.clear();
        stable_digest(
            b"psionic_first_swarm_open_adapter_contributor_manifest|",
            &clone,
        )
    }
}

/// Machine-legible contributor receipt emitted by the first swarm open-adapter lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirstSwarmOpenAdapterContributorReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Backend-tagged manifest.
    pub manifest: FirstSwarmOpenAdapterContributorManifest,
    /// Stable run identifier.
    pub run_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Executed optimization steps.
    pub executed_steps: usize,
    /// Packed batch count used by the run.
    pub batch_count: usize,
    /// Final mean loss from the last gradient batch.
    pub final_mean_loss: f32,
    /// Stable adapter artifact digest.
    pub adapter_artifact_digest: String,
    /// Stable adapter identity digest.
    pub adapter_identity_digest: String,
    /// Stable initial state-dict digest.
    pub initial_state_dict_digest: String,
    /// Stable final state-dict digest.
    pub final_state_dict_digest: String,
    /// Stable execution-provenance digest.
    pub execution_provenance_digest: String,
    /// Stable deterministic probe token id.
    pub deterministic_probe_top_token_id: usize,
    /// Explicit unsupported-precision refusal.
    pub unsupported_precision_refusal: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl FirstSwarmOpenAdapterContributorReceipt {
    /// Returns the stable digest over the contributor receipt.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_digest(
            b"psionic_first_swarm_open_adapter_contributor_receipt|",
            &clone,
        )
    }
}

/// Aggregate compatibility result for one comparable contributor set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSwarmOpenAdapterAggregationCompatibility {
    /// Stable receipt-contract digest admitted by the contributor set.
    pub receipt_contract_digest: String,
    /// Stable replay identity digest admitted by the contributor set.
    pub shared_replay_identity_digest: String,
    /// Stable contributor role ids admitted by the contributor set.
    pub contributor_role_ids: Vec<String>,
    /// Stable backend labels admitted by the contributor set.
    pub execution_backend_labels: Vec<String>,
    /// Stable contributor receipt digests admitted by the contributor set.
    pub contributor_receipt_digests: Vec<String>,
}

/// Errors surfaced while building or comparing first-swarm contributor receipts.
#[derive(Debug, Error)]
pub enum FirstSwarmOpenAdapterReceiptError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("first swarm open-adapter receipt mismatch for `{field}`: expected `{expected}`, got `{actual}`")]
    ContractMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("first swarm open-adapter contributor receipts disagree on `{field}`: left `{left}`, right `{right}`")]
    CrossBackendMismatch {
        field: &'static str,
        left: String,
        right: String,
    },
    #[error("first swarm open-adapter receipt digest drifted for `{field}`")]
    DigestMismatch { field: &'static str },
    #[error("first swarm open-adapter aggregation requires at least one contributor receipt")]
    EmptyContributorSet,
    #[error("first swarm open-adapter aggregation repeated contributor role `{role_id}`")]
    DuplicateContributorRole { role_id: String },
    #[error("first swarm open-adapter aggregation is missing contributor role `{role_id}`")]
    MissingContributorRole { role_id: String },
}

/// Returns the canonical first-swarm open-adapter receipt contract.
#[must_use]
pub fn first_swarm_open_adapter_receipt_contract() -> FirstSwarmOpenAdapterReceiptContract {
    let swarm_contract = first_swarm_run_contract();
    let tokenizer_contract_digest = PortableTokenizerBinding::new(
        first_swarm_tokenizer_digest(),
        PortableTokenizerAssetFormat::PsionicDigest,
        format!(
            "{}@{}",
            FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_ID, FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_REVISION
        ),
    )
    .contract_digest();
    let mut contract = FirstSwarmOpenAdapterReceiptContract {
        schema_version: 1,
        contract_id: String::from(FIRST_SWARM_OPEN_ADAPTER_RECEIPT_CONTRACT_ID),
        run_family_id: swarm_contract.run_family_id.clone(),
        swarm_contract_digest: swarm_contract.contract_digest.clone(),
        dataset_ref: swarm_contract.dataset.dataset_key.dataset_ref.clone(),
        dataset_manifest_digest: swarm_contract.dataset.dataset_manifest_digest.clone(),
        validator_policy_id: swarm_contract.governance.validator_policy_id.clone(),
        aggregation_policy_id: swarm_contract.governance.aggregation_policy_id.clone(),
        replay_policy_id: swarm_contract.governance.replay_policy_id.clone(),
        adapter_family: swarm_contract.adapter_family.clone(),
        adapter_format: swarm_contract.adapter_format.clone(),
        admissible_model_family: String::from("gpt_oss_decoder_lm_head_lora"),
        precision_policy: String::from(FIRST_SWARM_OPEN_ADAPTER_PRECISION_POLICY),
        base_model_id: String::from(FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_ID),
        base_model_revision: String::from(FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_REVISION),
        base_served_artifact_digest: String::from(
            FIRST_SWARM_OPEN_ADAPTER_BASE_SERVED_ARTIFACT_DIGEST,
        ),
        tokenizer_digest: swarm_contract.dataset.tokenizer.tokenizer_digest.clone(),
        tokenizer_contract_digest,
        hidden_size: FIRST_SWARM_OPEN_ADAPTER_HIDDEN_SIZE,
        vocab_size: FIRST_SWARM_OPEN_ADAPTER_VOCAB_SIZE,
        lora_rank: FIRST_SWARM_OPEN_ADAPTER_LORA_RANK,
        lora_alpha: FIRST_SWARM_OPEN_ADAPTER_LORA_ALPHA,
        batch_size: FIRST_SWARM_OPEN_ADAPTER_BATCH_SIZE,
        sample_count: FIRST_SWARM_OPEN_ADAPTER_SAMPLE_COUNT,
        deterministic_probe_target_token_id: FIRST_SWARM_OPEN_ADAPTER_PROBE_TARGET_TOKEN_ID,
        claim_boundary: String::from(
            "This contract freezes the first swarm lane to one f32-only open-adapter receipt language shared by the Mac MLX Metal contributor and the Linux CUDA contributor. It keeps backend label and logical device explicit, but it requires identical dataset, replay, adapter-family, tokenizer, base-model, and hidden-state geometry truth before aggregation may accept both contributors.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract
}

/// Writes the canonical first-swarm open-adapter receipt contract to one JSON path.
pub fn write_first_swarm_open_adapter_receipt_contract(
    output_path: impl AsRef<Path>,
) -> Result<FirstSwarmOpenAdapterReceiptContract, FirstSwarmOpenAdapterReceiptError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSwarmOpenAdapterReceiptError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = first_swarm_open_adapter_receipt_contract();
    let encoded = serde_json::to_string_pretty(&contract)
        .expect("first swarm open-adapter receipt contract should serialize");
    fs::write(output_path, format!("{encoded}\n")).map_err(|error| {
        FirstSwarmOpenAdapterReceiptError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

/// Returns the canonical first-swarm training config for one admitted backend label.
pub fn first_swarm_open_adapter_training_config(
    run_id: impl Into<String>,
    checkpoint_family: impl Into<String>,
    execution_backend_label: impl Into<String>,
) -> OpenAdapterExecutionConfig {
    OpenAdapterExecutionConfig {
        run_id: run_id.into(),
        checkpoint_family: checkpoint_family.into(),
        execution_backend_label: execution_backend_label.into(),
        admissible_model_family: OpenAdapterAdmissibleModelFamily::GptOssDecoderLmHeadLora,
        budget: TrainingLoopBudget::new(12, 1, 1).expect("first swarm budget should stay valid"),
        batch_size: FIRST_SWARM_OPEN_ADAPTER_BATCH_SIZE,
        precision_policy: OpenAdapterPrecisionPolicy::F32Reference,
        model: OpenAdapterReferenceModel {
            base_model_id: String::from(FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_ID),
            base_model_revision: String::from(FIRST_SWARM_OPEN_ADAPTER_BASE_MODEL_REVISION),
            base_served_artifact_digest: String::from(
                FIRST_SWARM_OPEN_ADAPTER_BASE_SERVED_ARTIFACT_DIGEST,
            ),
            tokenizer: first_swarm_tokenizer_digest(),
            hidden_size: FIRST_SWARM_OPEN_ADAPTER_HIDDEN_SIZE,
            vocab_size: FIRST_SWARM_OPEN_ADAPTER_VOCAB_SIZE,
            target: OpenAdapterLmHeadTarget {
                target_id: String::from("lm_head"),
                lora_rank: FIRST_SWARM_OPEN_ADAPTER_LORA_RANK,
                lora_alpha: FIRST_SWARM_OPEN_ADAPTER_LORA_ALPHA,
                optimizer: TrainingOptimizerConfig::adamw(0.2, 0.9, 0.99, 1e-8)
                    .with_gradient_clip_norm(1.0),
                optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
            },
        },
    }
}

/// Returns the canonical first-swarm supervision samples for one contributor.
pub fn first_swarm_open_adapter_samples(
    sample_prefix: &str,
) -> Result<Vec<OpenAdapterHiddenStateSample>, FirstSwarmOpenAdapterReceiptError> {
    Ok(vec![
        sample(sample_prefix, "a", vec![1.0, 0.0, 0.0, 0.0], 2, 16)?,
        sample(sample_prefix, "b", vec![0.0, 1.0, 0.0, 0.0], 3, 15)?,
        sample(sample_prefix, "c", vec![1.0, 0.0, 0.0, 0.0], 2, 14)?,
        sample(sample_prefix, "d", vec![0.0, 1.0, 0.0, 0.0], 3, 13)?,
    ])
}

/// Returns the canonical SFT request for the first swarm open-adapter lane.
#[must_use]
pub fn first_swarm_open_adapter_sft_request(
    adapter_id: impl Into<String>,
    adapter_revision: impl Into<String>,
    started_at_ms: u64,
    step_duration_ms: u64,
) -> OpenAdapterSftRunRequest {
    let receipt_contract = first_swarm_open_adapter_receipt_contract();
    OpenAdapterSftRunRequest {
        dataset_ref: receipt_contract.dataset_ref,
        validator_policy_ref: receipt_contract.validator_policy_id,
        adapter_id: adapter_id.into(),
        adapter_revision: adapter_revision.into(),
        started_at_ms,
        step_duration_ms,
    }
}

/// Builds the canonical comparable contributor receipt for one first-swarm run.
pub fn build_first_swarm_open_adapter_contributor_receipt(
    contributor_role_id: impl Into<String>,
    backend: &OpenAdapterTrainingExecutionBackend,
    outcome: &OpenAdapterSftRunOutcome,
    deterministic_probe_top_token_id: usize,
    unsupported_precision_refusal: impl Into<String>,
) -> Result<FirstSwarmOpenAdapterContributorReceipt, FirstSwarmOpenAdapterReceiptError> {
    let contributor_role_id = contributor_role_id.into();
    let contract = first_swarm_open_adapter_receipt_contract();
    let mut manifest = FirstSwarmOpenAdapterContributorManifest {
        receipt_contract_digest: contract.contract_digest.clone(),
        run_family_id: contract.run_family_id.clone(),
        swarm_contract_digest: contract.swarm_contract_digest.clone(),
        dataset_ref: contract.dataset_ref.clone(),
        dataset_manifest_digest: contract.dataset_manifest_digest.clone(),
        validator_policy_id: contract.validator_policy_id.clone(),
        aggregation_policy_id: contract.aggregation_policy_id.clone(),
        replay_policy_id: contract.replay_policy_id.clone(),
        contributor_role_id,
        execution_backend_label: backend.config().execution_backend_label.clone(),
        logical_device_kind: backend.provenance().logical_device_kind.to_string(),
        logical_device_label: backend.provenance().logical_device_label.clone(),
        admissible_model_family: String::from("gpt_oss_decoder_lm_head_lora"),
        adapter_family: backend.provenance().adapter_family.clone(),
        adapter_format: backend.provenance().adapter_format.clone(),
        precision_policy: precision_policy_label(backend.config().precision_policy).to_string(),
        base_model_id: backend.config().model.base_model_id.clone(),
        base_model_revision: backend.config().model.base_model_revision.clone(),
        base_served_artifact_digest: backend.config().model.base_served_artifact_digest.clone(),
        tokenizer_digest: backend.config().model.tokenizer.tokenizer_digest.clone(),
        tokenizer_contract_digest: backend.provenance().tokenizer_contract_digest.clone(),
        hidden_size: backend.config().model.hidden_size,
        vocab_size: backend.config().model.vocab_size,
        lora_rank: backend.config().model.target.lora_rank,
        lora_alpha: backend.config().model.target.lora_alpha,
        batch_size: backend.config().batch_size,
        sample_count: backend.provenance().sample_count,
        shared_replay_identity_digest: shared_replay_identity_digest(
            contract.contract_digest.as_str(),
            backend.provenance().adapter_family.as_str(),
            backend.provenance().adapter_format.as_str(),
            precision_policy_label(backend.config().precision_policy),
        ),
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = manifest.stable_digest();
    let mut receipt = FirstSwarmOpenAdapterContributorReceipt {
        schema_version: String::from(FIRST_SWARM_OPEN_ADAPTER_CONTRIBUTOR_RECEIPT_SCHEMA_VERSION),
        manifest,
        run_id: backend.config().run_id.clone(),
        checkpoint_family: backend.config().checkpoint_family.clone(),
        executed_steps: outcome.step_receipts.len(),
        batch_count: backend.batches().len(),
        final_mean_loss: outcome
            .gradient_records
            .last()
            .map(|record| record.mean_loss)
            .unwrap_or_default(),
        adapter_artifact_digest: outcome.summary.adapter_artifact_digest.clone(),
        adapter_identity_digest: outcome.summary.adapter_identity_digest.clone(),
        initial_state_dict_digest: outcome.summary.initial_state_dict_digest.clone(),
        final_state_dict_digest: outcome.summary.final_state_dict_digest.clone(),
        execution_provenance_digest: outcome.summary.execution_provenance.stable_digest(),
        deterministic_probe_top_token_id,
        unsupported_precision_refusal: unsupported_precision_refusal.into(),
        receipt_digest: String::new(),
    };
    validate_first_swarm_open_adapter_contributor_receipt(&receipt)?;
    receipt.receipt_digest = receipt.stable_digest();
    Ok(receipt)
}

/// Validates one first-swarm open-adapter contributor receipt against the shared contract.
pub fn validate_first_swarm_open_adapter_contributor_receipt(
    receipt: &FirstSwarmOpenAdapterContributorReceipt,
) -> Result<(), FirstSwarmOpenAdapterReceiptError> {
    let contract = first_swarm_open_adapter_receipt_contract();
    if receipt.schema_version != FIRST_SWARM_OPEN_ADAPTER_CONTRIBUTOR_RECEIPT_SCHEMA_VERSION {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "schema_version",
            expected: String::from(FIRST_SWARM_OPEN_ADAPTER_CONTRIBUTOR_RECEIPT_SCHEMA_VERSION),
            actual: receipt.schema_version.clone(),
        });
    }
    if receipt.manifest.receipt_contract_digest != contract.contract_digest {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "manifest.receipt_contract_digest",
            expected: contract.contract_digest,
            actual: receipt.manifest.receipt_contract_digest.clone(),
        });
    }
    ensure_contract_field(
        "manifest.run_family_id",
        contract.run_family_id.as_str(),
        receipt.manifest.run_family_id.as_str(),
    )?;
    ensure_contract_field(
        "manifest.swarm_contract_digest",
        contract.swarm_contract_digest.as_str(),
        receipt.manifest.swarm_contract_digest.as_str(),
    )?;
    ensure_contract_field(
        "manifest.dataset_ref",
        contract.dataset_ref.as_str(),
        receipt.manifest.dataset_ref.as_str(),
    )?;
    ensure_contract_field(
        "manifest.dataset_manifest_digest",
        contract.dataset_manifest_digest.as_str(),
        receipt.manifest.dataset_manifest_digest.as_str(),
    )?;
    ensure_contract_field(
        "manifest.validator_policy_id",
        contract.validator_policy_id.as_str(),
        receipt.manifest.validator_policy_id.as_str(),
    )?;
    ensure_contract_field(
        "manifest.aggregation_policy_id",
        contract.aggregation_policy_id.as_str(),
        receipt.manifest.aggregation_policy_id.as_str(),
    )?;
    ensure_contract_field(
        "manifest.replay_policy_id",
        contract.replay_policy_id.as_str(),
        receipt.manifest.replay_policy_id.as_str(),
    )?;
    ensure_contract_field(
        "manifest.adapter_family",
        contract.adapter_family.as_str(),
        receipt.manifest.adapter_family.as_str(),
    )?;
    ensure_contract_field(
        "manifest.adapter_format",
        contract.adapter_format.as_str(),
        receipt.manifest.adapter_format.as_str(),
    )?;
    ensure_contract_field(
        "manifest.admissible_model_family",
        contract.admissible_model_family.as_str(),
        receipt.manifest.admissible_model_family.as_str(),
    )?;
    ensure_contract_field(
        "manifest.precision_policy",
        contract.precision_policy.as_str(),
        receipt.manifest.precision_policy.as_str(),
    )?;
    ensure_contract_field(
        "manifest.base_model_id",
        contract.base_model_id.as_str(),
        receipt.manifest.base_model_id.as_str(),
    )?;
    ensure_contract_field(
        "manifest.base_model_revision",
        contract.base_model_revision.as_str(),
        receipt.manifest.base_model_revision.as_str(),
    )?;
    ensure_contract_field(
        "manifest.base_served_artifact_digest",
        contract.base_served_artifact_digest.as_str(),
        receipt.manifest.base_served_artifact_digest.as_str(),
    )?;
    ensure_contract_field(
        "manifest.tokenizer_digest",
        contract.tokenizer_digest.as_str(),
        receipt.manifest.tokenizer_digest.as_str(),
    )?;
    ensure_contract_field(
        "manifest.tokenizer_contract_digest",
        contract.tokenizer_contract_digest.as_str(),
        receipt.manifest.tokenizer_contract_digest.as_str(),
    )?;
    ensure_contract_usize(
        "manifest.hidden_size",
        contract.hidden_size,
        receipt.manifest.hidden_size,
    )?;
    ensure_contract_usize(
        "manifest.vocab_size",
        contract.vocab_size,
        receipt.manifest.vocab_size,
    )?;
    ensure_contract_usize(
        "manifest.lora_rank",
        contract.lora_rank,
        receipt.manifest.lora_rank,
    )?;
    ensure_contract_f32(
        "manifest.lora_alpha",
        contract.lora_alpha,
        receipt.manifest.lora_alpha,
    )?;
    ensure_contract_usize(
        "manifest.batch_size",
        contract.batch_size,
        receipt.manifest.batch_size,
    )?;
    ensure_contract_usize(
        "manifest.sample_count",
        contract.sample_count,
        receipt.manifest.sample_count,
    )?;
    if receipt.manifest.manifest_digest != receipt.manifest.stable_digest() {
        return Err(FirstSwarmOpenAdapterReceiptError::DigestMismatch {
            field: "manifest.manifest_digest",
        });
    }
    if receipt.executed_steps == 0 || receipt.batch_count == 0 {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "receipt.execution_counts",
            expected: String::from("positive step and batch counts"),
            actual: format!(
                "executed_steps={} batch_count={}",
                receipt.executed_steps, receipt.batch_count
            ),
        });
    }
    if receipt.deterministic_probe_top_token_id != contract.deterministic_probe_target_token_id {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "receipt.deterministic_probe_top_token_id",
            expected: contract.deterministic_probe_target_token_id.to_string(),
            actual: receipt.deterministic_probe_top_token_id.to_string(),
        });
    }
    if !receipt
        .unsupported_precision_refusal
        .contains("does not yet support precision policy")
    {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "receipt.unsupported_precision_refusal",
            expected: String::from("explicit unsupported precision refusal"),
            actual: receipt.unsupported_precision_refusal.clone(),
        });
    }
    Ok(())
}

/// Compares a contributor set and returns the admitted aggregation contract when comparable.
pub fn compare_first_swarm_open_adapter_contributor_receipts(
    receipts: &[FirstSwarmOpenAdapterContributorReceipt],
) -> Result<FirstSwarmOpenAdapterAggregationCompatibility, FirstSwarmOpenAdapterReceiptError> {
    if receipts.is_empty() {
        return Err(FirstSwarmOpenAdapterReceiptError::EmptyContributorSet);
    }
    for receipt in receipts {
        validate_first_swarm_open_adapter_contributor_receipt(receipt)?;
    }
    let mut role_ids = Vec::with_capacity(receipts.len());
    for receipt in receipts {
        if role_ids.contains(&receipt.manifest.contributor_role_id) {
            return Err(
                FirstSwarmOpenAdapterReceiptError::DuplicateContributorRole {
                    role_id: receipt.manifest.contributor_role_id.clone(),
                },
            );
        }
        role_ids.push(receipt.manifest.contributor_role_id.clone());
    }
    let swarm_contract = first_swarm_run_contract();
    for role in &swarm_contract.node_roles {
        if !role_ids.iter().any(|role_id| role_id == &role.role_id) {
            return Err(FirstSwarmOpenAdapterReceiptError::MissingContributorRole {
                role_id: role.role_id.clone(),
            });
        }
    }
    let first = &receipts[0];
    for receipt in &receipts[1..] {
        ensure_same_field(
            "manifest.receipt_contract_digest",
            first.manifest.receipt_contract_digest.as_str(),
            receipt.manifest.receipt_contract_digest.as_str(),
        )?;
        ensure_same_field(
            "manifest.shared_replay_identity_digest",
            first.manifest.shared_replay_identity_digest.as_str(),
            receipt.manifest.shared_replay_identity_digest.as_str(),
        )?;
        ensure_same_field(
            "manifest.dataset_manifest_digest",
            first.manifest.dataset_manifest_digest.as_str(),
            receipt.manifest.dataset_manifest_digest.as_str(),
        )?;
        ensure_same_field(
            "manifest.adapter_family",
            first.manifest.adapter_family.as_str(),
            receipt.manifest.adapter_family.as_str(),
        )?;
        ensure_same_field(
            "manifest.adapter_format",
            first.manifest.adapter_format.as_str(),
            receipt.manifest.adapter_format.as_str(),
        )?;
        ensure_same_field(
            "manifest.precision_policy",
            first.manifest.precision_policy.as_str(),
            receipt.manifest.precision_policy.as_str(),
        )?;
        ensure_same_field(
            "manifest.base_model_id",
            first.manifest.base_model_id.as_str(),
            receipt.manifest.base_model_id.as_str(),
        )?;
        ensure_same_field(
            "manifest.base_model_revision",
            first.manifest.base_model_revision.as_str(),
            receipt.manifest.base_model_revision.as_str(),
        )?;
        ensure_same_field(
            "manifest.base_served_artifact_digest",
            first.manifest.base_served_artifact_digest.as_str(),
            receipt.manifest.base_served_artifact_digest.as_str(),
        )?;
        ensure_same_field(
            "manifest.tokenizer_contract_digest",
            first.manifest.tokenizer_contract_digest.as_str(),
            receipt.manifest.tokenizer_contract_digest.as_str(),
        )?;
        ensure_same_field(
            "manifest.admissible_model_family",
            first.manifest.admissible_model_family.as_str(),
            receipt.manifest.admissible_model_family.as_str(),
        )?;
        ensure_same_field(
            "manifest.hidden_size",
            first.manifest.hidden_size.to_string().as_str(),
            receipt.manifest.hidden_size.to_string().as_str(),
        )?;
        ensure_same_field(
            "manifest.vocab_size",
            first.manifest.vocab_size.to_string().as_str(),
            receipt.manifest.vocab_size.to_string().as_str(),
        )?;
        ensure_same_field(
            "manifest.lora_rank",
            first.manifest.lora_rank.to_string().as_str(),
            receipt.manifest.lora_rank.to_string().as_str(),
        )?;
        ensure_same_field(
            "manifest.batch_size",
            first.manifest.batch_size.to_string().as_str(),
            receipt.manifest.batch_size.to_string().as_str(),
        )?;
        ensure_same_field(
            "manifest.sample_count",
            first.manifest.sample_count.to_string().as_str(),
            receipt.manifest.sample_count.to_string().as_str(),
        )?;
    }
    Ok(FirstSwarmOpenAdapterAggregationCompatibility {
        receipt_contract_digest: first.manifest.receipt_contract_digest.clone(),
        shared_replay_identity_digest: first.manifest.shared_replay_identity_digest.clone(),
        contributor_role_ids: receipts
            .iter()
            .map(|receipt| receipt.manifest.contributor_role_id.clone())
            .collect(),
        execution_backend_labels: receipts
            .iter()
            .map(|receipt| receipt.manifest.execution_backend_label.clone())
            .collect(),
        contributor_receipt_digests: receipts
            .iter()
            .map(|receipt| receipt.receipt_digest.clone())
            .collect(),
    })
}

fn sample(
    sample_prefix: &str,
    suffix: &str,
    hidden_state: Vec<f32>,
    target_token_id: u32,
    source_token_count: u32,
) -> Result<OpenAdapterHiddenStateSample, FirstSwarmOpenAdapterReceiptError> {
    OpenAdapterHiddenStateSample::new(
        format!("{sample_prefix}-{suffix}"),
        hidden_state,
        target_token_id,
        source_token_count,
    )
    .map_err(
        |error| FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field: "sample",
            expected: String::from("valid canonical first swarm supervision sample"),
            actual: error.to_string(),
        },
    )
}

fn precision_policy_label(policy: OpenAdapterPrecisionPolicy) -> &'static str {
    match policy {
        OpenAdapterPrecisionPolicy::F32Reference => FIRST_SWARM_OPEN_ADAPTER_PRECISION_POLICY,
        OpenAdapterPrecisionPolicy::Bf16Mixed => "bf16_mixed",
    }
}

fn shared_replay_identity_digest(
    receipt_contract_digest: &str,
    adapter_family: &str,
    adapter_format: &str,
    precision_policy: &str,
) -> String {
    stable_digest(
        b"psionic_first_swarm_open_adapter_replay_identity|",
        &(
            receipt_contract_digest,
            adapter_family,
            adapter_format,
            precision_policy,
        ),
    )
}

fn ensure_contract_field(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), FirstSwarmOpenAdapterReceiptError> {
    if expected != actual {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn ensure_contract_usize(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), FirstSwarmOpenAdapterReceiptError> {
    if expected != actual {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn ensure_contract_f32(
    field: &'static str,
    expected: f32,
    actual: f32,
) -> Result<(), FirstSwarmOpenAdapterReceiptError> {
    if expected != actual {
        return Err(FirstSwarmOpenAdapterReceiptError::ContractMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn ensure_same_field(
    field: &'static str,
    left: &str,
    right: &str,
) -> Result<(), FirstSwarmOpenAdapterReceiptError> {
    if left != right {
        return Err(FirstSwarmOpenAdapterReceiptError::CrossBackendMismatch {
            field,
            left: left.to_string(),
            right: right.to_string(),
        });
    }
    Ok(())
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{FirstSwarmLinuxCudaBringupReport, FirstSwarmMacMlxBringupReport};

    fn receipt_contract_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/swarm/first_swarm_open_adapter_receipt_contract_v1.json")
    }

    fn mac_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/swarm/reports/swarm_mac_mlx_bringup_v1.json")
    }

    fn linux_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/swarm/reports/swarm_linux_rtx4080_bringup_v1.json")
    }

    #[test]
    fn first_swarm_receipt_contract_stays_aligned_with_swarm_contract() {
        let contract = first_swarm_open_adapter_receipt_contract();
        let swarm_contract = first_swarm_run_contract();
        assert_eq!(contract.run_family_id, swarm_contract.run_family_id);
        assert_eq!(
            contract.dataset_manifest_digest,
            swarm_contract.dataset.dataset_manifest_digest
        );
        assert_eq!(
            contract.precision_policy,
            FIRST_SWARM_OPEN_ADAPTER_PRECISION_POLICY
        );
        assert_eq!(contract.hidden_size, FIRST_SWARM_OPEN_ADAPTER_HIDDEN_SIZE);
        assert_eq!(contract.batch_size, FIRST_SWARM_OPEN_ADAPTER_BATCH_SIZE);
        assert_eq!(
            contract.deterministic_probe_target_token_id,
            FIRST_SWARM_OPEN_ADAPTER_PROBE_TARGET_TOKEN_ID
        );
    }

    #[test]
    fn retained_mac_and_linux_contributor_receipts_stay_comparable() {
        let mac_report: FirstSwarmMacMlxBringupReport = serde_json::from_str(
            &fs::read_to_string(mac_fixture_path()).expect("mac fixture should read"),
        )
        .expect("mac fixture should parse");
        let linux_report: FirstSwarmLinuxCudaBringupReport = serde_json::from_str(
            &fs::read_to_string(linux_fixture_path()).expect("linux fixture should read"),
        )
        .expect("linux fixture should parse");
        let mac_receipt = mac_report
            .overfit_gate
            .expect("mac fixture should carry overfit gate")
            .contributor_receipt;
        let linux_receipt = linux_report.parity_harness.contributor_receipt;
        let compatibility = compare_first_swarm_open_adapter_contributor_receipts(&[
            mac_receipt.clone(),
            linux_receipt.clone(),
        ])
        .expect("retained swarm contributor receipts should stay comparable");
        assert_eq!(
            compatibility.receipt_contract_digest,
            mac_receipt.manifest.receipt_contract_digest
        );
        assert_eq!(
            compatibility.shared_replay_identity_digest,
            linux_receipt.manifest.shared_replay_identity_digest
        );
    }

    #[test]
    fn incompatible_precision_policy_is_rejected() {
        let mac_report: FirstSwarmMacMlxBringupReport = serde_json::from_str(
            &fs::read_to_string(mac_fixture_path()).expect("mac fixture should read"),
        )
        .expect("mac fixture should parse");
        let linux_report: FirstSwarmLinuxCudaBringupReport = serde_json::from_str(
            &fs::read_to_string(linux_fixture_path()).expect("linux fixture should read"),
        )
        .expect("linux fixture should parse");
        let mac_receipt = mac_report
            .overfit_gate
            .expect("mac fixture should carry overfit gate")
            .contributor_receipt;
        let mut linux_receipt = linux_report.parity_harness.contributor_receipt;
        linux_receipt.manifest.precision_policy = String::from("bf16_mixed");
        let error =
            compare_first_swarm_open_adapter_contributor_receipts(&[mac_receipt, linux_receipt])
                .expect_err("precision drift must reject aggregation");
        assert!(
            matches!(
                error,
                FirstSwarmOpenAdapterReceiptError::ContractMismatch { field, .. }
                    if field == "manifest.precision_policy"
            ) || matches!(
                error,
                FirstSwarmOpenAdapterReceiptError::CrossBackendMismatch { field, .. }
                    if field == "manifest.precision_policy"
            )
        );
    }

    #[test]
    fn retained_receipt_contract_fixture_matches_builtin_contract() {
        let retained: FirstSwarmOpenAdapterReceiptContract = serde_json::from_str(
            &fs::read_to_string(receipt_contract_fixture_path())
                .expect("retained receipt contract should read"),
        )
        .expect("retained receipt contract should parse");
        assert_eq!(retained, first_swarm_open_adapter_receipt_contract());
    }
}
