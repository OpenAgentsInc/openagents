use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
};
use psionic_core::QuantizationMode;
use psionic_data::{
    LEGAL_BENCHMARK_TRAINING_RECORD_SCHEMA_VERSION, TokenizerDigest, TokenizerFamily,
};
use safetensors::{Dtype as SafeTensorsDType, serialize, tensor::TensorView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY, QWEN_LEGAL_ADAPTER_LORA_ALPHA,
    QWEN_LEGAL_ADAPTER_SFT_LANE_ID, QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST,
    QWEN35_4B_LEGAL_SMOKE_MODEL_ID, QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID,
    QWEN35_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL, QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID,
    QwenLegalAdapterSftConfig, QwenLegalAdapterSftError, QwenLegalAdapterSftRunOutcome,
    QwenLegalAdapterSftRunRequest, QwenLegalAdapterSftTrainer, QwenLegalBaseArtifactMode,
    QwenLegalDatasetBinding, QwenLegalEvalPackBinding, QwenLegalLmHeadSupervisionSample,
    QwenLegalServedBaseModelBinding, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, canonical_qwen_legal_adapter_target_set,
};

/// Lane id for the first local Pylon-network Qwen legal adapter SFT result.
pub const QWEN_LEGAL_PYLON_NETWORK_SFT_LANE_ID: &str = "qwen_legal_pylon_network_sft_v1";
/// Objective id used to replace CS336 as the foreground paid-training target.
pub const QWEN_LEGAL_PYLON_OBJECTIVE_ID: &str = "harvey_legal_qwen_finetune_v1";
/// Report schema for the first Qwen legal Pylon network SFT artifact.
pub const QWEN_LEGAL_PYLON_NETWORK_SFT_REPORT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_pylon_network_sft_report.v1";
/// First aggregation rule for bounded Qwen legal LoRA factor aggregation.
pub const QWEN_LEGAL_PYLON_AGGREGATION_RULE: &str = "trusted_weighted_lora_factor_average_v1";
/// Retained fixture report path.
pub const QWEN_LEGAL_PYLON_NETWORK_SFT_REPORT_PATH: &str =
    "fixtures/qwen_legal/pylon_network_sft/pylon_network_sft_report_v1.json";
/// Retained fixture aggregate adapter path.
pub const QWEN_LEGAL_PYLON_NETWORK_SFT_AGGREGATE_ADAPTER_PATH: &str =
    "fixtures/qwen_legal/pylon_network_sft/aggregate-qwen-legal-lm-head-lora.safetensors";

/// One logical Pylon contributor in the local Qwen legal network SFT smoke.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonContributorSpec {
    /// Stable Pylon node id.
    pub pylon_node_id: String,
    /// Stable node public key used in receipts.
    pub node_pubkey: String,
    /// Stable worker id.
    pub worker_id: String,
    /// Stable assignment id.
    pub assignment_id: String,
    /// Stable contribution id.
    pub contribution_id: String,
    /// Logical shard reference.
    pub shard_ref: String,
    /// Relative adapter artifact path written for this contributor.
    pub adapter_artifact_path: String,
    /// Logical run start timestamp.
    pub started_at_ms: u64,
}

/// Summary of one accepted Qwen legal adapter contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPylonContributionReport {
    /// Contributor identity and assignment.
    pub contributor: QwenLegalPylonContributorSpec,
    /// Sample ids trained by this contributor.
    pub sample_ids: Vec<String>,
    /// Legal training record ids included in this contributor shard.
    pub legal_training_record_ids: Vec<String>,
    /// Number of samples in the shard.
    pub sample_count: usize,
    /// Source-token count carried by the shard.
    pub source_token_count: u32,
    /// Completed adapter SFT steps.
    pub completed_steps: u64,
    /// First recorded training loss in the local run.
    pub first_step_loss: f32,
    /// Final recorded training loss in the local run.
    pub final_step_loss: f32,
    /// Final checkpoint digest.
    pub checkpoint_digest: String,
    /// Exported adapter artifact digest.
    pub adapter_artifact_digest: String,
    /// Exported adapter identity digest.
    pub adapter_identity_digest: String,
    /// Run-summary digest for the local contributor.
    pub run_summary_digest: String,
    /// Stable contribution receipt digest.
    pub contribution_receipt_digest: String,
    /// Whether trusted aggregation accepted this contribution.
    pub accepted_for_aggregation: bool,
}

/// Aggregate adapter output from the bounded Qwen legal network run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalPylonAggregateReport {
    /// Aggregate id.
    pub aggregate_id: String,
    /// Aggregation rule.
    pub aggregation_rule: String,
    /// Accepted contribution count.
    pub accepted_contribution_count: usize,
    /// Model-progress participant count.
    pub model_progress_participant_count: usize,
    /// Total source-token weight.
    pub total_source_token_weight: u32,
    /// Relative aggregate adapter path.
    pub aggregate_adapter_artifact_path: String,
    /// Aggregate adapter artifact digest.
    pub aggregate_adapter_artifact_digest: String,
    /// Aggregate adapter identity digest.
    pub aggregate_adapter_identity_digest: String,
    /// Stable aggregate receipt digest.
    pub aggregate_receipt_digest: String,
}

/// Machine-readable report for the first Qwen legal Pylon network SFT smoke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPylonNetworkSftReport {
    /// Schema version.
    pub schema_version: String,
    /// Stable report id.
    pub report_id: String,
    /// Objective id.
    pub objective_id: String,
    /// Lane id for this network-shaped run.
    pub lane_id: String,
    /// Parent single-process Qwen legal SFT lane id.
    pub parent_lane_id: String,
    /// Public smoke model id.
    pub public_model_id: String,
    /// Retained target model id.
    pub retained_target_model_id: String,
    /// Served model id.
    pub served_model_id: String,
    /// Base artifact mode.
    pub artifact_mode: QwenLegalBaseArtifactMode,
    /// Base artifact digest.
    pub base_served_artifact_digest: String,
    /// Checkpoint family.
    pub checkpoint_family: String,
    /// Dataset digest.
    pub dataset_digest: String,
    /// Eval pack digest.
    pub eval_pack_digest: String,
    /// Claim boundary.
    pub claim_boundary: String,
    /// Contributor reports.
    pub contributions: Vec<QwenLegalPylonContributionReport>,
    /// Aggregate report.
    pub aggregate: QwenLegalPylonAggregateReport,
    /// Stable report digest.
    pub report_digest: String,
}

impl QwenLegalPylonNetworkSftReport {
    /// Returns the stable report digest with the digest field cleared.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_pylon_network_sft_report|", &clone)
    }

    /// Validates the report's self-digest and aggregate/contributor shape.
    pub fn validate(&self) -> Result<(), QwenLegalPylonNetworkSftError> {
        if self.schema_version != QWEN_LEGAL_PYLON_NETWORK_SFT_REPORT_SCHEMA_VERSION {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("schema version drifted"),
            });
        }
        if self.lane_id != QWEN_LEGAL_PYLON_NETWORK_SFT_LANE_ID {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("lane id drifted"),
            });
        }
        if self.parent_lane_id != QWEN_LEGAL_ADAPTER_SFT_LANE_ID {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("parent lane id drifted"),
            });
        }
        if self.contributions.len() < 2 {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("network SFT requires at least two contributors"),
            });
        }
        if self
            .contributions
            .iter()
            .any(|contribution| !contribution.accepted_for_aggregation)
        {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from(
                    "canonical network SFT report requires accepted contributions",
                ),
            });
        }
        if self.aggregate.accepted_contribution_count != self.contributions.len() {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("aggregate contribution count drifted"),
            });
        }
        if self.aggregate.model_progress_participant_count != self.contributions.len() {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("model-progress participant count drifted"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(QwenLegalPylonNetworkSftError::InvalidReport {
                detail: String::from("report digest drifted"),
            });
        }
        Ok(())
    }
}

/// Full in-memory result containing the report plus adapter bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPylonNetworkSftResult {
    /// Machine-readable report.
    pub report: QwenLegalPylonNetworkSftReport,
    /// Aggregate adapter bytes.
    pub aggregate_adapter_bytes: Vec<u8>,
    /// Per-contributor adapter bytes in report order.
    pub contributor_adapter_bytes: Vec<Vec<u8>>,
}

/// Error returned by the Qwen legal Pylon network SFT smoke.
#[derive(Debug, Error)]
pub enum QwenLegalPylonNetworkSftError {
    #[error("Qwen legal Pylon network SFT report is invalid: {detail}")]
    InvalidReport { detail: String },
    #[error("Qwen legal Pylon network SFT aggregation failed: {detail}")]
    Aggregation { detail: String },
    #[error("Qwen legal Pylon network SFT I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen legal Pylon network SFT serialization failed: {message}")]
    Serialization { message: String },
    #[error("Qwen legal Pylon network SFT safetensors export failed: {message}")]
    Safetensors { message: String },
    #[error(transparent)]
    QwenSft(#[from] QwenLegalAdapterSftError),
    #[error(transparent)]
    AdapterLoad(#[from] LmHeadLoraLoadError),
}

/// Runs the canonical two-contributor local Pylon network Qwen legal SFT smoke.
pub fn run_canonical_qwen_legal_pylon_network_sft()
-> Result<QwenLegalPylonNetworkSftResult, QwenLegalPylonNetworkSftError> {
    let base_binding = canonical_base_binding();
    let dataset_binding = canonical_dataset_binding();
    let eval_pack_binding = canonical_eval_pack_binding();
    let all_samples = canonical_supervision_samples();
    let contributors = canonical_contributors();
    let shards = vec![
        vec![
            all_samples[0].clone(),
            all_samples[2].clone(),
            all_samples[4].clone(),
        ],
        vec![
            all_samples[1].clone(),
            all_samples[3].clone(),
            all_samples[5].clone(),
        ],
    ];
    let mut contribution_reports = Vec::new();
    let mut loaded_adapters = Vec::new();
    let mut contributor_adapter_bytes = Vec::new();

    for (index, (contributor, samples)) in contributors.into_iter().zip(shards).enumerate() {
        let request = contributor_request(
            &dataset_binding,
            &eval_pack_binding,
            &contributor,
            index + 1,
        );
        let trainer = QwenLegalAdapterSftTrainer::new(
            contributor_config(&contributor, index + 1)?,
            canonical_qwen_legal_adapter_target_set(),
            base_binding.clone(),
            samples.clone(),
        )?;
        let outcome = trainer.run_sft(&request)?;
        let adapter = outcome.exported_artifact.load_lm_head_lora_artifact()?;
        let report = contribution_report(contributor, samples.as_slice(), &outcome, true)?;
        loaded_adapters.push(adapter);
        contributor_adapter_bytes.push(outcome.exported_artifact.adapter_bytes.clone());
        contribution_reports.push(report);
    }

    let (aggregate_adapter_bytes, aggregate_identity, aggregate_identity_digest) =
        aggregate_lora_adapters(
            loaded_adapters.as_slice(),
            contribution_reports.as_slice(),
            &base_binding,
        )?;
    let aggregate_artifact_digest = sha256_hex(aggregate_adapter_bytes.as_slice());
    let aggregate_receipt_digest = stable_json_digest(
        b"psionic_qwen_legal_pylon_network_sft_aggregate_receipt|",
        &AggregateReceiptDigestInput {
            aggregation_rule: String::from(QWEN_LEGAL_PYLON_AGGREGATION_RULE),
            contribution_receipt_digests: contribution_reports
                .iter()
                .map(|contribution| contribution.contribution_receipt_digest.clone())
                .collect(),
            aggregate_adapter_artifact_digest: aggregate_artifact_digest.clone(),
            aggregate_adapter_identity_digest: aggregate_identity_digest.clone(),
        },
    );
    let total_source_token_weight = contribution_reports
        .iter()
        .map(|contribution| contribution.source_token_count)
        .sum();
    let aggregate = QwenLegalPylonAggregateReport {
        aggregate_id: String::from("aggregate.qwen_legal_pylon_network_sft.000001"),
        aggregation_rule: String::from(QWEN_LEGAL_PYLON_AGGREGATION_RULE),
        accepted_contribution_count: contribution_reports.len(),
        model_progress_participant_count: contribution_reports.len(),
        total_source_token_weight,
        aggregate_adapter_artifact_path: String::from(
            "aggregate-qwen-legal-lm-head-lora.safetensors",
        ),
        aggregate_adapter_artifact_digest: aggregate_artifact_digest,
        aggregate_adapter_identity_digest: aggregate_identity_digest,
        aggregate_receipt_digest,
    };
    let mut report = QwenLegalPylonNetworkSftReport {
        schema_version: String::from(QWEN_LEGAL_PYLON_NETWORK_SFT_REPORT_SCHEMA_VERSION),
        report_id: String::from("qwen_legal_pylon_network_sft_report_000001"),
        objective_id: String::from(QWEN_LEGAL_PYLON_OBJECTIVE_ID),
        lane_id: String::from(QWEN_LEGAL_PYLON_NETWORK_SFT_LANE_ID),
        parent_lane_id: String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID),
        public_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_MODEL_ID),
        retained_target_model_id: String::from(QWEN36_35B_A3B_LEGAL_RETAINED_MODEL_ID),
        served_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID),
        artifact_mode: QwenLegalBaseArtifactMode::SyntheticHiddenStateSmoke,
        base_served_artifact_digest: String::from(QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST),
        checkpoint_family: String::from(QWEN_LEGAL_ADAPTER_CHECKPOINT_FAMILY),
        dataset_digest: dataset_binding.dataset_digest,
        eval_pack_digest: eval_pack_binding.eval_pack_digest,
        claim_boundary: String::from(
            "This is a real multi-contributor LM-head LoRA adapter-SFT artifact over synthetic Qwen legal hidden-state smoke samples. It proves the Pylon-network training and aggregation shape. It does not claim Qwen3.6 full-weight fine-tuning or a score lift on private Harvey benchmark tasks.",
        ),
        contributions: contribution_reports,
        aggregate,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    let aggregate_adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        aggregate_adapter_bytes.as_slice(),
        aggregate_identity,
        QWEN_LEGAL_ADAPTER_LORA_ALPHA,
    )?;
    if aggregate_adapter.identity.artifact_digest
        != report.aggregate.aggregate_adapter_artifact_digest
    {
        return Err(QwenLegalPylonNetworkSftError::Aggregation {
            detail: String::from("aggregate adapter identity digest drifted after reload"),
        });
    }
    Ok(QwenLegalPylonNetworkSftResult {
        report,
        aggregate_adapter_bytes,
        contributor_adapter_bytes,
    })
}

/// Writes the canonical report and adapter artifacts to `output_dir`.
pub fn write_canonical_qwen_legal_pylon_network_sft_fixture(
    output_dir: impl AsRef<Path>,
) -> Result<QwenLegalPylonNetworkSftReport, QwenLegalPylonNetworkSftError> {
    let output_dir = output_dir.as_ref();
    let result = run_canonical_qwen_legal_pylon_network_sft()?;
    fs::create_dir_all(output_dir).map_err(|error| QwenLegalPylonNetworkSftError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;
    for (contribution, bytes) in result
        .report
        .contributions
        .iter()
        .zip(result.contributor_adapter_bytes.iter())
    {
        write_bytes(
            output_dir.join(&contribution.contributor.adapter_artifact_path),
            bytes,
        )?;
    }
    write_bytes(
        output_dir.join(&result.report.aggregate.aggregate_adapter_artifact_path),
        result.aggregate_adapter_bytes.as_slice(),
    )?;
    write_json(
        output_dir.join("pylon_network_sft_report_v1.json"),
        &result.report,
    )?;
    Ok(result.report)
}

fn canonical_base_binding() -> QwenLegalServedBaseModelBinding {
    let template_digest = "sha256:qwen35-legal-template-smoke";
    QwenLegalServedBaseModelBinding {
        public_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_MODEL_ID),
        served_model_id: String::from(QWEN35_4B_LEGAL_SMOKE_SERVED_MODEL_ID),
        model_family_acceptance_label: String::from(QWEN35_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL),
        base_model_revision: String::from("qwen3.5-4b-smoke-revision"),
        base_served_artifact_digest: String::from(QWEN_LEGAL_SYNTHETIC_SMOKE_BASE_ARTIFACT_DIGEST),
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

fn canonical_dataset_binding() -> QwenLegalDatasetBinding {
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

fn canonical_eval_pack_binding() -> QwenLegalEvalPackBinding {
    QwenLegalEvalPackBinding {
        eval_pack_id: String::from("legal-benchmark-retained-smoke"),
        eval_pack_digest: String::from("sha256:legal-retained-smoke-eval-pack"),
        benchmark_suite_id: String::from("harvey-legal-benchmark"),
        retained_slice_id: String::from("retained-smoke"),
        scorer_version: String::from("psionic-legal-scorer.v1"),
        import_target: String::from("autopilot4://legal-benchmark/runs"),
    }
}

fn canonical_supervision_samples() -> Vec<QwenLegalLmHeadSupervisionSample> {
    vec![
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-a",
            vec![1.0, 0.0, 0.0, 0.0],
            12,
            41,
            "legal-record-a",
        ),
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-b",
            vec![0.0, 1.0, 0.0, 0.0],
            35,
            39,
            "legal-record-b",
        ),
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-c",
            vec![0.0, 0.0, 1.0, 0.0],
            62,
            47,
            "legal-record-c",
        ),
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-d",
            vec![0.0, 0.0, 0.0, 1.0],
            90,
            44,
            "legal-record-d",
        ),
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-e",
            vec![0.6, 0.4, 0.0, 0.0],
            118,
            52,
            "legal-record-e",
        ),
        QwenLegalLmHeadSupervisionSample::new(
            "legal-network-f",
            vec![0.0, 0.2, 0.5, 0.3],
            143,
            49,
            "legal-record-f",
        ),
    ]
}

fn canonical_contributors() -> Vec<QwenLegalPylonContributorSpec> {
    vec![
        QwenLegalPylonContributorSpec {
            pylon_node_id: String::from("pylon.local.legal.cuda.01"),
            node_pubkey: String::from("npub1qwenlegalcuda000000000000000000000000000000000001"),
            worker_id: String::from("worker.qwen-legal.cuda.01"),
            assignment_id: String::from("assignment.qwen-legal-pylon-sft.000001.a"),
            contribution_id: String::from("contribution.qwen-legal-pylon-sft.000001.a"),
            shard_ref: String::from("shard://harvey-legal-smoke/train/even"),
            adapter_artifact_path: String::from(
                "contributor-pylon-local-legal-cuda-01.safetensors",
            ),
            started_at_ms: 2_000,
        },
        QwenLegalPylonContributorSpec {
            pylon_node_id: String::from("pylon.local.legal.metal.01"),
            node_pubkey: String::from("npub1qwenlegalmetal00000000000000000000000000000000001"),
            worker_id: String::from("worker.qwen-legal.metal.01"),
            assignment_id: String::from("assignment.qwen-legal-pylon-sft.000001.b"),
            contribution_id: String::from("contribution.qwen-legal-pylon-sft.000001.b"),
            shard_ref: String::from("shard://harvey-legal-smoke/train/odd"),
            adapter_artifact_path: String::from(
                "contributor-pylon-local-legal-metal-01.safetensors",
            ),
            started_at_ms: 2_500,
        },
    ]
}

fn contributor_config(
    _contributor: &QwenLegalPylonContributorSpec,
    index: usize,
) -> Result<QwenLegalAdapterSftConfig, QwenLegalPylonNetworkSftError> {
    Ok(QwenLegalAdapterSftConfig {
        run_id: format!("qwen-legal-pylon-network-sft-contributor-{index}"),
        budget: TrainingLoopBudget::new(4, 1, 1).map_err(QwenLegalAdapterSftError::from)?,
        batch_size: 2,
        optimizer: TrainingOptimizerConfig::adamw(0.12, 0.9, 0.99, 1e-8)
            .with_gradient_clip_norm(1.0),
        optimizer_residency_policy: TrainingOptimizerResidencyPolicy::host_only(),
    })
}

fn contributor_request(
    dataset_binding: &QwenLegalDatasetBinding,
    eval_pack_binding: &QwenLegalEvalPackBinding,
    contributor: &QwenLegalPylonContributorSpec,
    index: usize,
) -> QwenLegalAdapterSftRunRequest {
    QwenLegalAdapterSftRunRequest {
        dataset_binding: dataset_binding.clone(),
        eval_pack_binding: eval_pack_binding.clone(),
        validator_policy_ref: String::from("policy://validator/legal-benchmark/qwen-pylon-smoke"),
        adapter_id: String::from("qwen35-4b-legal-pylon-network-smoke"),
        adapter_revision: format!("r1-contributor-{index}"),
        started_at_ms: contributor.started_at_ms,
        step_duration_ms: 20,
    }
}

fn contribution_report(
    contributor: QwenLegalPylonContributorSpec,
    samples: &[QwenLegalLmHeadSupervisionSample],
    outcome: &QwenLegalAdapterSftRunOutcome,
    accepted_for_aggregation: bool,
) -> Result<QwenLegalPylonContributionReport, QwenLegalPylonNetworkSftError> {
    let Some(first_receipt) = outcome.step_receipts.first() else {
        return Err(QwenLegalPylonNetworkSftError::InvalidReport {
            detail: format!(
                "contribution `{}` emitted no step receipts",
                contributor.contribution_id
            ),
        });
    };
    let Some(final_receipt) = outcome.step_receipts.last() else {
        return Err(QwenLegalPylonNetworkSftError::InvalidReport {
            detail: format!(
                "contribution `{}` emitted no final receipt",
                contributor.contribution_id
            ),
        });
    };
    let run_summary_digest = stable_json_digest(
        b"psionic_qwen_legal_pylon_network_sft_contributor_run_summary|",
        &outcome.summary.run_summary,
    );
    let source_token_count = samples.iter().map(|sample| sample.source_token_count).sum();
    let sample_ids = samples
        .iter()
        .map(|sample| sample.sample_id.clone())
        .collect::<Vec<_>>();
    let legal_training_record_ids = samples
        .iter()
        .map(|sample| sample.legal_training_record_id.clone())
        .collect::<Vec<_>>();
    let contribution_receipt_digest = stable_json_digest(
        b"psionic_qwen_legal_pylon_network_sft_contribution_receipt|",
        &ContributionReceiptDigestInput {
            contribution_id: contributor.contribution_id.clone(),
            worker_id: contributor.worker_id.clone(),
            node_pubkey: contributor.node_pubkey.clone(),
            shard_ref: contributor.shard_ref.clone(),
            sample_ids: sample_ids.clone(),
            source_token_count,
            adapter_artifact_digest: outcome.exported_artifact.adapter_artifact_digest.clone(),
            checkpoint_digest: outcome.final_checkpoint.checkpoint_digest.clone(),
            run_summary_digest: run_summary_digest.clone(),
            accepted_for_aggregation,
        },
    );
    Ok(QwenLegalPylonContributionReport {
        contributor,
        sample_ids,
        legal_training_record_ids,
        sample_count: samples.len(),
        source_token_count,
        completed_steps: outcome.summary.run_summary.completed_steps,
        first_step_loss: first_receipt.loss,
        final_step_loss: final_receipt.loss,
        checkpoint_digest: outcome.final_checkpoint.checkpoint_digest.clone(),
        adapter_artifact_digest: outcome.exported_artifact.adapter_artifact_digest.clone(),
        adapter_identity_digest: outcome.exported_artifact.adapter_identity_digest.clone(),
        run_summary_digest,
        contribution_receipt_digest,
        accepted_for_aggregation,
    })
}

fn aggregate_lora_adapters(
    adapters: &[LmHeadLoraAdapterArtifact],
    contributions: &[QwenLegalPylonContributionReport],
    base_binding: &QwenLegalServedBaseModelBinding,
) -> Result<(Vec<u8>, AdapterArtifactIdentity, String), QwenLegalPylonNetworkSftError> {
    let Some(first) = adapters.first() else {
        return Err(QwenLegalPylonNetworkSftError::Aggregation {
            detail: String::from("no adapters supplied for aggregation"),
        });
    };
    if adapters.len() != contributions.len() {
        return Err(QwenLegalPylonNetworkSftError::Aggregation {
            detail: String::from("adapter and contribution counts differ"),
        });
    }
    let mut total_weight = 0.0_f32;
    let mut lora_a = vec![0.0_f32; first.lora_a().len()];
    let mut lora_b = vec![0.0_f32; first.lora_b().len()];
    for (adapter, contribution) in adapters.iter().zip(contributions.iter()) {
        if adapter.rank != first.rank
            || adapter.hidden_size != first.hidden_size
            || adapter.vocab_size != first.vocab_size
        {
            return Err(QwenLegalPylonNetworkSftError::Aggregation {
                detail: String::from("adapter shapes differ across contributors"),
            });
        }
        let weight = contribution.source_token_count.max(1) as f32;
        total_weight += weight;
        for (target, value) in lora_a.iter_mut().zip(adapter.lora_a().iter()) {
            *target += *value * weight;
        }
        for (target, value) in lora_b.iter_mut().zip(adapter.lora_b().iter()) {
            *target += *value * weight;
        }
    }
    if total_weight <= 0.0 {
        return Err(QwenLegalPylonNetworkSftError::Aggregation {
            detail: String::from("aggregate source-token weight must be positive"),
        });
    }
    for value in &mut lora_a {
        *value /= total_weight;
    }
    for value in &mut lora_b {
        *value /= total_weight;
    }
    let adapter_bytes = export_lora_safetensors(
        &lora_a,
        &lora_b,
        first.rank,
        first.hidden_size,
        first.vocab_size,
    )?;
    let artifact_digest = sha256_hex(adapter_bytes.as_slice());
    let identity = AdapterArtifactIdentity::new(
        "qwen35-4b-legal-pylon-network-smoke",
        "r1-trusted-aggregate",
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        base_binding.public_model_id.clone(),
        base_binding.base_model_revision.clone(),
        base_binding.base_served_artifact_digest.clone(),
        artifact_digest,
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        u64::try_from(
            first
                .rank
                .saturating_mul(first.hidden_size + first.vocab_size),
        )
        .unwrap_or(u64::MAX),
    )
    .with_provenance_digest(stable_json_digest(
        b"psionic_qwen_legal_pylon_network_sft_aggregate_provenance|",
        &contributions
            .iter()
            .map(|contribution| contribution.contribution_receipt_digest.clone())
            .collect::<Vec<_>>(),
    ))
    .with_governance_digest(stable_json_digest(
        b"psionic_qwen_legal_pylon_network_sft_aggregate_governance|",
        &QWEN_LEGAL_PYLON_AGGREGATION_RULE,
    ));
    let identity_digest = identity.stable_digest();
    Ok((adapter_bytes, identity, identity_digest))
}

fn export_lora_safetensors(
    lora_a: &[f32],
    lora_b: &[f32],
    rank: usize,
    hidden_size: usize,
    vocab_size: usize,
) -> Result<Vec<u8>, QwenLegalPylonNetworkSftError> {
    let raw_a = encode_f32_bytes(lora_a);
    let raw_b = encode_f32_bytes(lora_b);
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![rank, hidden_size],
        raw_a.as_slice(),
    )
    .map_err(|error| QwenLegalPylonNetworkSftError::Safetensors {
        message: error.to_string(),
    })?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, rank],
        raw_b.as_slice(),
    )
    .map_err(|error| QwenLegalPylonNetworkSftError::Safetensors {
        message: error.to_string(),
    })?;
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from("openagents.aggregate_manifest"),
        String::from(QWEN_LEGAL_PYLON_AGGREGATION_RULE),
    );
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        Some(metadata),
    )
    .map_err(|error| QwenLegalPylonNetworkSftError::Safetensors {
        message: error.to_string(),
    })
}

fn write_bytes(path: PathBuf, bytes: &[u8]) -> Result<(), QwenLegalPylonNetworkSftError> {
    fs::write(&path, bytes).map_err(|error| QwenLegalPylonNetworkSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json(
    path: PathBuf,
    report: &QwenLegalPylonNetworkSftReport,
) -> Result<(), QwenLegalPylonNetworkSftError> {
    let json = serde_json::to_vec_pretty(report).map_err(|error| {
        QwenLegalPylonNetworkSftError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(&path, json).map_err(|error| QwenLegalPylonNetworkSftError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_json_digest(prefix: &[u8], payload: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ContributionReceiptDigestInput {
    contribution_id: String,
    worker_id: String,
    node_pubkey: String,
    shard_ref: String,
    sample_ids: Vec<String>,
    source_token_count: u32,
    adapter_artifact_digest: String,
    checkpoint_digest: String,
    run_summary_digest: String,
    accepted_for_aggregation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct AggregateReceiptDigestInput {
    aggregation_rule: String,
    contribution_receipt_digests: Vec<String>,
    aggregate_adapter_artifact_digest: String,
    aggregate_adapter_identity_digest: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_legal_pylon_network_sft_emits_two_contributor_aggregate()
    -> Result<(), Box<dyn std::error::Error>> {
        let result = run_canonical_qwen_legal_pylon_network_sft()?;
        assert_eq!(result.report.objective_id, QWEN_LEGAL_PYLON_OBJECTIVE_ID);
        assert_eq!(result.report.contributions.len(), 2);
        assert_eq!(result.report.aggregate.accepted_contribution_count, 2);
        assert_eq!(result.report.aggregate.model_progress_participant_count, 2);
        assert_ne!(
            result.report.contributions[0].adapter_artifact_digest,
            result.report.contributions[1].adapter_artifact_digest
        );
        assert!(!result.aggregate_adapter_bytes.is_empty());
        result.report.validate()?;
        Ok(())
    }

    #[test]
    fn qwen_legal_pylon_network_sft_fixture_writes_loadable_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let report = write_canonical_qwen_legal_pylon_network_sft_fixture(dir.path())?;
        let aggregate_path = dir
            .path()
            .join(&report.aggregate.aggregate_adapter_artifact_path);
        assert!(aggregate_path.is_file());
        let bytes = fs::read(aggregate_path)?;
        assert_eq!(
            sha256_hex(bytes.as_slice()),
            report.aggregate.aggregate_adapter_artifact_digest
        );
        for contribution in &report.contributions {
            assert!(
                dir.path()
                    .join(&contribution.contributor.adapter_artifact_path)
                    .is_file()
            );
        }
        report.validate()?;
        Ok(())
    }
}
