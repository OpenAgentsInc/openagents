//! Deterministic Qwen legal LoRA adapter aggregation for Pylon workers.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact, LmHeadLoraLoadError,
};
use psionic_core::QuantizationMode;
use psionic_eval::{run_legal_benchmark_eval_suite, LegalBenchmarkEvalSuiteRunConfig};
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::QWEN_LEGAL_ADAPTER_LORA_ALPHA;

/// Manifest schema for deterministic legal LoRA merge jobs.
pub const QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_lora_merge_manifest.v1";
/// Receipt schema emitted by `psionic-train merge-lora`.
pub const QWEN_LEGAL_LORA_MERGE_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_lora_merge_receipt.v1";
/// Quarantine record schema emitted before rejecting incompatible worker artifacts.
pub const QWEN_LEGAL_LORA_QUARANTINE_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_lora_quarantine_record.v1";
/// Deterministic delta-averaging rule for worker LoRA factors.
pub const QWEN_LEGAL_LORA_DELTA_AVERAGE_RULE: &str = "trusted_weighted_lora_factor_average_v1";
/// Deterministic sequential shard handoff rule.
pub const QWEN_LEGAL_LORA_SEQUENTIAL_RULE: &str = "trusted_sequential_lora_shard_handoff_v1";

/// Merge strategy for distributed worker adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalLoraMergeMode {
    /// Workers train from the same parent adapter and the aggregator averages deltas by token count.
    DeltaAveraging,
    /// Workers train one after another, with each worker continuing from the previous adapter.
    ShardSequentialTraining,
}

impl QwenLegalLoraMergeMode {
    fn aggregation_rule(self) -> &'static str {
        match self {
            Self::DeltaAveraging => QWEN_LEGAL_LORA_DELTA_AVERAGE_RULE,
            Self::ShardSequentialTraining => QWEN_LEGAL_LORA_SEQUENTIAL_RULE,
        }
    }
}

/// Base model binding shared by all worker adapters in a merge job.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeBaseModel {
    /// Public model id the adapter targets.
    pub base_model_id: String,
    /// Base model revision the adapter targets.
    pub base_model_revision: String,
    /// Stable digest for the served base artifact.
    pub base_served_artifact_digest: String,
}

/// One worker-produced LoRA adapter input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraWorkerAdapterInput {
    /// Stable worker id.
    pub worker_id: String,
    /// Stable adapter id written by the worker.
    pub adapter_id: String,
    /// Stable adapter revision written by the worker.
    pub adapter_revision: String,
    /// Adapter artifact path.
    pub path: String,
    /// Expected artifact SHA-256 digest.
    pub sha256: String,
    /// Dataset shard hash trained by this worker.
    pub dataset_shard_hash: String,
    /// Training-token count used as the merge weight.
    pub token_count: u64,
    /// Parent adapter hash observed by this worker before training.
    pub parent_adapter_sha256: String,
    /// Strict compatibility facts for real distributed Qwen aggregation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<QwenLegalLoraWorkerCompatibilityFacts>,
    /// Optional validator replay result for this worker update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_replay: Option<QwenLegalLoraValidatorReplayClaim>,
}

/// Shared compatibility contract for one distributed Qwen aggregation window.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeCompatibilityContract {
    /// Base checkpoint hash all workers must train from.
    pub base_checkpoint_sha256: String,
    /// Tokenizer hash all workers must use.
    pub tokenizer_sha256: String,
    /// Config hash all workers must use.
    pub config_sha256: String,
    /// Global corpus manifest hash.
    pub corpus_manifest_sha256: String,
    /// Exact target module set.
    pub target_modules: Vec<String>,
    /// Optimizer config hash.
    pub optimizer_config_sha256: String,
    /// Precision policy, such as bf16-lora-f32-master.
    pub precision_policy: String,
    /// Stable training window id.
    pub step_window_id: String,
}

/// Worker-declared compatibility facts for one update.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraWorkerCompatibilityFacts {
    /// Base checkpoint hash seen by the worker.
    pub base_checkpoint_sha256: String,
    /// Tokenizer hash seen by the worker.
    pub tokenizer_sha256: String,
    /// Config hash seen by the worker.
    pub config_sha256: String,
    /// Global corpus manifest hash seen by the worker.
    pub corpus_manifest_sha256: String,
    /// Concrete corpus shard hash trained by this worker.
    pub corpus_shard_hash: String,
    /// Exact target module set trained by this worker.
    pub target_modules: Vec<String>,
    /// Optimizer config hash used by the worker.
    pub optimizer_config_sha256: String,
    /// Precision policy used by the worker.
    pub precision_policy: String,
    /// Stable training window id used by the worker.
    pub step_window_id: String,
}

/// Validator replay status for a distributed worker update.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalLoraValidatorReplayStatus {
    /// No validator replay was requested for this contribution.
    NotRequested,
    /// Validator replay accepted this contribution.
    Passed,
    /// Validator replay rejected this contribution.
    Failed,
}

/// Worker validator replay claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraValidatorReplayClaim {
    /// Replay status.
    pub status: QwenLegalLoraValidatorReplayStatus,
    /// Replay receipt hash, when a replay ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_receipt_sha256: Option<String>,
    /// Plain reason retained for audit.
    pub reason: String,
}

/// Output adapter artifact declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeOutput {
    /// Stable aggregate adapter id.
    pub adapter_id: String,
    /// Stable aggregate adapter revision.
    pub adapter_revision: String,
    /// Output artifact path.
    pub path: String,
    /// Optional expected output artifact hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
}

/// Optional local eval and promotion-gate declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeValidation {
    /// Legal benchmark eval suite path.
    pub suite_path: String,
    /// Base model id used for the eval comparison.
    pub base_model: String,
    /// Eval output directory.
    pub output_dir: String,
    /// Current champion adapter id, if known.
    pub champion_adapter_id: String,
    /// Champion score in basis points on this same suite.
    pub champion_score_bps: u32,
}

/// Manifest consumed by `psionic-train merge-lora`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeManifest {
    /// Schema version.
    pub schema_version: String,
    /// Stable merge id.
    pub merge_id: String,
    /// Merge strategy.
    pub mode: QwenLegalLoraMergeMode,
    /// Parent adapter hash all independent workers started from, or the first parent in sequential mode.
    pub parent_adapter_sha256: String,
    /// Shared base model binding.
    pub base_model: QwenLegalLoraMergeBaseModel,
    /// Output artifact declaration.
    pub output_adapter: QwenLegalLoraMergeOutput,
    /// Worker adapters to merge.
    pub worker_adapters: Vec<QwenLegalLoraWorkerAdapterInput>,
    /// Strict compatibility contract for real distributed Qwen aggregation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<QwenLegalLoraMergeCompatibilityContract>,
    /// Optional local eval gate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<QwenLegalLoraMergeValidation>,
}

/// One worker row in the compatibility matrix.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraCompatibilityMatrixRow {
    /// Worker id.
    pub worker_id: String,
    /// Adapter artifact hash.
    pub adapter_sha256: String,
    /// Dataset shard hash.
    pub dataset_shard_hash: String,
    /// Whether this contribution passed all compatibility checks.
    pub accepted: bool,
    /// Human-readable rejection or audit reasons.
    pub reasons: Vec<String>,
    /// Base checkpoint compatibility.
    pub base_checkpoint_match: bool,
    /// Tokenizer compatibility.
    pub tokenizer_match: bool,
    /// Config compatibility.
    pub config_match: bool,
    /// Global corpus manifest compatibility.
    pub corpus_manifest_match: bool,
    /// Corpus shard declaration compatibility.
    pub corpus_shard_match: bool,
    /// Target module set compatibility.
    pub target_modules_match: bool,
    /// Optimizer compatibility.
    pub optimizer_config_match: bool,
    /// Precision compatibility.
    pub precision_policy_match: bool,
    /// Training window compatibility.
    pub step_window_match: bool,
    /// Duplicate contribution status.
    pub duplicate_contribution: bool,
    /// Validator replay status.
    pub validator_replay_status: QwenLegalLoraValidatorReplayStatus,
}

/// Rejected worker contribution retained in receipts or errors.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraRejectedContribution {
    /// Worker id.
    pub worker_id: String,
    /// Adapter hash.
    pub adapter_sha256: String,
    /// Rejection reasons.
    pub reasons: Vec<String>,
}

/// Retained debug record for one rejected worker artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraQuarantineRecord {
    /// Schema version.
    pub schema_version: String,
    /// Merge id that rejected this artifact.
    pub merge_id: String,
    /// Worker id.
    pub worker_id: String,
    /// Adapter artifact path as declared by the worker.
    pub adapter_path: String,
    /// Adapter artifact hash.
    pub adapter_sha256: String,
    /// Dataset shard hash.
    pub dataset_shard_hash: String,
    /// Retention class.
    pub retention_class: String,
    /// Whether validator replay is required before this artifact can be reconsidered.
    pub replay_required: bool,
    /// Rejection reasons.
    pub reasons: Vec<String>,
    /// Stable quarantine digest.
    pub quarantine_digest: String,
}

impl QwenLegalLoraQuarantineRecord {
    fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.quarantine_digest.clear();
        stable_json_digest("psionic.qwen_legal_lora_quarantine_record.v1", &clone)
    }
}

/// One worker's verified contribution inside the merge receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeWorkerReceipt {
    /// Stable worker id.
    pub worker_id: String,
    /// Adapter id.
    pub adapter_id: String,
    /// Adapter revision.
    pub adapter_revision: String,
    /// Adapter artifact path.
    pub adapter_path: String,
    /// Verified worker adapter hash.
    pub adapter_sha256: String,
    /// Dataset shard hash.
    pub dataset_shard_hash: String,
    /// Token count.
    pub token_count: u64,
    /// Merge weight, after token-count normalization.
    pub merge_weight: f64,
    /// Parent adapter hash declared by the worker.
    pub parent_adapter_sha256: String,
}

/// Local eval summary attached to a merge receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeValidationReceipt {
    /// Eval suite id.
    pub suite_id: String,
    /// Eval suite hash.
    pub suite_hash: String,
    /// Eval output directory.
    pub output_dir: String,
    /// Base score in basis points.
    pub base_score_bps: u32,
    /// Candidate adapter score in basis points.
    pub adapter_score_bps: u32,
    /// Candidate score delta in basis points.
    pub score_delta_bps: i32,
    /// Candidate answer-file success rate.
    pub answer_file_success_rate_bps: u32,
    /// Candidate integrity failures.
    pub integrity_failure_count: u64,
    /// Candidate tool failures.
    pub tool_failure_count: u64,
    /// Candidate timeouts.
    pub timeout_failure_count: u64,
    /// Local eval report hash.
    pub report_hash: String,
}

/// Promotion gate decision for a merged adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalLoraMergePromotionDecision {
    /// Candidate beats champion and has no hard eval failures.
    Promote,
    /// Candidate is valid but does not beat champion.
    Hold,
    /// Candidate has hard eval failures.
    Reject,
}

/// Promotion-gate receipt. It does not mutate the adapter registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergePromotionGateReceipt {
    /// Champion adapter id.
    pub champion_adapter_id: String,
    /// Candidate adapter id.
    pub candidate_adapter_id: String,
    /// Champion score in basis points.
    pub champion_score_bps: u32,
    /// Candidate score in basis points.
    pub candidate_score_bps: u32,
    /// Candidate minus champion score.
    pub score_delta_bps: i32,
    /// Promotion decision.
    pub decision: QwenLegalLoraMergePromotionDecision,
    /// Reasons for the decision.
    pub reasons: Vec<String>,
}

/// Receipt emitted by a completed merge job.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalLoraMergeReceipt {
    /// Schema version.
    pub schema_version: String,
    /// Stable merge id.
    pub merge_id: String,
    /// Merge mode.
    pub mode: QwenLegalLoraMergeMode,
    /// Aggregation rule.
    pub aggregation_rule: String,
    /// Manifest path.
    pub manifest_path: String,
    /// Manifest hash.
    pub manifest_hash: String,
    /// Parent adapter hash.
    pub parent_adapter_sha256: String,
    /// Worker receipts.
    pub workers: Vec<QwenLegalLoraMergeWorkerReceipt>,
    /// Strict compatibility matrix for all worker updates.
    pub compatibility_matrix: Vec<QwenLegalLoraCompatibilityMatrixRow>,
    /// Contributions rejected before aggregation. Successful receipts keep this empty.
    pub rejected_contributions: Vec<QwenLegalLoraRejectedContribution>,
    /// Total token count across accepted worker adapters.
    pub total_token_count: u64,
    /// Output adapter path.
    pub output_adapter_path: String,
    /// Output adapter SHA-256 digest.
    pub output_adapter_sha256: String,
    /// Output adapter identity digest.
    pub output_adapter_identity_digest: String,
    /// Local eval receipt, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<QwenLegalLoraMergeValidationReceipt>,
    /// Promotion gate receipt, when local eval was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotion_gate: Option<QwenLegalLoraMergePromotionGateReceipt>,
    /// Promotion candidate pointer usable by registry, eval, and settlement checks.
    pub promotion_candidate_pointer: String,
    /// Deterministic replay command.
    pub deterministic_merge_command: Vec<String>,
    /// Stable receipt hash.
    pub receipt_hash: String,
}

impl QwenLegalLoraMergeReceipt {
    fn stable_hash(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_hash.clear();
        stable_json_digest("psionic.qwen_legal_lora_merge_receipt.v1", &clone)
    }
}

/// Error returned by deterministic Qwen legal LoRA merge jobs.
#[derive(Debug, Error)]
pub enum QwenLegalLoraMergeError {
    #[error("merge-lora usage: psionic-train merge-lora --manifest <manifest.json>")]
    Usage,
    #[error("invalid merge manifest: {0}")]
    InvalidManifest(String),
    #[error("distributed Qwen compatibility rejected {rejected_count} contribution(s): {reasons}")]
    CompatibilityRejected {
        rejected_count: usize,
        reasons: String,
    },
    #[error("input hash mismatch for `{path}`: expected {expected}, got {actual}")]
    InputHash {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("output hash mismatch for `{path}`: expected {expected}, got {actual}")]
    OutputHash {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("safetensors export failed: {0}")]
    Safetensors(String),
    #[error("adapter load failed: {0}")]
    AdapterLoad(#[from] LmHeadLoraLoadError),
    #[error("local eval failed: {0}")]
    Eval(#[from] psionic_eval::LegalBenchmarkEvalSuiteError),
}

/// Runs the `merge-lora` CLI path.
pub fn run_psionic_legal_merge_lora_cli(
    args: &[String],
) -> Result<QwenLegalLoraMergeReceipt, QwenLegalLoraMergeError> {
    let manifest_path = parse_manifest_path(args)?;
    run_qwen_legal_lora_merge_manifest(manifest_path)
}

/// Runs one manifest-driven deterministic LoRA merge.
pub fn run_qwen_legal_lora_merge_manifest(
    manifest_path: impl AsRef<Path>,
) -> Result<QwenLegalLoraMergeReceipt, QwenLegalLoraMergeError> {
    let manifest_path = manifest_path.as_ref();
    let manifest_bytes = read_file(manifest_path)?;
    let manifest = serde_json::from_slice::<QwenLegalLoraMergeManifest>(&manifest_bytes)?;
    validate_manifest(&manifest)?;
    let (compatibility_matrix, rejected_contributions) = compatibility_preflight(&manifest);
    if !rejected_contributions.is_empty() {
        write_quarantine_records(manifest_path, &manifest, rejected_contributions.as_slice())?;
        return Err(QwenLegalLoraMergeError::CompatibilityRejected {
            rejected_count: rejected_contributions.len(),
            reasons: rejected_contributions
                .iter()
                .flat_map(|rejection| {
                    rejection
                        .reasons
                        .iter()
                        .map(|reason| format!("{}: {reason}", rejection.worker_id))
                })
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    let manifest_hash = stable_json_digest("psionic.qwen_legal_lora_merge_manifest.v1", &manifest);
    let loaded = load_worker_adapters(manifest_path, &manifest)?;
    let (output_bytes, output_identity, worker_receipts, total_token_count) =
        merge_loaded_adapters(&manifest, loaded.as_slice())?;
    let output_path = resolve_output_path(manifest.output_adapter.path.as_str());
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| QwenLegalLoraMergeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&output_path, output_bytes.as_slice()).map_err(|source| {
        QwenLegalLoraMergeError::Io {
            path: output_path.clone(),
            source,
        }
    })?;
    let output_adapter_sha256 = sha256_hex(output_bytes.as_slice());
    if let Some(expected) = &manifest.output_adapter.expected_sha256 {
        if expected != &output_adapter_sha256 {
            return Err(QwenLegalLoraMergeError::OutputHash {
                path: output_path.display().to_string(),
                expected: expected.clone(),
                actual: output_adapter_sha256,
            });
        }
    }
    let deterministic_merge_command = vec![
        String::from("cargo"),
        String::from("run"),
        String::from("-p"),
        String::from("psionic-train"),
        String::from("--"),
        String::from("merge-lora"),
        String::from("--manifest"),
        manifest_path.display().to_string(),
    ];
    let validation = run_validation(
        manifest.validation.as_ref(),
        output_path.as_path(),
        deterministic_merge_command.as_slice(),
    )?;
    let promotion_gate = validation.as_ref().and_then(|validation_receipt| {
        manifest.validation.as_ref().map(|validation_manifest| {
            promotion_gate_receipt(
                manifest.output_adapter.adapter_id.as_str(),
                validation_manifest,
                validation_receipt,
            )
        })
    });
    let mut receipt = QwenLegalLoraMergeReceipt {
        schema_version: String::from(QWEN_LEGAL_LORA_MERGE_RECEIPT_SCHEMA_VERSION),
        merge_id: manifest.merge_id.clone(),
        mode: manifest.mode,
        aggregation_rule: String::from(manifest.mode.aggregation_rule()),
        manifest_path: manifest_path.display().to_string(),
        manifest_hash,
        parent_adapter_sha256: manifest.parent_adapter_sha256.clone(),
        workers: worker_receipts,
        compatibility_matrix,
        rejected_contributions,
        total_token_count,
        output_adapter_path: output_path.display().to_string(),
        output_adapter_sha256: output_adapter_sha256.clone(),
        output_adapter_identity_digest: output_identity.stable_digest(),
        validation,
        promotion_gate,
        promotion_candidate_pointer: format!(
            "adapter://{}@{}#{}",
            manifest.output_adapter.adapter_id,
            manifest.output_adapter.adapter_revision,
            output_adapter_sha256
        ),
        deterministic_merge_command,
        receipt_hash: String::new(),
    };
    receipt.receipt_hash = receipt.stable_hash();
    let receipt_path = output_path.with_extension("merge-receipt.json");
    write_file_pretty(receipt_path.as_path(), &receipt)?;
    Ok(receipt)
}

#[derive(Clone)]
struct LoadedWorkerAdapter {
    input: QwenLegalLoraWorkerAdapterInput,
    bytes: Vec<u8>,
    adapter: LmHeadLoraAdapterArtifact,
}

fn parse_manifest_path(args: &[String]) -> Result<PathBuf, QwenLegalLoraMergeError> {
    args.windows(2)
        .find_map(|window| (window[0] == "--manifest").then(|| PathBuf::from(&window[1])))
        .ok_or(QwenLegalLoraMergeError::Usage)
}

fn validate_manifest(manifest: &QwenLegalLoraMergeManifest) -> Result<(), QwenLegalLoraMergeError> {
    if manifest.schema_version != QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION {
        return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
            "schema_version must be {QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION}"
        )));
    }
    if manifest.merge_id.trim().is_empty() {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "merge_id must not be empty",
        )));
    }
    if !is_complete_sha256(manifest.parent_adapter_sha256.as_str()) {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "parent_adapter_sha256 must be a 64-character SHA-256 hex digest",
        )));
    }
    if manifest.worker_adapters.len() < 2 {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "at least two worker adapters are required",
        )));
    }
    if manifest.output_adapter.adapter_id.trim().is_empty()
        || manifest.output_adapter.adapter_revision.trim().is_empty()
        || manifest.output_adapter.path.trim().is_empty()
    {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "output adapter id, revision, and path must be present",
        )));
    }
    if let Some(expected) = &manifest.output_adapter.expected_sha256 {
        if !is_complete_sha256(expected) {
            return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
                "output expected_sha256 must be a 64-character SHA-256 hex digest",
            )));
        }
    }
    for worker in &manifest.worker_adapters {
        if worker.worker_id.trim().is_empty()
            || worker.adapter_id.trim().is_empty()
            || worker.adapter_revision.trim().is_empty()
            || worker.path.trim().is_empty()
            || worker.dataset_shard_hash.trim().is_empty()
        {
            return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
                "worker id, adapter id, revision, path, and dataset shard hash must be present",
            )));
        }
        if worker.token_count == 0 {
            return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                "worker `{}` token_count must be > 0",
                worker.worker_id
            )));
        }
        if !is_complete_sha256(worker.sha256.as_str()) {
            return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                "worker `{}` sha256 must be a 64-character SHA-256 hex digest",
                worker.worker_id
            )));
        }
        if !is_complete_sha256(worker.parent_adapter_sha256.as_str()) {
            return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                "worker `{}` parent_adapter_sha256 must be a 64-character SHA-256 hex digest",
                worker.worker_id
            )));
        }
        if let Some(replay) = &worker.validator_replay {
            if replay.status != QwenLegalLoraValidatorReplayStatus::NotRequested
                && replay
                    .replay_receipt_sha256
                    .as_deref()
                    .is_some_and(|hash| !is_complete_sha256(hash))
            {
                return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                    "worker `{}` validator replay receipt hash must be a 64-character SHA-256 hex digest",
                    worker.worker_id
                )));
            }
        }
    }
    if let Some(contract) = &manifest.compatibility {
        validate_compatibility_contract(contract)?;
        for worker in &manifest.worker_adapters {
            let Some(facts) = &worker.compatibility else {
                return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                    "worker `{}` is missing compatibility facts",
                    worker.worker_id
                )));
            };
            validate_worker_compatibility_facts(worker.worker_id.as_str(), facts)?;
        }
    }
    match manifest.mode {
        QwenLegalLoraMergeMode::DeltaAveraging => {
            for worker in &manifest.worker_adapters {
                if worker.parent_adapter_sha256 != manifest.parent_adapter_sha256 {
                    return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                        "worker `{}` did not train from the merge parent adapter",
                        worker.worker_id
                    )));
                }
            }
        }
        QwenLegalLoraMergeMode::ShardSequentialTraining => {
            let mut expected_parent = manifest.parent_adapter_sha256.as_str();
            for worker in &manifest.worker_adapters {
                if worker.parent_adapter_sha256 != expected_parent {
                    return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                        "sequential worker `{}` parent hash mismatch",
                        worker.worker_id
                    )));
                }
                expected_parent = worker.sha256.as_str();
            }
        }
    }
    Ok(())
}

fn validate_compatibility_contract(
    contract: &QwenLegalLoraMergeCompatibilityContract,
) -> Result<(), QwenLegalLoraMergeError> {
    for (field, value) in [
        (
            "base_checkpoint_sha256",
            contract.base_checkpoint_sha256.as_str(),
        ),
        ("tokenizer_sha256", contract.tokenizer_sha256.as_str()),
        ("config_sha256", contract.config_sha256.as_str()),
        (
            "corpus_manifest_sha256",
            contract.corpus_manifest_sha256.as_str(),
        ),
        (
            "optimizer_config_sha256",
            contract.optimizer_config_sha256.as_str(),
        ),
    ] {
        if !is_complete_sha256(value) {
            return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                "compatibility.{field} must be a 64-character SHA-256 hex digest"
            )));
        }
    }
    if contract.target_modules.is_empty()
        || contract.precision_policy.trim().is_empty()
        || contract.step_window_id.trim().is_empty()
    {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "compatibility target_modules, precision_policy, and step_window_id must be present",
        )));
    }
    Ok(())
}

fn validate_worker_compatibility_facts(
    worker_id: &str,
    facts: &QwenLegalLoraWorkerCompatibilityFacts,
) -> Result<(), QwenLegalLoraMergeError> {
    for (field, value) in [
        (
            "base_checkpoint_sha256",
            facts.base_checkpoint_sha256.as_str(),
        ),
        ("tokenizer_sha256", facts.tokenizer_sha256.as_str()),
        ("config_sha256", facts.config_sha256.as_str()),
        (
            "corpus_manifest_sha256",
            facts.corpus_manifest_sha256.as_str(),
        ),
        (
            "optimizer_config_sha256",
            facts.optimizer_config_sha256.as_str(),
        ),
    ] {
        if !is_complete_sha256(value) {
            return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
                "worker `{worker_id}` compatibility.{field} must be a 64-character SHA-256 hex digest"
            )));
        }
    }
    if facts.corpus_shard_hash.trim().is_empty()
        || facts.target_modules.is_empty()
        || facts.precision_policy.trim().is_empty()
        || facts.step_window_id.trim().is_empty()
    {
        return Err(QwenLegalLoraMergeError::InvalidManifest(format!(
            "worker `{worker_id}` compatibility corpus_shard_hash, target_modules, precision_policy, and step_window_id must be present"
        )));
    }
    Ok(())
}

fn compatibility_preflight(
    manifest: &QwenLegalLoraMergeManifest,
) -> (
    Vec<QwenLegalLoraCompatibilityMatrixRow>,
    Vec<QwenLegalLoraRejectedContribution>,
) {
    let mut seen_worker_ids = BTreeSet::new();
    let mut seen_adapter_hashes = BTreeSet::new();
    let mut seen_dataset_shards = BTreeSet::new();
    let mut rows = Vec::with_capacity(manifest.worker_adapters.len());
    let mut rejected = Vec::new();

    for worker in &manifest.worker_adapters {
        let duplicate_contribution = !seen_worker_ids.insert(worker.worker_id.clone())
            || !seen_adapter_hashes.insert(worker.sha256.clone())
            || !seen_dataset_shards.insert(worker.dataset_shard_hash.clone());
        let replay_status = worker
            .validator_replay
            .as_ref()
            .map(|replay| replay.status)
            .unwrap_or(QwenLegalLoraValidatorReplayStatus::NotRequested);
        let mut reasons = Vec::new();
        if duplicate_contribution {
            reasons.push(String::from(
                "duplicate worker id, adapter hash, or dataset shard hash",
            ));
        }
        if replay_status == QwenLegalLoraValidatorReplayStatus::Failed {
            reasons.push(String::from("validator replay rejected contribution"));
        }

        let mut base_checkpoint_match = true;
        let mut tokenizer_match = true;
        let mut config_match = true;
        let mut corpus_manifest_match = true;
        let mut corpus_shard_match = true;
        let mut target_modules_match = true;
        let mut optimizer_config_match = true;
        let mut precision_policy_match = true;
        let mut step_window_match = true;

        if let Some(contract) = &manifest.compatibility {
            if let Some(facts) = &worker.compatibility {
                base_checkpoint_match =
                    facts.base_checkpoint_sha256 == contract.base_checkpoint_sha256;
                tokenizer_match = facts.tokenizer_sha256 == contract.tokenizer_sha256;
                config_match = facts.config_sha256 == contract.config_sha256;
                corpus_manifest_match =
                    facts.corpus_manifest_sha256 == contract.corpus_manifest_sha256;
                corpus_shard_match = facts.corpus_shard_hash == worker.dataset_shard_hash;
                target_modules_match = sorted_strings(&facts.target_modules)
                    == sorted_strings(&contract.target_modules);
                optimizer_config_match =
                    facts.optimizer_config_sha256 == contract.optimizer_config_sha256;
                precision_policy_match = facts.precision_policy == contract.precision_policy;
                step_window_match = facts.step_window_id == contract.step_window_id;
            } else {
                base_checkpoint_match = false;
                tokenizer_match = false;
                config_match = false;
                corpus_manifest_match = false;
                corpus_shard_match = false;
                target_modules_match = false;
                optimizer_config_match = false;
                precision_policy_match = false;
                step_window_match = false;
                reasons.push(String::from("missing worker compatibility facts"));
            }
        }

        for (matched, reason) in [
            (base_checkpoint_match, "base checkpoint mismatch"),
            (tokenizer_match, "tokenizer mismatch"),
            (config_match, "config mismatch"),
            (corpus_manifest_match, "corpus manifest mismatch"),
            (corpus_shard_match, "corpus shard mismatch"),
            (target_modules_match, "target module set mismatch"),
            (optimizer_config_match, "optimizer config mismatch"),
            (precision_policy_match, "precision policy mismatch"),
            (step_window_match, "step window mismatch"),
        ] {
            if !matched {
                reasons.push(String::from(reason));
            }
        }

        let accepted = reasons.is_empty();
        let row = QwenLegalLoraCompatibilityMatrixRow {
            worker_id: worker.worker_id.clone(),
            adapter_sha256: worker.sha256.clone(),
            dataset_shard_hash: worker.dataset_shard_hash.clone(),
            accepted,
            reasons: reasons.clone(),
            base_checkpoint_match,
            tokenizer_match,
            config_match,
            corpus_manifest_match,
            corpus_shard_match,
            target_modules_match,
            optimizer_config_match,
            precision_policy_match,
            step_window_match,
            duplicate_contribution,
            validator_replay_status: replay_status,
        };
        if !accepted {
            rejected.push(QwenLegalLoraRejectedContribution {
                worker_id: worker.worker_id.clone(),
                adapter_sha256: worker.sha256.clone(),
                reasons,
            });
        }
        rows.push(row);
    }

    (rows, rejected)
}

fn write_quarantine_records(
    manifest_path: &Path,
    manifest: &QwenLegalLoraMergeManifest,
    rejected: &[QwenLegalLoraRejectedContribution],
) -> Result<(), QwenLegalLoraMergeError> {
    let records = rejected
        .iter()
        .map(|rejection| {
            let worker = manifest
                .worker_adapters
                .iter()
                .find(|worker| {
                    worker.worker_id == rejection.worker_id
                        && worker.sha256 == rejection.adapter_sha256
                })
                .or_else(|| {
                    manifest
                        .worker_adapters
                        .iter()
                        .find(|worker| worker.worker_id == rejection.worker_id)
                });
            let mut record = QwenLegalLoraQuarantineRecord {
                schema_version: String::from(QWEN_LEGAL_LORA_QUARANTINE_SCHEMA_VERSION),
                merge_id: manifest.merge_id.clone(),
                worker_id: rejection.worker_id.clone(),
                adapter_path: worker
                    .map(|worker| worker.path.clone())
                    .unwrap_or_else(|| String::from("unknown")),
                adapter_sha256: rejection.adapter_sha256.clone(),
                dataset_shard_hash: worker
                    .map(|worker| worker.dataset_shard_hash.clone())
                    .unwrap_or_default(),
                retention_class: String::from("debug_only_not_aggregation_eligible"),
                replay_required: true,
                reasons: rejection.reasons.clone(),
                quarantine_digest: String::new(),
            };
            record.quarantine_digest = record.stable_digest();
            record
        })
        .collect::<Vec<_>>();
    write_file_pretty(
        manifest_path.with_extension("quarantine.json").as_path(),
        &records,
    )
}

fn sorted_strings(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values
}

fn load_worker_adapters(
    manifest_path: &Path,
    manifest: &QwenLegalLoraMergeManifest,
) -> Result<Vec<LoadedWorkerAdapter>, QwenLegalLoraMergeError> {
    let mut workers = manifest.worker_adapters.clone();
    if manifest.mode == QwenLegalLoraMergeMode::DeltaAveraging {
        workers.sort_by(|left, right| {
            left.worker_id
                .cmp(&right.worker_id)
                .then(left.adapter_id.cmp(&right.adapter_id))
                .then(left.sha256.cmp(&right.sha256))
        });
    }
    workers
        .into_iter()
        .map(|worker| {
            let path = resolve_input_path(manifest_path, worker.path.as_str());
            let bytes = read_file(path.as_path())?;
            let actual = sha256_hex(bytes.as_slice());
            if actual != worker.sha256 {
                return Err(QwenLegalLoraMergeError::InputHash {
                    path: path.display().to_string(),
                    expected: worker.sha256,
                    actual,
                });
            }
            let identity = adapter_identity_for_hash(
                &manifest.base_model,
                worker.adapter_id.clone(),
                worker.adapter_revision.clone(),
                actual,
                0,
            );
            let adapter = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
                bytes.as_slice(),
                identity,
                QWEN_LEGAL_ADAPTER_LORA_ALPHA,
            )?;
            Ok(LoadedWorkerAdapter {
                input: worker,
                bytes,
                adapter,
            })
        })
        .collect()
}

fn merge_loaded_adapters(
    manifest: &QwenLegalLoraMergeManifest,
    loaded: &[LoadedWorkerAdapter],
) -> Result<
    (
        Vec<u8>,
        AdapterArtifactIdentity,
        Vec<QwenLegalLoraMergeWorkerReceipt>,
        u64,
    ),
    QwenLegalLoraMergeError,
> {
    let total_token_count = loaded
        .iter()
        .map(|worker| worker.input.token_count)
        .sum::<u64>();
    let workers = loaded
        .iter()
        .map(|worker| QwenLegalLoraMergeWorkerReceipt {
            worker_id: worker.input.worker_id.clone(),
            adapter_id: worker.input.adapter_id.clone(),
            adapter_revision: worker.input.adapter_revision.clone(),
            adapter_path: worker.input.path.clone(),
            adapter_sha256: worker.input.sha256.clone(),
            dataset_shard_hash: worker.input.dataset_shard_hash.clone(),
            token_count: worker.input.token_count,
            merge_weight: worker.input.token_count as f64 / total_token_count as f64,
            parent_adapter_sha256: worker.input.parent_adapter_sha256.clone(),
        })
        .collect::<Vec<_>>();
    let output_bytes = match manifest.mode {
        QwenLegalLoraMergeMode::DeltaAveraging => merge_delta_average(loaded, total_token_count)?,
        QwenLegalLoraMergeMode::ShardSequentialTraining => loaded
            .last()
            .map(|worker| worker.bytes.clone())
            .ok_or_else(|| {
                QwenLegalLoraMergeError::InvalidManifest(String::from(
                    "sequential merge had no worker adapters",
                ))
            })?,
    };
    let output_hash = sha256_hex(output_bytes.as_slice());
    let first = &loaded[0].adapter;
    let parameter_count = u64::try_from(
        first
            .rank
            .saturating_mul(first.hidden_size.saturating_add(first.vocab_size)),
    )
    .unwrap_or(u64::MAX);
    let identity = adapter_identity_for_hash(
        &manifest.base_model,
        manifest.output_adapter.adapter_id.clone(),
        manifest.output_adapter.adapter_revision.clone(),
        output_hash,
        parameter_count,
    )
    .with_provenance_digest(stable_json_digest(
        "psionic.qwen_legal_lora_merge.provenance.v1",
        &workers,
    ))
    .with_governance_digest(stable_json_digest(
        "psionic.qwen_legal_lora_merge.governance.v1",
        &manifest.mode.aggregation_rule(),
    ));
    let reloaded = LmHeadLoraAdapterArtifact::from_safetensors_bytes(
        output_bytes.as_slice(),
        identity.clone(),
        QWEN_LEGAL_ADAPTER_LORA_ALPHA,
    )?;
    if reloaded.rank != first.rank
        || reloaded.hidden_size != first.hidden_size
        || reloaded.vocab_size != first.vocab_size
    {
        return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
            "output adapter shape changed after reload",
        )));
    }
    Ok((output_bytes, identity, workers, total_token_count))
}

fn merge_delta_average(
    loaded: &[LoadedWorkerAdapter],
    total_token_count: u64,
) -> Result<Vec<u8>, QwenLegalLoraMergeError> {
    let first = &loaded[0].adapter;
    let mut lora_a = vec![0.0_f32; first.lora_a().len()];
    let mut lora_b = vec![0.0_f32; first.lora_b().len()];
    for worker in loaded {
        let adapter = &worker.adapter;
        if adapter.rank != first.rank
            || adapter.hidden_size != first.hidden_size
            || adapter.vocab_size != first.vocab_size
        {
            return Err(QwenLegalLoraMergeError::InvalidManifest(String::from(
                "worker adapter shapes differ",
            )));
        }
        let weight = worker.input.token_count as f32;
        for (target, value) in lora_a.iter_mut().zip(adapter.lora_a().iter()) {
            *target += *value * weight;
        }
        for (target, value) in lora_b.iter_mut().zip(adapter.lora_b().iter()) {
            *target += *value * weight;
        }
    }
    let total_weight = total_token_count as f32;
    for value in &mut lora_a {
        *value /= total_weight;
    }
    for value in &mut lora_b {
        *value /= total_weight;
    }
    export_lora_safetensors(
        &lora_a,
        &lora_b,
        first.rank,
        first.hidden_size,
        first.vocab_size,
    )
}

fn export_lora_safetensors(
    lora_a: &[f32],
    lora_b: &[f32],
    rank: usize,
    hidden_size: usize,
    vocab_size: usize,
) -> Result<Vec<u8>, QwenLegalLoraMergeError> {
    let raw_a = encode_f32_bytes(lora_a);
    let raw_b = encode_f32_bytes(lora_b);
    let view_a = TensorView::new(
        SafeTensorsDType::F32,
        vec![rank, hidden_size],
        raw_a.as_slice(),
    )
    .map_err(|error| QwenLegalLoraMergeError::Safetensors(error.to_string()))?;
    let view_b = TensorView::new(
        SafeTensorsDType::F32,
        vec![vocab_size, rank],
        raw_b.as_slice(),
    )
    .map_err(|error| QwenLegalLoraMergeError::Safetensors(error.to_string()))?;
    let mut metadata = HashMap::new();
    metadata.insert(
        String::from("openagents.aggregate_manifest"),
        String::from(QWEN_LEGAL_LORA_DELTA_AVERAGE_RULE),
    );
    serialize(
        [
            ("lm_head.lora_A.weight", view_a),
            ("lm_head.lora_B.weight", view_b),
        ],
        Some(metadata),
    )
    .map_err(|error| QwenLegalLoraMergeError::Safetensors(error.to_string()))
}

fn run_validation(
    validation: Option<&QwenLegalLoraMergeValidation>,
    output_adapter_path: &Path,
    deterministic_merge_command: &[String],
) -> Result<Option<QwenLegalLoraMergeValidationReceipt>, QwenLegalLoraMergeError> {
    let Some(validation) = validation else {
        return Ok(None);
    };
    let output_dir = resolve_output_path(validation.output_dir.as_str());
    let suite_path = resolve_output_path(validation.suite_path.as_str());
    let report = run_legal_benchmark_eval_suite(&LegalBenchmarkEvalSuiteRunConfig {
        suite_path,
        base_model: validation.base_model.clone(),
        adapter: output_adapter_path.display().to_string(),
        output_dir: output_dir.clone(),
        replay_command: deterministic_merge_command.to_vec(),
    })?;
    Ok(Some(QwenLegalLoraMergeValidationReceipt {
        suite_id: report.suite_id,
        suite_hash: report.suite_hash,
        output_dir: output_dir.display().to_string(),
        base_score_bps: report.base_model_result.legal_score_bps,
        adapter_score_bps: report.adapter_result.legal_score_bps,
        score_delta_bps: report.comparison.score_delta_bps,
        answer_file_success_rate_bps: report.adapter_result.answer_file_success_rate_bps,
        integrity_failure_count: report.adapter_result.integrity_failure_count,
        tool_failure_count: report.adapter_result.tool_failure_count,
        timeout_failure_count: report.adapter_result.timeout_failure_count,
        report_hash: report.replay_receipt.report_hash,
    }))
}

fn promotion_gate_receipt(
    candidate_adapter_id: &str,
    validation_manifest: &QwenLegalLoraMergeValidation,
    validation: &QwenLegalLoraMergeValidationReceipt,
) -> QwenLegalLoraMergePromotionGateReceipt {
    let score_delta_bps = i32::try_from(validation.adapter_score_bps).unwrap_or(i32::MAX)
        - i32::try_from(validation_manifest.champion_score_bps).unwrap_or(i32::MAX);
    let mut reasons = Vec::new();
    let hard_failures = validation.integrity_failure_count
        + validation.tool_failure_count
        + validation.timeout_failure_count;
    let decision = if hard_failures > 0 {
        reasons.push(String::from(
            "candidate has integrity, tool, or timeout failures",
        ));
        QwenLegalLoraMergePromotionDecision::Reject
    } else if validation.adapter_score_bps <= validation_manifest.champion_score_bps {
        reasons.push(String::from("candidate score does not beat champion"));
        QwenLegalLoraMergePromotionDecision::Hold
    } else {
        reasons.push(String::from(
            "candidate beats champion on the same local eval suite",
        ));
        QwenLegalLoraMergePromotionDecision::Promote
    };
    QwenLegalLoraMergePromotionGateReceipt {
        champion_adapter_id: validation_manifest.champion_adapter_id.clone(),
        candidate_adapter_id: candidate_adapter_id.to_owned(),
        champion_score_bps: validation_manifest.champion_score_bps,
        candidate_score_bps: validation.adapter_score_bps,
        score_delta_bps,
        decision,
        reasons,
    }
}

fn adapter_identity_for_hash(
    base_model: &QwenLegalLoraMergeBaseModel,
    adapter_id: String,
    adapter_revision: String,
    artifact_digest: String,
    parameter_count: u64,
) -> AdapterArtifactIdentity {
    AdapterArtifactIdentity::new(
        adapter_id,
        adapter_revision,
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        base_model.base_model_id.clone(),
        base_model.base_model_revision.clone(),
        base_model.base_served_artifact_digest.clone(),
        artifact_digest,
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        parameter_count,
    )
}

fn resolve_input_path(manifest_path: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() || path.exists() {
        path
    } else {
        manifest_path
            .parent()
            .map(|parent| parent.join(&path))
            .unwrap_or(path)
    }
}

fn resolve_output_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

fn read_file(path: &Path) -> Result<Vec<u8>, QwenLegalLoraMergeError> {
    fs::read(path).map_err(|source| QwenLegalLoraMergeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file_pretty<T>(path: &Path, value: &T) -> Result<(), QwenLegalLoraMergeError>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|source| QwenLegalLoraMergeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_json_digest<T>(namespace: &str, value: &T) -> String
where
    T: Serialize,
{
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update(b"|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn is_complete_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO_PARENT: &str = "821e206ee5e1b9bd158faf1ff8dc2faa0cbafa3cc7767eef77eaffd44d9a009d";
    const CUDA_SHA: &str = "5298c62de7f6b318f8889957433fe401ba693911997c1c606ce5099e73ca41cb";
    const METAL_SHA: &str = "6e8b6da6083606cbd36105a65d532e48e067094809f9db552ca9a9d13105af9b";
    const AGGREGATE_SHA: &str = "8e8dea3bc639ed2c147d6901f6ceda9b5f1a176034dc7bb65219daf7dd33116d";
    const BASE_CHECKPOINT_SHA: &str =
        "8b0a0f5dbe6f8167f92943da022510cda118f64b8dd84c8f0db0d6acef934c67";
    const TOKENIZER_SHA: &str = "0c451f3bcf463f0e614b62e7816948c2122813e51d88f0e3e0e4fe58fdf2d7c0";
    const CONFIG_SHA: &str = "23a420499cc74ad251c80de0dff8a1af3f872f99d435828ac52161e5505e90b9";
    const CORPUS_MANIFEST_SHA: &str =
        "ea6cd51dbf44853c2215c73b56e49b9f71052b646af77ceaa7653cc880b5659d";
    const OPTIMIZER_SHA: &str = "35e41a47cfbaf3211bd9234f6517920100e8e1178f85827b2a4534978be577b7";

    #[test]
    fn merge_lora_delta_averaging_is_deterministic() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.output_adapter.path = temp
            .path()
            .join("aggregate.safetensors")
            .display()
            .to_string();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let first = run_qwen_legal_lora_merge_manifest(&manifest_path)?;
        let second = run_qwen_legal_lora_merge_manifest(&manifest_path)?;
        assert_eq!(first.output_adapter_sha256, AGGREGATE_SHA);
        assert_eq!(second.output_adapter_sha256, AGGREGATE_SHA);
        assert_eq!(first.output_adapter_sha256, second.output_adapter_sha256);
        assert_eq!(first.workers.len(), 2);
        assert_eq!(first.total_token_count, 272);
        assert_eq!(first.compatibility_matrix.len(), 2);
        assert!(first.compatibility_matrix.iter().all(|row| row.accepted));
        assert!(first.rejected_contributions.is_empty());
        assert!(first.promotion_candidate_pointer.contains(AGGREGATE_SHA));
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_worker_hash_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[0].sha256 =
            "0000000000000000000000000000000000000000000000000000000000000000".into();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("bad worker hash must fail");
        assert!(error.to_string().contains("input hash mismatch"));
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_base_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[0]
            .compatibility
            .as_mut()
            .expect("compatibility")
            .base_checkpoint_sha256 =
            "0000000000000000000000000000000000000000000000000000000000000000".into();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("base mismatch must fail");
        assert!(error.to_string().contains("base checkpoint mismatch"));
        let records = serde_json::from_slice::<Vec<QwenLegalLoraQuarantineRecord>>(&fs::read(
            manifest_path.with_extension("quarantine.json"),
        )?)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].worker_id, manifest.worker_adapters[0].worker_id);
        assert_eq!(
            records[0].retention_class,
            "debug_only_not_aggregation_eligible"
        );
        assert!(records[0].replay_required);
        assert_eq!(records[0].quarantine_digest, records[0].stable_digest());
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_corpus_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[0]
            .compatibility
            .as_mut()
            .expect("compatibility")
            .corpus_manifest_sha256 =
            "1111111111111111111111111111111111111111111111111111111111111111".into();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("corpus mismatch must fail");
        assert!(error.to_string().contains("corpus manifest mismatch"));
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_target_module_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[0]
            .compatibility
            .as_mut()
            .expect("compatibility")
            .target_modules
            .push(String::from("lm_head.extra"));
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("target mismatch must fail");
        assert!(error.to_string().contains("target module set mismatch"));
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_duplicate_contribution() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[1].dataset_shard_hash =
            manifest.worker_adapters[0].dataset_shard_hash.clone();
        manifest.worker_adapters[1]
            .compatibility
            .as_mut()
            .expect("compatibility")
            .corpus_shard_hash = manifest.worker_adapters[0].dataset_shard_hash.clone();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("duplicate contribution must fail");
        assert!(error.to_string().contains("duplicate worker id"));
        Ok(())
    }

    #[test]
    fn merge_lora_rejects_failed_validator_replay() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.worker_adapters[0].validator_replay = Some(QwenLegalLoraValidatorReplayClaim {
            status: QwenLegalLoraValidatorReplayStatus::Failed,
            replay_receipt_sha256: Some(
                "2222222222222222222222222222222222222222222222222222222222222222".into(),
            ),
            reason: String::from("validator replay loss diverged"),
        });
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let error = run_qwen_legal_lora_merge_manifest(&manifest_path)
            .expect_err("failed replay must fail");
        assert!(error.to_string().contains("validator replay rejected"));
        Ok(())
    }

    #[test]
    fn merge_lora_sequential_mode_uses_last_adapter_and_validates_chain(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest_path = temp.path().join("merge.json");
        let mut manifest = fixture_manifest(temp.path());
        manifest.mode = QwenLegalLoraMergeMode::ShardSequentialTraining;
        manifest.output_adapter.path = temp.path().join("last.safetensors").display().to_string();
        manifest.output_adapter.expected_sha256 = Some(String::from(METAL_SHA));
        manifest.worker_adapters[1].parent_adapter_sha256 = String::from(CUDA_SHA);
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        let receipt = run_qwen_legal_lora_merge_manifest(&manifest_path)?;
        assert_eq!(receipt.aggregation_rule, QWEN_LEGAL_LORA_SEQUENTIAL_RULE);
        assert_eq!(receipt.output_adapter_sha256, METAL_SHA);
        Ok(())
    }

    #[test]
    fn promotion_gate_holds_non_improving_candidate() {
        let validation_manifest = QwenLegalLoraMergeValidation {
            suite_path: String::from("suite.json"),
            base_model: String::from("Qwen/Qwen3.6-27B"),
            output_dir: String::from("target/legal/eval"),
            champion_adapter_id: String::from("champion"),
            champion_score_bps: 9_000,
        };
        let validation = QwenLegalLoraMergeValidationReceipt {
            suite_id: String::from("suite"),
            suite_hash: String::from("hash"),
            output_dir: String::from("target/legal/eval"),
            base_score_bps: 3_000,
            adapter_score_bps: 9_000,
            score_delta_bps: 6_000,
            answer_file_success_rate_bps: 10_000,
            integrity_failure_count: 0,
            tool_failure_count: 0,
            timeout_failure_count: 0,
            report_hash: String::from("report"),
        };
        let receipt = promotion_gate_receipt("candidate", &validation_manifest, &validation);
        assert_eq!(receipt.decision, QwenLegalLoraMergePromotionDecision::Hold);
        assert!(receipt
            .reasons
            .iter()
            .any(|reason| reason.contains("does not beat")));
    }

    fn fixture_manifest(temp: &Path) -> QwenLegalLoraMergeManifest {
        QwenLegalLoraMergeManifest {
            schema_version: String::from(QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION),
            merge_id: String::from("legal-sft-round-001"),
            mode: QwenLegalLoraMergeMode::DeltaAveraging,
            parent_adapter_sha256: String::from(ZERO_PARENT),
            base_model: QwenLegalLoraMergeBaseModel {
                base_model_id: String::from("Qwen/Qwen3.5-4B"),
                base_model_revision: String::from("qwen3.5-4b-smoke-revision"),
                base_served_artifact_digest: String::from("sha256:synthetic-qwen35-4b-legal-smoke"),
            },
            output_adapter: QwenLegalLoraMergeOutput {
                adapter_id: String::from("qwen35-4b-legal-pylon-network-smoke"),
                adapter_revision: String::from("r1-trusted-aggregate"),
                path: temp.join("aggregate.safetensors").display().to_string(),
                expected_sha256: Some(String::from(AGGREGATE_SHA)),
            },
            worker_adapters: vec![
                QwenLegalLoraWorkerAdapterInput {
                    worker_id: String::from("worker.qwen-legal.cuda.01"),
                    adapter_id: String::from("qwen35-4b-legal-pylon-network-smoke"),
                    adapter_revision: String::from("r1-contributor-1"),
                    path: fixture_path("contributor-pylon-local-legal-cuda-01.safetensors"),
                    sha256: String::from(CUDA_SHA),
                    dataset_shard_hash: String::from("shard://harvey-legal-smoke/train/even"),
                    token_count: 140,
                    parent_adapter_sha256: String::from(ZERO_PARENT),
                    compatibility: Some(worker_compat("shard://harvey-legal-smoke/train/even")),
                    validator_replay: Some(QwenLegalLoraValidatorReplayClaim {
                        status: QwenLegalLoraValidatorReplayStatus::Passed,
                        replay_receipt_sha256: Some(
                            "9af4ddf49df707f8d6df8bad92fb2610f8f5fe1f1b9ca18d21f685064fd82620"
                                .into(),
                        ),
                        reason: String::from("deterministic replay accepted"),
                    }),
                },
                QwenLegalLoraWorkerAdapterInput {
                    worker_id: String::from("worker.qwen-legal.metal.01"),
                    adapter_id: String::from("qwen35-4b-legal-pylon-network-smoke"),
                    adapter_revision: String::from("r1-contributor-2"),
                    path: fixture_path("contributor-pylon-local-legal-metal-01.safetensors"),
                    sha256: String::from(METAL_SHA),
                    dataset_shard_hash: String::from("shard://harvey-legal-smoke/train/odd"),
                    token_count: 132,
                    parent_adapter_sha256: String::from(ZERO_PARENT),
                    compatibility: Some(worker_compat("shard://harvey-legal-smoke/train/odd")),
                    validator_replay: Some(QwenLegalLoraValidatorReplayClaim {
                        status: QwenLegalLoraValidatorReplayStatus::Passed,
                        replay_receipt_sha256: Some(
                            "4bef1d9d4b8827f4dd3a03aa921a098f384da0d21b9383cc9b4eb7bbab5c5c02"
                                .into(),
                        ),
                        reason: String::from("deterministic replay accepted"),
                    }),
                },
            ],
            compatibility: Some(compatibility_contract()),
            validation: None,
        }
    }

    fn compatibility_contract() -> QwenLegalLoraMergeCompatibilityContract {
        QwenLegalLoraMergeCompatibilityContract {
            base_checkpoint_sha256: String::from(BASE_CHECKPOINT_SHA),
            tokenizer_sha256: String::from(TOKENIZER_SHA),
            config_sha256: String::from(CONFIG_SHA),
            corpus_manifest_sha256: String::from(CORPUS_MANIFEST_SHA),
            target_modules: target_modules(),
            optimizer_config_sha256: String::from(OPTIMIZER_SHA),
            precision_policy: String::from("bf16-lora-f32-master"),
            step_window_id: String::from("qwen-legal-window-000001"),
        }
    }

    fn worker_compat(corpus_shard_hash: &str) -> QwenLegalLoraWorkerCompatibilityFacts {
        QwenLegalLoraWorkerCompatibilityFacts {
            base_checkpoint_sha256: String::from(BASE_CHECKPOINT_SHA),
            tokenizer_sha256: String::from(TOKENIZER_SHA),
            config_sha256: String::from(CONFIG_SHA),
            corpus_manifest_sha256: String::from(CORPUS_MANIFEST_SHA),
            corpus_shard_hash: String::from(corpus_shard_hash),
            target_modules: target_modules(),
            optimizer_config_sha256: String::from(OPTIMIZER_SHA),
            precision_policy: String::from("bf16-lora-f32-master"),
            step_window_id: String::from("qwen-legal-window-000001"),
        }
    }

    fn target_modules() -> Vec<String> {
        vec![
            String::from("lm_head.lora_A.weight"),
            String::from("lm_head.lora_B.weight"),
        ]
    }

    fn fixture_path(file: &str) -> String {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/qwen_legal/pylon_network_sft")
            .join(file)
            .display()
            .to_string()
    }
}
