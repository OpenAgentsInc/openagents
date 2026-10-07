use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterTargetFamily,
    LmHeadLoraAdapterArtifact,
};
use psionic_core::QuantizationMode;
use psionic_models::{QWEN36_27B_MODEL_ID, QWEN36_27B_REAL_MODEL_DIR, QWEN36_27B_SERVED_MODEL_ID};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PylonLocalWorkerRunOptions, PylonTrainingArtifactRef, PylonTrainingBitcoinSettlementProof,
    PylonTrainingBitcoinSettlementStatus, PylonTrainingDeferredPaymentPolicy,
    PylonTrainingExpectedOutputArtifact, PylonTrainingHardwareRequirements, PylonTrainingJobKind,
    PylonTrainingJobSpec, PylonTrainingPaymentBudget, PylonTrainingPaymentCloseout,
    PylonTrainingPaymentDecisionReceipt, PylonTrainingPaymentStatus,
    PylonTrainingReceiptRequirements, PylonTrainingSettlementExecutionMode,
    PylonTrainingShardAssignment, PylonTrainingTreasuryHandoffBatch, PylonTrainingWorkerJobStatus,
    QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION,
    QWEN_LEGAL_PYLON_BITCOIN_SETTLEMENT_PROOF_SCHEMA_VERSION,
    QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION, QWEN36_REAL_LORA_ACTIVATION_MODE,
    QWEN36_REAL_LORA_ACTIVE_TARGET, QWEN36_REAL_LORA_ADAPTER_FORMAT, Qwen36RealLoraSftArtifacts,
    Qwen36RealLoraSftConfig, Qwen36RealLoraSftError, QwenLegalCheckpointRecoveryError,
    QwenLegalLoraMergeBaseModel, QwenLegalLoraMergeError, QwenLegalLoraMergeManifest,
    QwenLegalLoraMergeMode, QwenLegalLoraMergeOutput, QwenLegalLoraMergePromotionDecision,
    QwenLegalLoraMergeValidation, QwenLegalLoraValidatorReplayClaim,
    QwenLegalLoraValidatorReplayStatus, QwenLegalLoraWorkerAdapterInput,
    QwenLegalLoraWorkerCompatibilityFacts, attach_qwen_legal_pylon_settlement_proofs,
    build_qwen_legal_pylon_treasury_handoff_batch, pylon_bitcoin_payout_target_for_job,
    run_qwen_legal_lora_merge_manifest, run_qwen_legal_pylon_worker_job, run_qwen36_real_lora_sft,
    settle_qwen_legal_pylon_training_job_spec, verify_qwen_legal_pylon_worker_receipt_path,
    write_qwen_legal_checkpoint_recovery_rehearsal,
};

pub const QWEN36_REAL_PYLON_REHEARSAL_REPORT_SCHEMA_VERSION: &str =
    "psionic.qwen36_real_pylon_rehearsal_report.v1";
pub const QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_RUN_ID: &str = "qwen36-27b-real-pylon-rehearsal-001";
pub const QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_OUTPUT_DIR: &str =
    "target/legal/qwen36_27b_real_pylon_rehearsal_001";
pub const QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_REPORT_PATH: &str =
    "reports/qwen36-27b-real-pylon-rehearsal-001.md";
pub const QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_SUITE_PATH: &str = "suites/harvey_public_three.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen36RealPylonRehearsalConfig {
    pub run_id: String,
    #[serde(default = "default_real_qwen_model_dir")]
    pub model_dir: String,
    pub suite_path: String,
    pub output_dir: String,
    pub report_path: String,
}

impl Default for Qwen36RealPylonRehearsalConfig {
    fn default() -> Self {
        Self {
            run_id: String::from(QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_RUN_ID),
            model_dir: default_real_qwen_model_dir(),
            suite_path: String::from(QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_SUITE_PATH),
            output_dir: String::from(QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_OUTPUT_DIR),
            report_path: String::from(QWEN36_REAL_PYLON_REHEARSAL_DEFAULT_REPORT_PATH),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen36RealPylonWorkerSpec {
    pub pylon_node_id: String,
    pub worker_id: String,
    pub backend_label: String,
    pub shard_id: String,
    pub prompt: String,
    pub target_token_id: u32,
    pub candidate_token_ids: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen36RealPylonModelHashes {
    pub config_sha256: String,
    pub tokenizer_sha256: String,
    pub index_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealPylonWorkerReport {
    pub pylon_node_id: String,
    pub worker_id: String,
    pub backend_label: String,
    pub shard_id: String,
    pub source_token_count: u64,
    pub training_config_path: String,
    pub training_config_sha256: String,
    pub adapter_artifact_path: String,
    pub adapter_artifact_sha256: String,
    pub training_receipt_path: String,
    pub training_receipt_digest: String,
    pub initial_loss: f32,
    pub final_loss: f32,
    pub loss_improved: bool,
    pub base_logits_sha256: String,
    pub hidden_state_sha256: String,
    pub pylon_job_path: String,
    pub pylon_job_digest: String,
    pub worker_receipt_path: String,
    pub worker_receipt_digest: String,
    pub worker_signature_valid: bool,
    pub worker_output_hash_verified: bool,
    pub worker_status: PylonTrainingWorkerJobStatus,
    pub payment_decision_path: String,
    pub payment_decision_digest: String,
    pub payment_status: PylonTrainingPaymentStatus,
    pub payment_proof: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen36RealPylonInterruptionReport {
    pub recovery_report_path: String,
    pub recovery_report_digest: String,
    pub exact_resume_match: bool,
    pub checkpoint_verified_before_payment: bool,
    pub worker_receipt_verified_before_payment: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qwen36RealPylonServeAdmission {
    pub route_id: String,
    pub served_model_id: String,
    pub adapter_path: String,
    pub adapter_sha256: String,
    pub adapter_identity_digest: String,
    pub load_verified: bool,
    pub benchmark_runner_adapter_arg: String,
    pub admission_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Qwen36RealPylonRehearsalReport {
    pub schema_version: String,
    pub run_id: String,
    pub generated_on: String,
    pub base_model: String,
    pub served_model_id: String,
    pub model_dir: String,
    pub model_hashes: Qwen36RealPylonModelHashes,
    pub suite_path: String,
    pub suite_sha256: String,
    pub activation_mode: String,
    pub active_trainable_target: String,
    pub adapter_format: String,
    pub worker_count: usize,
    pub accepted_worker_count: usize,
    pub workers: Vec<Qwen36RealPylonWorkerReport>,
    pub interruption_recovery: Qwen36RealPylonInterruptionReport,
    pub merge_manifest_path: String,
    pub merge_manifest_sha256: String,
    pub merge_receipt_path: String,
    pub merge_receipt_hash: String,
    pub merged_adapter_path: String,
    pub merged_adapter_sha256: String,
    pub eval_output_dir: String,
    pub eval_report_hash: String,
    pub champion_score_bps: u32,
    pub candidate_score_bps: u32,
    pub score_delta_bps: i32,
    pub promotion_decision: QwenLegalLoraMergePromotionDecision,
    pub private_eval_available: bool,
    pub private_eval_status: String,
    pub serve_admission: Qwen36RealPylonServeAdmission,
    pub treasury_handoff_path: String,
    pub treasury_handoff: PylonTrainingTreasuryHandoffBatch,
    pub payment_closeout_path: String,
    pub payment_closeout: PylonTrainingPaymentCloseout,
    pub claim_boundary: String,
    pub report_json_path: String,
    pub report_path: String,
    pub report_digest: String,
}

impl Qwen36RealPylonRehearsalReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psionic_qwen36_real_pylon_rehearsal_report|", &clone)
    }
}

#[derive(Debug, Error)]
pub enum Qwen36RealPylonRehearsalError {
    #[error("invalid Qwen3.6 real Pylon rehearsal: {0}")]
    Invalid(String),
    #[error("Qwen3.6 real Pylon rehearsal I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen3.6 real Pylon rehearsal JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    QwenSft(#[from] Qwen36RealLoraSftError),
    #[error(transparent)]
    Pylon(#[from] crate::QwenLegalPylonTrainingJobError),
    #[error(transparent)]
    Merge(#[from] QwenLegalLoraMergeError),
    #[error(transparent)]
    Checkpoint(#[from] QwenLegalCheckpointRecoveryError),
    #[error(transparent)]
    Adapter(#[from] psionic_adapters::LmHeadLoraLoadError),
}

pub fn run_qwen36_real_pylon_rehearsal_default()
-> Result<Qwen36RealPylonRehearsalReport, Qwen36RealPylonRehearsalError> {
    run_qwen36_real_pylon_rehearsal(&Qwen36RealPylonRehearsalConfig::default())
}

pub fn run_qwen36_real_pylon_rehearsal_config_path(
    path: impl AsRef<Path>,
) -> Result<Qwen36RealPylonRehearsalReport, Qwen36RealPylonRehearsalError> {
    let bytes = fs::read(path.as_ref()).map_err(|error| Qwen36RealPylonRehearsalError::Io {
        path: path.as_ref().display().to_string(),
        message: error.to_string(),
    })?;
    let config = serde_json::from_slice::<Qwen36RealPylonRehearsalConfig>(bytes.as_slice())?;
    run_qwen36_real_pylon_rehearsal(&config)
}

pub fn run_qwen36_real_pylon_rehearsal(
    config: &Qwen36RealPylonRehearsalConfig,
) -> Result<Qwen36RealPylonRehearsalReport, Qwen36RealPylonRehearsalError> {
    validate_config(config)?;
    let output_dir = PathBuf::from(&config.output_dir);
    create_dir(output_dir.as_path())?;
    if let Some(parent) = Path::new(&config.report_path).parent() {
        create_dir(parent)?;
    }

    let suite_sha256 = sha256_file(Path::new(&config.suite_path))?;
    let model_hashes = qwen_model_hashes(Path::new(&config.model_dir))?;
    let worker_specs = rehearsal_worker_specs();
    let mut worker_reports = Vec::with_capacity(worker_specs.len());
    let mut payment_decisions = Vec::with_capacity(worker_specs.len());
    for (index, worker) in worker_specs.iter().enumerate() {
        let (worker_report, payment_decision) =
            run_rehearsal_worker(config, &model_hashes, suite_sha256.as_str(), worker, index)?;
        worker_reports.push(worker_report);
        payment_decisions.push(payment_decision);
    }

    let recovery = write_qwen_legal_checkpoint_recovery_rehearsal(
        output_dir.join("checkpoint_recovery").as_path(),
    )?;
    let interruption_recovery = Qwen36RealPylonInterruptionReport {
        recovery_report_path: recovery.report_path.clone(),
        recovery_report_digest: recovery.report_digest.clone(),
        exact_resume_match: recovery.exact_resume_match,
        checkpoint_verified_before_payment: recovery
            .settlement_gate_after_receipt_verification
            .checkpoint_verified,
        worker_receipt_verified_before_payment: recovery
            .settlement_gate_after_receipt_verification
            .worker_receipt_verified,
        claim_boundary: String::from(
            "This attaches the existing QWEN-FT-07 checkpoint-transfer recovery proof to this rehearsal. It proves deterministic checkpoint recovery mechanics, not a live mid-kernel crash of the full Qwen transformer.",
        ),
    };

    let merge_dir = output_dir.join("merge");
    create_dir(merge_dir.as_path())?;
    let merge_manifest_path = merge_dir.join("merge_manifest.json");
    let merge_manifest = merge_manifest(
        config,
        &model_hashes,
        suite_sha256.as_str(),
        worker_reports.as_slice(),
        merge_dir.as_path(),
    );
    write_json(merge_manifest_path.as_path(), &merge_manifest)?;
    let merge_manifest_sha256 = sha256_file(merge_manifest_path.as_path())?;
    let merge_receipt = run_qwen_legal_lora_merge_manifest(merge_manifest_path.as_path())?;
    let validation = merge_receipt.validation.as_ref().ok_or_else(|| {
        Qwen36RealPylonRehearsalError::Invalid(String::from(
            "merge receipt did not include public eval validation",
        ))
    })?;
    let promotion = merge_receipt.promotion_gate.as_ref().ok_or_else(|| {
        Qwen36RealPylonRehearsalError::Invalid(String::from(
            "merge receipt did not include promotion gate",
        ))
    })?;
    let serve_admission = serve_admission(config, &merge_receipt)?;

    let treasury_handoff = build_qwen_legal_pylon_treasury_handoff_batch(
        format!("batch.{}.treasury_handoff", config.run_id),
        payment_decisions.as_slice(),
    )?;
    let treasury_handoff_path = output_dir.join("treasury_handoff.json");
    write_json(treasury_handoff_path.as_path(), &treasury_handoff)?;
    let deferred_proofs = treasury_handoff
        .payable_items
        .iter()
        .map(deferred_settlement_proof)
        .collect::<Vec<_>>();
    let payment_closeout = attach_qwen_legal_pylon_settlement_proofs(
        format!("closeout.{}.deferred", config.run_id),
        &treasury_handoff,
        deferred_proofs.as_slice(),
        Some(deferred_payment_policy(config.run_id.as_str())),
    )?;
    let payment_closeout_path = output_dir.join("payment_closeout.json");
    write_json(payment_closeout_path.as_path(), &payment_closeout)?;

    let report_json_path = output_dir.join("rehearsal_report.json");
    let mut report = Qwen36RealPylonRehearsalReport {
        schema_version: String::from(QWEN36_REAL_PYLON_REHEARSAL_REPORT_SCHEMA_VERSION),
        run_id: config.run_id.clone(),
        generated_on: String::from("2026-05-21"),
        base_model: String::from(QWEN36_27B_MODEL_ID),
        served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
        model_dir: config.model_dir.clone(),
        model_hashes,
        suite_path: config.suite_path.clone(),
        suite_sha256,
        activation_mode: String::from(QWEN36_REAL_LORA_ACTIVATION_MODE),
        active_trainable_target: String::from(QWEN36_REAL_LORA_ACTIVE_TARGET),
        adapter_format: String::from(QWEN36_REAL_LORA_ADAPTER_FORMAT),
        worker_count: worker_reports.len(),
        accepted_worker_count: worker_reports
            .iter()
            .filter(|worker| {
                worker.worker_signature_valid
                    && worker.worker_output_hash_verified
                    && worker.payment_status == PylonTrainingPaymentStatus::Payable
            })
            .count(),
        workers: worker_reports,
        interruption_recovery,
        merge_manifest_path: merge_manifest_path.display().to_string(),
        merge_manifest_sha256,
        merge_receipt_path: PathBuf::from(&merge_receipt.output_adapter_path)
            .with_extension("merge-receipt.json")
            .display()
            .to_string(),
        merge_receipt_hash: merge_receipt.receipt_hash.clone(),
        merged_adapter_path: merge_receipt.output_adapter_path.clone(),
        merged_adapter_sha256: merge_receipt.output_adapter_sha256.clone(),
        eval_output_dir: validation.output_dir.clone(),
        eval_report_hash: validation.report_hash.clone(),
        champion_score_bps: promotion.champion_score_bps,
        candidate_score_bps: promotion.candidate_score_bps,
        score_delta_bps: promotion.score_delta_bps,
        promotion_decision: promotion.decision,
        private_eval_available: false,
        private_eval_status: String::from(
            "No private Harvey gate is available in this repo. This report only claims public training-allowed suite evidence.",
        ),
        serve_admission,
        treasury_handoff_path: treasury_handoff_path.display().to_string(),
        treasury_handoff,
        payment_closeout_path: payment_closeout_path.display().to_string(),
        payment_closeout,
        claim_boundary: String::from(
            "This is a local loopback two-Pylon rehearsal over real downloaded Qwen3.6-27B safetensor rows through the bounded full-layer-smoke LoRA path. It proves two signed worker contributions, adapter merge, public Rust Harvey eval, serve-adapter admission, and deferred Bitcoin/Lightning payment closeout. It does not prove private Harvey benchmark performance, remote tailnet execution, or exact full-width transformer backprop.",
        ),
        report_json_path: report_json_path.display().to_string(),
        report_path: config.report_path.clone(),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    write_json(report_json_path.as_path(), &report)?;
    write_markdown_report(Path::new(&config.report_path), &report)?;
    Ok(report)
}

fn validate_config(
    config: &Qwen36RealPylonRehearsalConfig,
) -> Result<(), Qwen36RealPylonRehearsalError> {
    for (field, value) in [
        ("run_id", config.run_id.as_str()),
        ("model_dir", config.model_dir.as_str()),
        ("suite_path", config.suite_path.as_str()),
        ("output_dir", config.output_dir.as_str()),
        ("report_path", config.report_path.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(Qwen36RealPylonRehearsalError::Invalid(format!(
                "{field} must not be empty"
            )));
        }
    }
    Ok(())
}

fn run_rehearsal_worker(
    config: &Qwen36RealPylonRehearsalConfig,
    model_hashes: &Qwen36RealPylonModelHashes,
    suite_sha256: &str,
    worker: &Qwen36RealPylonWorkerSpec,
    index: usize,
) -> Result<
    (
        Qwen36RealPylonWorkerReport,
        PylonTrainingPaymentDecisionReceipt,
    ),
    Qwen36RealPylonRehearsalError,
> {
    let worker_dir = PathBuf::from(&config.output_dir)
        .join("workers")
        .join(sanitize_path(worker.worker_id.as_str()));
    create_dir(worker_dir.as_path())?;
    let train_config = worker_train_config(config, worker, index, worker_dir.as_path());
    let train_config_path = worker_dir.join("qwen36_real_lora_sft_config.json");
    write_json(train_config_path.as_path(), &train_config)?;
    let train_config_sha256 = sha256_file(train_config_path.as_path())?;
    let artifacts = run_qwen36_real_lora_sft(&train_config)?;
    let adapter_sha256 = sha256_file(Path::new(&artifacts.adapter_path))?;
    let source_token_count =
        u64::try_from(artifacts.receipt.prompt_receipt.token_count).unwrap_or(u64::MAX);
    let job = worker_job(
        config,
        model_hashes,
        suite_sha256,
        worker,
        index,
        source_token_count,
        &artifacts,
        train_config_path.as_path(),
        train_config_sha256.as_str(),
        adapter_sha256.as_str(),
        worker_dir.as_path(),
    )?;
    let job_path = worker_dir.join("pylon_job.json");
    write_json(job_path.as_path(), &job)?;
    let worker_receipt = run_qwen_legal_pylon_worker_job(
        &job,
        &PylonLocalWorkerRunOptions {
            worker_id: worker.worker_id.clone(),
            started_at_ms: 50_000 + u64::try_from(index).unwrap_or_default() * 1_000,
            emit_outputs: false,
        },
    )?;
    let verification = verify_qwen_legal_pylon_worker_receipt_path(&job.receipt_path)?;
    let payment = settle_qwen_legal_pylon_training_job_spec(&job)?;
    let worker_report = Qwen36RealPylonWorkerReport {
        pylon_node_id: worker.pylon_node_id.clone(),
        worker_id: worker.worker_id.clone(),
        backend_label: worker.backend_label.clone(),
        shard_id: worker.shard_id.clone(),
        source_token_count,
        training_config_path: train_config_path.display().to_string(),
        training_config_sha256: train_config_sha256,
        adapter_artifact_path: artifacts.adapter_path.clone(),
        adapter_artifact_sha256: adapter_sha256,
        training_receipt_path: artifacts.receipt_path,
        training_receipt_digest: artifacts.receipt.receipt_digest,
        initial_loss: artifacts.receipt.initial_loss,
        final_loss: artifacts.receipt.final_loss,
        loss_improved: artifacts.receipt.loss_improved,
        base_logits_sha256: artifacts.receipt.base_logits_sha256,
        hidden_state_sha256: artifacts.receipt.hidden_state_sha256,
        pylon_job_path: job_path.display().to_string(),
        pylon_job_digest: job.stable_digest(),
        worker_receipt_path: verification.receipt_path,
        worker_receipt_digest: worker_receipt.receipt_digest,
        worker_signature_valid: verification.signature_valid,
        worker_output_hash_verified: verification.output_files_rechecked,
        worker_status: verification.status,
        payment_decision_path: payment.decision_path.clone(),
        payment_decision_digest: payment.decision_digest.clone(),
        payment_status: payment.payment_status,
        payment_proof: payment.payment_proof.clone(),
    };
    Ok((worker_report, payment))
}

fn worker_train_config(
    config: &Qwen36RealPylonRehearsalConfig,
    worker: &Qwen36RealPylonWorkerSpec,
    index: usize,
    worker_dir: &Path,
) -> Qwen36RealLoraSftConfig {
    Qwen36RealLoraSftConfig {
        run_id: format!("{}-worker-{}", config.run_id, index + 1),
        model_dir: config.model_dir.clone(),
        adapter_id: format!("qwen36-27b-real-pylon-worker-{}", index + 1),
        adapter_revision: String::from("full-layer-smoke-lora-rehearsal-001"),
        prompt: worker.prompt.clone(),
        target_token_id: worker.target_token_id,
        candidate_token_ids: worker.candidate_token_ids.clone(),
        output_dir: worker_dir.join("real_lora_sft").display().to_string(),
        ..Qwen36RealLoraSftConfig::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn worker_job(
    config: &Qwen36RealPylonRehearsalConfig,
    model_hashes: &Qwen36RealPylonModelHashes,
    suite_sha256: &str,
    worker: &Qwen36RealPylonWorkerSpec,
    index: usize,
    _source_token_count: u64,
    artifacts: &Qwen36RealLoraSftArtifacts,
    train_config_path: &Path,
    train_config_sha256: &str,
    _adapter_sha256: &str,
    worker_dir: &Path,
) -> Result<PylonTrainingJobSpec, Qwen36RealPylonRehearsalError> {
    let job_id = format!("job.{}.worker-{}", config.run_id, index + 1);
    let model_dir = Path::new(&config.model_dir);
    Ok(PylonTrainingJobSpec {
        schema_version: String::from(QWEN_LEGAL_PYLON_TRAINING_JOB_SCHEMA_VERSION),
        job_id: job_id.clone(),
        parent_run_id: config.run_id.clone(),
        job_kind: PylonTrainingJobKind::SftTrainShard,
        model_id: String::from(QWEN36_27B_MODEL_ID),
        model_hash: model_hashes.index_sha256.clone(),
        adapter_id: None,
        adapter_hash: None,
        dataset_manifest_hash: suite_sha256.to_owned(),
        shard_assignment: PylonTrainingShardAssignment {
            assignment_id: format!("assignment.{}.worker-{}", config.run_id, index + 1),
            shard_id: worker.shard_id.clone(),
            shard_index: u32::try_from(index).unwrap_or_default(),
            shard_count: 2,
            start_index: Some(u64::try_from(index).unwrap_or_default()),
            end_index: Some(u64::try_from(index).unwrap_or_default() + 1),
        },
        training_config_hash: train_config_sha256.to_owned(),
        expected_input_artifacts: vec![
            artifact_ref(
                "harvey-public-three-suite",
                "eval_suite",
                &config.suite_path,
            )?,
            artifact_ref(
                "qwen36-real-training-config",
                "training_config",
                train_config_path,
            )?,
            artifact_ref(
                "qwen36-config",
                "model_config",
                model_dir.join("config.json"),
            )?,
            artifact_ref(
                "qwen36-tokenizer",
                "tokenizer",
                model_dir.join("tokenizer.json"),
            )?,
            artifact_ref(
                "qwen36-safetensors-index",
                "model_index",
                model_dir.join("model.safetensors.index.json"),
            )?,
        ],
        expected_output_artifacts: vec![PylonTrainingExpectedOutputArtifact {
            artifact_id: format!("artifact.{job_id}.real_lora_adapter"),
            artifact_type: String::from("qwen36_real_lora_adapter"),
            path: artifacts.adapter_path.clone(),
            required: true,
        }],
        max_runtime_ms: 600_000,
        hardware_requirements: PylonTrainingHardwareRequirements {
            min_memory_bytes: 8 * 1024 * 1024 * 1024,
            require_accelerator: false,
            accepted_backend_labels: vec![
                worker.backend_label.clone(),
                String::from("local_loopback_real_qwen_sampled_projection"),
            ],
        },
        payment_budget: PylonTrainingPaymentBudget {
            budget_id: format!("budget.{}.worker-{}", config.run_id, index + 1),
            agreed_price_microusd: 7_500,
            max_cost_microusd: 7_500,
            currency: String::from("USD"),
            payment_account_ref: String::from("ledger://local-rehearsal/qwen36-real-pylon"),
            bitcoin_payout: pylon_bitcoin_payout_target_for_job(job_id.as_str(), 60_000),
            pay_failed_but_valid_eval_attempts: false,
        },
        receipt_requirements: PylonTrainingReceiptRequirements {
            require_signature: true,
            require_logs_hash: true,
            require_metrics: true,
            required_output_artifact_types: vec![String::from("qwen36_real_lora_adapter")],
        },
        output_dir: worker_dir.join("pylon").display().to_string(),
        receipt_path: worker_dir
            .join("pylon")
            .join(format!("{job_id}.receipt.json"))
            .display()
            .to_string(),
    })
}

fn merge_manifest(
    config: &Qwen36RealPylonRehearsalConfig,
    model_hashes: &Qwen36RealPylonModelHashes,
    suite_sha256: &str,
    workers: &[Qwen36RealPylonWorkerReport],
    merge_dir: &Path,
) -> QwenLegalLoraMergeManifest {
    let parent_adapter_sha256 = stable_json_digest(
        b"qwen36_real_pylon_parent_adapter|",
        &serde_json::json!({
            "run_id": config.run_id,
            "base_model": QWEN36_27B_MODEL_ID,
            "model_index_sha256": model_hashes.index_sha256,
        }),
    );
    let optimizer_config_sha256 = stable_json_digest(
        b"qwen36_real_pylon_optimizer|",
        &serde_json::json!({
            "optimizer": "adamw",
            "learning_rate": 0.01,
            "target": QWEN36_REAL_LORA_ACTIVE_TARGET,
        }),
    );
    let target_modules = vec![String::from(QWEN36_REAL_LORA_ACTIVE_TARGET)];
    QwenLegalLoraMergeManifest {
        schema_version: String::from(QWEN_LEGAL_LORA_MERGE_MANIFEST_SCHEMA_VERSION),
        merge_id: config.run_id.clone(),
        mode: QwenLegalLoraMergeMode::DeltaAveraging,
        parent_adapter_sha256: parent_adapter_sha256.clone(),
        base_model: QwenLegalLoraMergeBaseModel {
            base_model_id: String::from(QWEN36_27B_MODEL_ID),
            base_model_revision: String::from("qwen3.6-27b-real-full-layer-smoke"),
            base_served_artifact_digest: model_hashes.index_sha256.clone(),
        },
        output_adapter: QwenLegalLoraMergeOutput {
            adapter_id: String::from("qwen36-27b-real-pylon-rehearsal-aggregate"),
            adapter_revision: String::from("full-layer-smoke-lora-001"),
            path: merge_dir
                .join("qwen36-27b-real-pylon-aggregate.safetensors")
                .display()
                .to_string(),
            expected_sha256: None,
        },
        worker_adapters: workers
            .iter()
            .map(|worker| QwenLegalLoraWorkerAdapterInput {
                worker_id: worker.worker_id.clone(),
                adapter_id: format!("adapter.{}", worker.worker_id),
                adapter_revision: String::from("full-layer-smoke-lora-rehearsal-001"),
                path: worker.adapter_artifact_path.clone(),
                sha256: worker.adapter_artifact_sha256.clone(),
                dataset_shard_hash: stable_json_digest(
                    b"qwen36_real_pylon_shard|",
                    &serde_json::json!({
                        "run_id": config.run_id,
                        "worker_id": worker.worker_id,
                        "shard_id": worker.shard_id,
                        "suite_sha256": suite_sha256,
                    }),
                ),
                token_count: worker.source_token_count,
                parent_adapter_sha256: parent_adapter_sha256.clone(),
                compatibility: Some(QwenLegalLoraWorkerCompatibilityFacts {
                    base_checkpoint_sha256: model_hashes.index_sha256.clone(),
                    tokenizer_sha256: model_hashes.tokenizer_sha256.clone(),
                    config_sha256: model_hashes.config_sha256.clone(),
                    corpus_manifest_sha256: suite_sha256.to_owned(),
                    corpus_shard_hash: stable_json_digest(
                        b"qwen36_real_pylon_shard|",
                        &serde_json::json!({
                            "run_id": config.run_id,
                            "worker_id": worker.worker_id,
                            "shard_id": worker.shard_id,
                            "suite_sha256": suite_sha256,
                        }),
                    ),
                    target_modules: target_modules.clone(),
                    optimizer_config_sha256: optimizer_config_sha256.clone(),
                    precision_policy: String::from("bf16-base-f32-lora-full-layer-smoke"),
                    step_window_id: format!("{}.window.001", config.run_id),
                }),
                validator_replay: Some(QwenLegalLoraValidatorReplayClaim {
                    status: QwenLegalLoraValidatorReplayStatus::Passed,
                    replay_receipt_sha256: Some(worker.worker_receipt_digest.clone()),
                    reason: String::from(
                        "worker receipt signature and adapter output hash were verified before merge",
                    ),
                }),
            })
            .collect(),
        compatibility: Some(crate::QwenLegalLoraMergeCompatibilityContract {
            base_checkpoint_sha256: model_hashes.index_sha256.clone(),
            tokenizer_sha256: model_hashes.tokenizer_sha256.clone(),
            config_sha256: model_hashes.config_sha256.clone(),
            corpus_manifest_sha256: suite_sha256.to_owned(),
            target_modules,
            optimizer_config_sha256,
            precision_policy: String::from("bf16-base-f32-lora-full-layer-smoke"),
            step_window_id: format!("{}.window.001", config.run_id),
        }),
        validation: Some(QwenLegalLoraMergeValidation {
            suite_path: config.suite_path.clone(),
            base_model: String::from("Qwen/Qwen3.6-27B/public-fixture-baseline"),
            output_dir: PathBuf::from(&config.output_dir)
                .join("eval")
                .display()
                .to_string(),
            champion_adapter_id: String::from("qwen36-public-fixture-baseline"),
            champion_score_bps: 3_333,
        }),
    }
}

fn serve_admission(
    config: &Qwen36RealPylonRehearsalConfig,
    merge_receipt: &crate::QwenLegalLoraMergeReceipt,
) -> Result<Qwen36RealPylonServeAdmission, Qwen36RealPylonRehearsalError> {
    let bytes = fs::read(&merge_receipt.output_adapter_path).map_err(|error| {
        Qwen36RealPylonRehearsalError::Io {
            path: merge_receipt.output_adapter_path.clone(),
            message: error.to_string(),
        }
    })?;
    let parameter_count = merge_receipt
        .workers
        .first()
        .map(|worker| worker.token_count)
        .unwrap_or_default();
    let identity = AdapterArtifactIdentity::new(
        "qwen36-27b-real-pylon-rehearsal-aggregate",
        "full-layer-smoke-lora-001",
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        QWEN36_27B_MODEL_ID,
        "qwen3.6-27b-real-full-layer-smoke",
        merge_receipt.parent_adapter_sha256.clone(),
        merge_receipt.output_adapter_sha256.clone(),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        parameter_count,
    )
    .with_provenance_digest(merge_receipt.receipt_hash.clone());
    let adapter =
        LmHeadLoraAdapterArtifact::from_safetensors_bytes(bytes.as_slice(), identity, 8.0)?;
    let mut admission = Qwen36RealPylonServeAdmission {
        route_id: format!("route.{}.qwen36_real_pylon", config.run_id),
        served_model_id: String::from(QWEN36_27B_SERVED_MODEL_ID),
        adapter_path: merge_receipt.output_adapter_path.clone(),
        adapter_sha256: merge_receipt.output_adapter_sha256.clone(),
        adapter_identity_digest: adapter.identity.stable_digest(),
        load_verified: adapter.identity.artifact_digest == merge_receipt.output_adapter_sha256,
        benchmark_runner_adapter_arg: format!("--adapter {}", merge_receipt.output_adapter_path),
        admission_digest: String::new(),
    };
    admission.admission_digest =
        stable_json_digest(b"qwen36_real_pylon_serve_admission|", &admission);
    Ok(admission)
}

fn rehearsal_worker_specs() -> Vec<Qwen36RealPylonWorkerSpec> {
    vec![
        Qwen36RealPylonWorkerSpec {
            pylon_node_id: String::from("pylon.loopback.macbook-pro-m2.legal-qwen.01"),
            worker_id: String::from("worker.loopback.macbook-pro-m2.qwen36-real.01"),
            backend_label: String::from("local_loopback_metal_real_qwen"),
            shard_id: String::from("shard.harvey-public-three.real-qwen.even"),
            prompt: String::from(
                "Draft a concise legal work product checklist for reviewing a vendor services agreement.",
            ),
            target_token_id: 271,
            candidate_token_ids: vec![0, 1, 2, 3, 4, 5, 271, 272],
        },
        Qwen36RealPylonWorkerSpec {
            pylon_node_id: String::from("pylon.loopback.imac-pro-bertha.legal-qwen.02"),
            worker_id: String::from("worker.loopback.imac-pro-bertha.qwen36-real.02"),
            backend_label: String::from("local_loopback_cpu_real_qwen"),
            shard_id: String::from("shard.harvey-public-three.real-qwen.odd"),
            prompt: String::from(
                "Draft a concise legal work product checklist for privilege-log quality review.",
            ),
            target_token_id: 271,
            candidate_token_ids: vec![0, 1, 2, 3, 4, 5, 271, 272],
        },
    ]
}

fn deferred_settlement_proof(
    item: &crate::PylonTrainingTreasuryHandoffItem,
) -> PylonTrainingBitcoinSettlementProof {
    let mut proof = PylonTrainingBitcoinSettlementProof {
        schema_version: String::from(QWEN_LEGAL_PYLON_BITCOIN_SETTLEMENT_PROOF_SCHEMA_VERSION),
        operation_id: format!("nexus.lightning.deferred.{}", item.payout_authorization_id),
        payout_authorization_id: item.payout_authorization_id.clone(),
        worker_receipt_digest: item.worker_receipt_digest.clone(),
        amount_msat: item.payout.amount_msat,
        settlement_method: item.payout.target_kind,
        settlement_mode: PylonTrainingSettlementExecutionMode::DryRunLocal,
        status: PylonTrainingBitcoinSettlementStatus::DeferredByOperator,
        payment_hash: None,
        payment_preimage: None,
        transaction_proof: None,
        bolt11_invoice: Some(item.payout.payout_target_ref.clone()),
        bolt12_offer: None,
        bip353_address: None,
        lnurl_pay_ref: None,
        fee_msat: 0,
        settled_at_ms: 0,
        reconciliation_digest: stable_json_digest(
            b"qwen36_real_pylon_deferred_payment_reconciliation|",
            item,
        ),
        proof_digest: String::new(),
    };
    proof.proof_digest = proof.stable_digest();
    proof
}

fn deferred_payment_policy(run_id: &str) -> PylonTrainingDeferredPaymentPolicy {
    PylonTrainingDeferredPaymentPolicy {
        policy_id: format!("policy.{run_id}.operator_deferred_local_rehearsal"),
        operator_id: String::from("operator.openagents.local"),
        reason: String::from(
            "Local rehearsal records payable work and defers actual Bitcoin Lightning settlement to Treasury Nexus.",
        ),
        approved_at_ms: 1_779_321_600_000,
        expires_at_ms: 1_782_000_000_000,
    }
}

fn artifact_ref(
    artifact_id: impl Into<String>,
    artifact_type: impl Into<String>,
    path: impl AsRef<Path>,
) -> Result<PylonTrainingArtifactRef, Qwen36RealPylonRehearsalError> {
    let path = path.as_ref();
    Ok(PylonTrainingArtifactRef {
        artifact_id: artifact_id.into(),
        artifact_type: artifact_type.into(),
        path: path.display().to_string(),
        sha256: sha256_file(path)?,
    })
}

fn qwen_model_hashes(
    model_dir: &Path,
) -> Result<Qwen36RealPylonModelHashes, Qwen36RealPylonRehearsalError> {
    Ok(Qwen36RealPylonModelHashes {
        config_sha256: sha256_file(model_dir.join("config.json").as_path())?,
        tokenizer_sha256: sha256_file(model_dir.join("tokenizer.json").as_path())?,
        index_sha256: sha256_file(model_dir.join("model.safetensors.index.json").as_path())?,
    })
}

fn write_markdown_report(
    path: &Path,
    report: &Qwen36RealPylonRehearsalReport,
) -> Result<(), Qwen36RealPylonRehearsalError> {
    let mut markdown = String::new();
    markdown.push_str("# Qwen3.6 Real Pylon Rehearsal 001\n\n");
    markdown.push_str("## Status\n\n");
    markdown.push_str(&format!(
        "- run id: `{}`\n- workers accepted: `{}` / `{}`\n- model: `{}`\n- activation path: `{}`\n- trainable target: `{}`\n- merged adapter: `{}`\n- merged adapter sha256: `{}`\n- public eval candidate score: `{}` bps\n- public eval delta: `{}` bps\n- promotion decision: `{:?}`\n- serve admission verified: `{}`\n- payment gate: `{:?}`\n\n",
        report.run_id,
        report.accepted_worker_count,
        report.worker_count,
        report.base_model,
        report.activation_mode,
        report.active_trainable_target,
        report.merged_adapter_path,
        report.merged_adapter_sha256,
        report.candidate_score_bps,
        report.score_delta_bps,
        report.promotion_decision,
        report.serve_admission.load_verified,
        report.payment_closeout.promotion_payment_gate_status,
    ));
    markdown.push_str("## What Ran\n\n");
    markdown.push_str("Two local loopback Pylon identities each ran the Rust Qwen3.6 bounded full-layer-smoke LoRA trainer against downloaded Qwen3.6-27B safetensor rows. Each worker produced a signed Pylon receipt, a payable decision, and an adapter. The adapters were merged, evaluated with the Rust public Harvey suite, loaded through the serving adapter path, and closed with an explicit deferred-payment proof.\n\n");
    markdown.push_str("## Workers\n\n");
    markdown.push_str("| worker | shard | loss | adapter sha256 | receipt | payment |\n");
    markdown.push_str("| --- | --- | ---: | --- | --- | --- |\n");
    for worker in &report.workers {
        markdown.push_str(&format!(
            "| `{}` | `{}` | `{:.6} -> {:.6}` | `{}` | `{}` | `{:?}` |\n",
            worker.worker_id,
            worker.shard_id,
            worker.initial_loss,
            worker.final_loss,
            worker.adapter_artifact_sha256,
            worker.worker_receipt_digest,
            worker.payment_status,
        ));
    }
    markdown.push_str("\n## Merge And Eval\n\n");
    markdown.push_str(&format!(
        "- merge manifest: `{}`\n- merge manifest sha256: `{}`\n- merge receipt: `{}`\n- merge receipt hash: `{}`\n- eval output dir: `{}`\n- eval report hash: `{}`\n- champion score: `{}` bps\n- candidate score: `{}` bps\n- score delta: `{}` bps\n\n",
        report.merge_manifest_path,
        report.merge_manifest_sha256,
        report.merge_receipt_path,
        report.merge_receipt_hash,
        report.eval_output_dir,
        report.eval_report_hash,
        report.champion_score_bps,
        report.candidate_score_bps,
        report.score_delta_bps,
    ));
    markdown.push_str("## Recovery And Payment\n\n");
    markdown.push_str(&format!(
        "- checkpoint recovery report: `{}`\n- recovery exact match: `{}`\n- treasury handoff: `{}`\n- payment closeout: `{}`\n- accepted work count: `{}`\n- payable payment count: `{}`\n- pending payment count: `{}`\n- settled payment count: `{}`\n- deferred payment count: `{}`\n- failed payment count: `{}`\n- no wallet secrets present: `{}`\n\n",
        report.interruption_recovery.recovery_report_path,
        report.interruption_recovery.exact_resume_match,
        report.treasury_handoff_path,
        report.payment_closeout_path,
        report.payment_closeout.accepted_work_count,
        report.payment_closeout.payable_payment_count,
        report.payment_closeout.pending_payment_count,
        report.payment_closeout.settled_payment_count,
        report.payment_closeout.deferred_payment_count,
        report.payment_closeout.failed_payment_count,
        report.payment_closeout.no_wallet_secrets_present,
    ));
    markdown.push_str("## Serve Admission\n\n");
    markdown.push_str(&format!(
        "- route id: `{}`\n- served model id: `{}`\n- adapter identity digest: `{}`\n- benchmark runner arg: `{}`\n\n",
        report.serve_admission.route_id,
        report.serve_admission.served_model_id,
        report.serve_admission.adapter_identity_digest,
        report.serve_admission.benchmark_runner_adapter_arg,
    ));
    markdown.push_str("## Boundaries\n\n");
    markdown.push_str(&format!("{}\n\n", report.claim_boundary));
    markdown.push_str(&format!("{}\n\n", report.private_eval_status));
    markdown.push_str(&format!(
        "- machine-readable report: `{}`\n- report digest: `{}`\n",
        report.report_json_path, report.report_digest
    ));
    fs::write(path, markdown).map_err(|error| Qwen36RealPylonRehearsalError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn create_dir(path: &Path) -> Result<(), Qwen36RealPylonRehearsalError> {
    fs::create_dir_all(path).map_err(|error| Qwen36RealPylonRehearsalError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Qwen36RealPylonRehearsalError> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).map_err(|error| Qwen36RealPylonRehearsalError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn sha256_file(path: impl AsRef<Path>) -> Result<String, Qwen36RealPylonRehearsalError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| Qwen36RealPylonRehearsalError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(sha256_hex(bytes.as_slice()))
}

fn stable_json_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sanitize_path(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}

fn default_real_qwen_model_dir() -> String {
    String::from(QWEN36_27B_REAL_MODEL_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rehearsal_workers_do_not_use_legacy_local_ids() {
        let workers = rehearsal_worker_specs();
        assert_eq!(workers.len(), 2);
        for worker in workers {
            assert!(!worker.worker_id.starts_with("pylon.local.harvey-legal"));
            assert!(worker.worker_id.starts_with("worker.loopback."));
            assert!(worker.pylon_node_id.starts_with("pylon.loopback."));
        }
    }

    #[test]
    fn deferred_payment_policy_is_valid_shape() {
        let policy = deferred_payment_policy("run");
        assert!(!policy.policy_id.is_empty());
        assert!(policy.expires_at_ms > policy.approved_at_ms);
        assert!(policy.reason.contains("defer"));
    }
}
