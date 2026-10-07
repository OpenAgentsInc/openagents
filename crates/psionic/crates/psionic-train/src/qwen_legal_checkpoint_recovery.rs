use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN_LEGAL_CHECKPOINT_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_checkpoint_manifest.v1";
pub const QWEN_LEGAL_CHECKPOINT_STREAM_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_checkpoint_stream_receipt.v1";
pub const QWEN_LEGAL_LATE_JOIN_BOOTSTRAP_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_late_join_bootstrap_receipt.v1";
pub const QWEN_LEGAL_CHECKPOINT_SETTLEMENT_GATE_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_checkpoint_settlement_gate.v1";
pub const QWEN_LEGAL_CHECKPOINT_RECOVERY_REPORT_SCHEMA_VERSION: &str =
    "psionic.qwen_legal_checkpoint_recovery_report.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalCheckpointArtifactFamily {
    BaseModelCachePointer,
    AdapterCheckpoint,
    OptimizerState,
    SchedulerCursorState,
    AggregateCandidate,
    EvalCandidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointArtifactPointer {
    pub artifact_id: String,
    pub family: QwenLegalCheckpointArtifactFamily,
    pub path: String,
    pub sha256: String,
    pub byte_len: u64,
    pub secret_free: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointManifest {
    pub schema_version: String,
    pub run_id: String,
    pub checkpoint_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_checkpoint_id: Option<String>,
    pub accepted_step: u64,
    pub base_model_id: String,
    pub base_model_cache_pointer: QwenLegalCheckpointArtifactPointer,
    pub corpus_shard_lock: String,
    pub artifacts: Vec<QwenLegalCheckpointArtifactPointer>,
    pub deterministic_replay_digest: String,
    pub manifest_digest: String,
}

impl QwenLegalCheckpointManifest {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.manifest_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_checkpoint_manifest|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointChunkReceipt {
    pub chunk_index: u64,
    pub byte_start: u64,
    pub byte_end_exclusive: u64,
    pub sha256: String,
    pub retry_count: u32,
    pub accepted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointStreamReceipt {
    pub schema_version: String,
    pub artifact_id: String,
    pub family: QwenLegalCheckpointArtifactFamily,
    pub source_path: String,
    pub destination_path: String,
    pub total_bytes: u64,
    pub full_sha256: String,
    pub chunk_size: u64,
    pub chunks: Vec<QwenLegalCheckpointChunkReceipt>,
    pub transfer_retry_count: u32,
    pub stream_receipt_digest: String,
}

impl QwenLegalCheckpointStreamReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.stream_receipt_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_checkpoint_stream_receipt|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointStreamVerification {
    pub artifact_id: String,
    pub destination_path: String,
    pub total_bytes: u64,
    pub full_sha256: String,
    pub chunk_count: u64,
    pub retry_count: u32,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalLateJoinBootstrapReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub joiner_worker_id: String,
    pub from_checkpoint_id: String,
    pub resume_step: u64,
    pub corpus_shard_lock: String,
    pub checkpoint_manifest_digest: String,
    pub adapter_checkpoint_sha256: String,
    pub optimizer_state_sha256: String,
    pub scheduler_cursor_sha256: String,
    pub checkpoint_verified: bool,
    pub secret_scan_passed: bool,
    pub bootstrap_accepted: bool,
    pub refusal_reason: Option<String>,
    pub receipt_digest: String,
}

impl QwenLegalLateJoinBootstrapReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_late_join_bootstrap_receipt|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalCheckpointSettlementStatus {
    WithheldCheckpointUnverified,
    WithheldWorkerReceiptUnverified,
    Payable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointSettlementGateReceipt {
    pub schema_version: String,
    pub gate_id: String,
    pub run_id: String,
    pub checkpoint_verified: bool,
    pub worker_receipt_verified: bool,
    pub transfer_receipts_verified: bool,
    pub status: QwenLegalCheckpointSettlementStatus,
    pub reason: String,
    pub receipt_digest: String,
}

impl QwenLegalCheckpointSettlementGateReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipt_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_checkpoint_settlement_gate|", &clone)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointRetentionPolicy {
    pub policy_id: String,
    pub keep_latest_accepted: u32,
    pub keep_failed_upload_hours: u32,
    pub cleanup_targets: Vec<String>,
    pub delete_secrets: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QwenLegalCheckpointRecoveryReport {
    pub schema_version: String,
    pub run_id: String,
    pub artifact_families: Vec<QwenLegalCheckpointArtifactFamily>,
    pub uninterrupted_final_sha256: String,
    pub resumed_final_sha256: String,
    pub exact_resume_match: bool,
    pub accepted_divergence_reason: Option<String>,
    pub last_accepted_step: u64,
    pub latest_checkpoint_manifest: QwenLegalCheckpointManifest,
    pub stream_receipts: Vec<QwenLegalCheckpointStreamReceipt>,
    pub stream_verifications: Vec<QwenLegalCheckpointStreamVerification>,
    pub late_join_bootstrap: QwenLegalLateJoinBootstrapReceipt,
    pub settlement_gate_before_receipt_verification: QwenLegalCheckpointSettlementGateReceipt,
    pub settlement_gate_after_receipt_verification: QwenLegalCheckpointSettlementGateReceipt,
    pub retention_policy: QwenLegalCheckpointRetentionPolicy,
    pub report_path: String,
    pub report_digest: String,
}

impl QwenLegalCheckpointRecoveryReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_json_digest(b"psionic_qwen_legal_checkpoint_recovery_report|", &clone)
    }
}

#[derive(Debug, Error)]
pub enum QwenLegalCheckpointRecoveryError {
    #[error("Qwen checkpoint recovery invalid input: {0}")]
    InvalidInput(String),
    #[error("Qwen checkpoint stream verification failed: {0}")]
    Verification(String),
    #[error("Qwen checkpoint recovery I/O failed at `{path}`: {message}")]
    Io { path: String, message: String },
    #[error("Qwen checkpoint recovery serialization failed: {0}")]
    Serialization(String),
}

pub fn write_qwen_legal_checkpoint_recovery_rehearsal(
    output_dir: impl AsRef<Path>,
) -> Result<QwenLegalCheckpointRecoveryReport, QwenLegalCheckpointRecoveryError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
        path: output_dir.display().to_string(),
        message: error.to_string(),
    })?;

    let run_id = "qwen-legal-checkpoint-recovery-rehearsal-001";
    let base_model_pointer_path = output_dir.join("base-model-cache-pointer.json");
    let base_pointer_bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "model_id": "Qwen/Qwen3.6-27B",
        "cache_ref": "hf-cache://qwen3.6-27b/legal-rehearsal",
        "credential_policy": "redacted; no raw credentials are stored in this pointer",
    }))
    .map_err(|error| QwenLegalCheckpointRecoveryError::Serialization(error.to_string()))?;
    write_bytes(
        base_model_pointer_path.as_path(),
        base_pointer_bytes.as_slice(),
    )?;

    let base_state = sha256_hex(b"qwen-legal-recovery-base-state");
    let interrupted_step = 2;
    let final_step = 4;
    let interrupted_state = deterministic_adapter_state(base_state.as_str(), interrupted_step);
    let uninterrupted_final = deterministic_adapter_state(base_state.as_str(), final_step);
    let resumed_final =
        resume_adapter_state(interrupted_state.as_str(), interrupted_step + 1, final_step);

    let checkpoint_dir = output_dir.join("checkpoints").join("step-2");
    fs::create_dir_all(checkpoint_dir.as_path()).map_err(|error| {
        QwenLegalCheckpointRecoveryError::Io {
            path: checkpoint_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let adapter_path = checkpoint_dir.join("adapter.safetensors");
    let optimizer_path = checkpoint_dir.join("optimizer-state.json");
    let scheduler_path = checkpoint_dir.join("scheduler-cursor.json");
    write_bytes(adapter_path.as_path(), interrupted_state.as_bytes())?;
    write_bytes(
        optimizer_path.as_path(),
        serde_json::to_vec_pretty(&serde_json::json!({
            "optimizer": "adamw",
            "accepted_step": interrupted_step,
            "secret_policy": "none",
        }))
        .map_err(|error| QwenLegalCheckpointRecoveryError::Serialization(error.to_string()))?
        .as_slice(),
    )?;
    write_bytes(
        scheduler_path.as_path(),
        serde_json::to_vec_pretty(&serde_json::json!({
            "scheduler": "linear-warmup-cosine",
            "next_step": interrupted_step + 1,
            "cursor_state": "accepted",
        }))
        .map_err(|error| QwenLegalCheckpointRecoveryError::Serialization(error.to_string()))?
        .as_slice(),
    )?;

    let base_model_cache_pointer = artifact_pointer(
        "artifact.qwen36.base_model_cache_pointer",
        QwenLegalCheckpointArtifactFamily::BaseModelCachePointer,
        base_model_pointer_path.as_path(),
    )?;
    let adapter_pointer = artifact_pointer(
        "artifact.qwen_legal.step_2.adapter_checkpoint",
        QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
        adapter_path.as_path(),
    )?;
    let optimizer_pointer = artifact_pointer(
        "artifact.qwen_legal.step_2.optimizer_state",
        QwenLegalCheckpointArtifactFamily::OptimizerState,
        optimizer_path.as_path(),
    )?;
    let scheduler_pointer = artifact_pointer(
        "artifact.qwen_legal.step_2.scheduler_cursor_state",
        QwenLegalCheckpointArtifactFamily::SchedulerCursorState,
        scheduler_path.as_path(),
    )?;
    let aggregate_candidate_path = output_dir.join("aggregate-candidate.safetensors");
    write_bytes(aggregate_candidate_path.as_path(), resumed_final.as_bytes())?;
    let eval_candidate_path = output_dir.join("eval-candidate.json");
    write_bytes(
        eval_candidate_path.as_path(),
        serde_json::to_vec_pretty(&serde_json::json!({
            "candidate_adapter_sha256": sha256_hex(resumed_final.as_bytes()),
            "eval_gate": "pending",
        }))
        .map_err(|error| QwenLegalCheckpointRecoveryError::Serialization(error.to_string()))?
        .as_slice(),
    )?;
    let aggregate_pointer = artifact_pointer(
        "artifact.qwen_legal.aggregate_candidate",
        QwenLegalCheckpointArtifactFamily::AggregateCandidate,
        aggregate_candidate_path.as_path(),
    )?;
    let eval_pointer = artifact_pointer(
        "artifact.qwen_legal.eval_candidate",
        QwenLegalCheckpointArtifactFamily::EvalCandidate,
        eval_candidate_path.as_path(),
    )?;

    let replay_digest = stable_json_digest(
        b"psionic_qwen_legal_replay_digest|",
        &serde_json::json!({
            "run_id": run_id,
            "base_state": base_state,
            "interrupted_step": interrupted_step,
            "final_step": final_step,
            "resumed_final": resumed_final,
        }),
    );
    let mut manifest = QwenLegalCheckpointManifest {
        schema_version: String::from(QWEN_LEGAL_CHECKPOINT_MANIFEST_SCHEMA_VERSION),
        run_id: String::from(run_id),
        checkpoint_id: String::from("qwen-legal-checkpoint-step-2"),
        parent_checkpoint_id: None,
        accepted_step: interrupted_step,
        base_model_id: String::from("Qwen/Qwen3.6-27B"),
        base_model_cache_pointer,
        corpus_shard_lock: String::from("corpus://harvey-legal/public-training-slice/even"),
        artifacts: vec![
            adapter_pointer,
            optimizer_pointer,
            scheduler_pointer,
            aggregate_pointer,
            eval_pointer,
        ],
        deterministic_replay_digest: replay_digest,
        manifest_digest: String::new(),
    };
    manifest.manifest_digest = manifest.stable_digest();
    write_json(
        checkpoint_dir.join("checkpoint_manifest.json").as_path(),
        &manifest,
    )?;

    let remote_dir = output_dir.join("remote-cache");
    let stream_receipts = vec![
        stream_qwen_legal_checkpoint_artifact(
            adapter_path.as_path(),
            remote_dir.join("adapter.safetensors").as_path(),
            "artifact.qwen_legal.step_2.adapter_checkpoint",
            QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
            8,
            1,
        )?,
        stream_qwen_legal_checkpoint_artifact(
            optimizer_path.as_path(),
            remote_dir.join("optimizer-state.json").as_path(),
            "artifact.qwen_legal.step_2.optimizer_state",
            QwenLegalCheckpointArtifactFamily::OptimizerState,
            16,
            0,
        )?,
        stream_qwen_legal_checkpoint_artifact(
            scheduler_path.as_path(),
            remote_dir.join("scheduler-cursor.json").as_path(),
            "artifact.qwen_legal.step_2.scheduler_cursor_state",
            QwenLegalCheckpointArtifactFamily::SchedulerCursorState,
            16,
            0,
        )?,
    ];
    let mut stream_verifications = Vec::with_capacity(stream_receipts.len());
    for receipt in &stream_receipts {
        stream_verifications.push(verify_qwen_legal_checkpoint_stream_receipt(receipt)?);
    }

    let late_join_bootstrap =
        build_qwen_legal_late_join_bootstrap(&manifest, "worker.qwen-legal.latejoin.01", true);
    let settlement_gate_before_receipt_verification =
        qwen_legal_checkpoint_settlement_gate(run_id, true, false, true);
    let settlement_gate_after_receipt_verification =
        qwen_legal_checkpoint_settlement_gate(run_id, true, true, true);

    let report_path = output_dir
        .join("qwen_legal_checkpoint_recovery_report.json")
        .display()
        .to_string();
    let mut report = QwenLegalCheckpointRecoveryReport {
        schema_version: String::from(QWEN_LEGAL_CHECKPOINT_RECOVERY_REPORT_SCHEMA_VERSION),
        run_id: String::from(run_id),
        artifact_families: vec![
            QwenLegalCheckpointArtifactFamily::BaseModelCachePointer,
            QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
            QwenLegalCheckpointArtifactFamily::OptimizerState,
            QwenLegalCheckpointArtifactFamily::SchedulerCursorState,
            QwenLegalCheckpointArtifactFamily::AggregateCandidate,
            QwenLegalCheckpointArtifactFamily::EvalCandidate,
        ],
        uninterrupted_final_sha256: sha256_hex(uninterrupted_final.as_bytes()),
        resumed_final_sha256: sha256_hex(resumed_final.as_bytes()),
        exact_resume_match: uninterrupted_final == resumed_final,
        accepted_divergence_reason: None,
        last_accepted_step: interrupted_step,
        latest_checkpoint_manifest: manifest,
        stream_receipts,
        stream_verifications,
        late_join_bootstrap,
        settlement_gate_before_receipt_verification,
        settlement_gate_after_receipt_verification,
        retention_policy: QwenLegalCheckpointRetentionPolicy {
            policy_id: String::from("qwen_legal_checkpoint_retention_v1"),
            keep_latest_accepted: 3,
            keep_failed_upload_hours: 24,
            cleanup_targets: vec![
                String::from("failed_chunk_uploads"),
                String::from("superseded_unaccepted_checkpoints"),
                String::from("orphaned_temp_transfer_files"),
            ],
            delete_secrets: true,
        },
        report_path,
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    write_json(Path::new(report.report_path.as_str()), &report)?;
    Ok(report)
}

pub fn stream_qwen_legal_checkpoint_artifact(
    source_path: impl AsRef<Path>,
    destination_path: impl AsRef<Path>,
    artifact_id: impl Into<String>,
    family: QwenLegalCheckpointArtifactFamily,
    chunk_size: u64,
    transfer_retry_count: u32,
) -> Result<QwenLegalCheckpointStreamReceipt, QwenLegalCheckpointRecoveryError> {
    if chunk_size == 0 {
        return Err(QwenLegalCheckpointRecoveryError::InvalidInput(
            String::from("chunk_size must be non-zero"),
        ));
    }
    let source_path = source_path.as_ref();
    let destination_path = destination_path.as_ref();
    let bytes = fs::read(source_path).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
        path: source_path.display().to_string(),
        message: error.to_string(),
    })?;
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    fs::write(destination_path, bytes.as_slice()).map_err(|error| {
        QwenLegalCheckpointRecoveryError::Io {
            path: destination_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let mut chunks = Vec::new();
    let mut cursor = 0_usize;
    let chunk_size_usize = usize::try_from(chunk_size).unwrap_or(usize::MAX);
    while cursor < bytes.len() {
        let end = cursor.saturating_add(chunk_size_usize).min(bytes.len());
        chunks.push(QwenLegalCheckpointChunkReceipt {
            chunk_index: u64::try_from(chunks.len()).unwrap_or(u64::MAX),
            byte_start: u64::try_from(cursor).unwrap_or(u64::MAX),
            byte_end_exclusive: u64::try_from(end).unwrap_or(u64::MAX),
            sha256: sha256_hex(&bytes[cursor..end]),
            retry_count: if cursor == 0 { transfer_retry_count } else { 0 },
            accepted: true,
        });
        cursor = end;
    }
    let mut receipt = QwenLegalCheckpointStreamReceipt {
        schema_version: String::from(QWEN_LEGAL_CHECKPOINT_STREAM_RECEIPT_SCHEMA_VERSION),
        artifact_id: artifact_id.into(),
        family,
        source_path: source_path.display().to_string(),
        destination_path: destination_path.display().to_string(),
        total_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        full_sha256: sha256_hex(bytes.as_slice()),
        chunk_size,
        chunks,
        transfer_retry_count,
        stream_receipt_digest: String::new(),
    };
    receipt.stream_receipt_digest = receipt.stable_digest();
    Ok(receipt)
}

pub fn verify_qwen_legal_checkpoint_stream_receipt(
    receipt: &QwenLegalCheckpointStreamReceipt,
) -> Result<QwenLegalCheckpointStreamVerification, QwenLegalCheckpointRecoveryError> {
    if receipt.schema_version != QWEN_LEGAL_CHECKPOINT_STREAM_RECEIPT_SCHEMA_VERSION {
        return Err(QwenLegalCheckpointRecoveryError::Verification(
            String::from("stream receipt schema version drifted"),
        ));
    }
    if receipt.stream_receipt_digest != receipt.stable_digest() {
        return Err(QwenLegalCheckpointRecoveryError::Verification(
            String::from("stream receipt digest drifted"),
        ));
    }
    let bytes = fs::read(receipt.destination_path.as_str()).map_err(|error| {
        QwenLegalCheckpointRecoveryError::Io {
            path: receipt.destination_path.clone(),
            message: error.to_string(),
        }
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != receipt.total_bytes {
        return Err(QwenLegalCheckpointRecoveryError::Verification(
            String::from("stream destination byte length drifted"),
        ));
    }
    if sha256_hex(bytes.as_slice()) != receipt.full_sha256 {
        return Err(QwenLegalCheckpointRecoveryError::Verification(
            String::from("stream destination full hash drifted"),
        ));
    }
    let mut cursor = 0_u64;
    for (index, chunk) in receipt.chunks.iter().enumerate() {
        if !chunk.accepted {
            return Err(QwenLegalCheckpointRecoveryError::Verification(format!(
                "chunk {} was not accepted",
                chunk.chunk_index
            )));
        }
        if chunk.chunk_index != u64::try_from(index).unwrap_or(u64::MAX) {
            return Err(QwenLegalCheckpointRecoveryError::Verification(
                String::from("stream chunks are reordered"),
            ));
        }
        if chunk.byte_start != cursor || chunk.byte_end_exclusive <= chunk.byte_start {
            return Err(QwenLegalCheckpointRecoveryError::Verification(
                String::from("stream chunk byte ranges are not contiguous"),
            ));
        }
        if chunk.byte_end_exclusive > receipt.total_bytes {
            return Err(QwenLegalCheckpointRecoveryError::Verification(
                String::from("stream chunk byte range exceeds artifact length"),
            ));
        }
        let start = usize::try_from(chunk.byte_start).unwrap_or(usize::MAX);
        let end = usize::try_from(chunk.byte_end_exclusive).unwrap_or(usize::MAX);
        if end > bytes.len() || sha256_hex(&bytes[start..end]) != chunk.sha256 {
            return Err(QwenLegalCheckpointRecoveryError::Verification(format!(
                "stream chunk {} hash drifted",
                chunk.chunk_index
            )));
        }
        cursor = chunk.byte_end_exclusive;
    }
    if cursor != receipt.total_bytes {
        return Err(QwenLegalCheckpointRecoveryError::Verification(
            String::from("stream chunks do not cover the full artifact"),
        ));
    }
    Ok(QwenLegalCheckpointStreamVerification {
        artifact_id: receipt.artifact_id.clone(),
        destination_path: receipt.destination_path.clone(),
        total_bytes: receipt.total_bytes,
        full_sha256: receipt.full_sha256.clone(),
        chunk_count: u64::try_from(receipt.chunks.len()).unwrap_or(u64::MAX),
        retry_count: receipt.transfer_retry_count,
        verified: true,
    })
}

pub fn build_qwen_legal_late_join_bootstrap(
    manifest: &QwenLegalCheckpointManifest,
    joiner_worker_id: impl Into<String>,
    checkpoint_verified: bool,
) -> QwenLegalLateJoinBootstrapReceipt {
    let joiner_worker_id = joiner_worker_id.into();
    let adapter = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.family == QwenLegalCheckpointArtifactFamily::AdapterCheckpoint);
    let optimizer = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.family == QwenLegalCheckpointArtifactFamily::OptimizerState);
    let scheduler = manifest.artifacts.iter().find(|artifact| {
        artifact.family == QwenLegalCheckpointArtifactFamily::SchedulerCursorState
    });
    let secret_scan_passed = manifest.base_model_cache_pointer.secret_free
        && manifest
            .artifacts
            .iter()
            .all(|artifact| artifact.secret_free);
    let missing = adapter.is_none() || optimizer.is_none() || scheduler.is_none();
    let bootstrap_accepted = checkpoint_verified && secret_scan_passed && !missing;
    let refusal_reason = if bootstrap_accepted {
        None
    } else if !checkpoint_verified {
        Some(String::from("checkpoint_not_verified"))
    } else if !secret_scan_passed {
        Some(String::from("checkpoint_artifact_secret_scan_failed"))
    } else {
        Some(String::from("required_checkpoint_family_missing"))
    };
    let mut receipt = QwenLegalLateJoinBootstrapReceipt {
        schema_version: String::from(QWEN_LEGAL_LATE_JOIN_BOOTSTRAP_RECEIPT_SCHEMA_VERSION),
        run_id: manifest.run_id.clone(),
        joiner_worker_id,
        from_checkpoint_id: manifest.checkpoint_id.clone(),
        resume_step: manifest.accepted_step.saturating_add(1),
        corpus_shard_lock: manifest.corpus_shard_lock.clone(),
        checkpoint_manifest_digest: manifest.manifest_digest.clone(),
        adapter_checkpoint_sha256: adapter
            .map(|artifact| artifact.sha256.clone())
            .unwrap_or_default(),
        optimizer_state_sha256: optimizer
            .map(|artifact| artifact.sha256.clone())
            .unwrap_or_default(),
        scheduler_cursor_sha256: scheduler
            .map(|artifact| artifact.sha256.clone())
            .unwrap_or_default(),
        checkpoint_verified,
        secret_scan_passed,
        bootstrap_accepted,
        refusal_reason,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

pub fn qwen_legal_checkpoint_settlement_gate(
    run_id: impl Into<String>,
    checkpoint_verified: bool,
    worker_receipt_verified: bool,
    transfer_receipts_verified: bool,
) -> QwenLegalCheckpointSettlementGateReceipt {
    let status = if !checkpoint_verified || !transfer_receipts_verified {
        QwenLegalCheckpointSettlementStatus::WithheldCheckpointUnverified
    } else if !worker_receipt_verified {
        QwenLegalCheckpointSettlementStatus::WithheldWorkerReceiptUnverified
    } else {
        QwenLegalCheckpointSettlementStatus::Payable
    };
    let reason = match status {
        QwenLegalCheckpointSettlementStatus::WithheldCheckpointUnverified => {
            "checkpoint or transfer receipt verification has not passed"
        }
        QwenLegalCheckpointSettlementStatus::WithheldWorkerReceiptUnverified => {
            "worker receipt verification has not passed"
        }
        QwenLegalCheckpointSettlementStatus::Payable => {
            "checkpoint, transfer receipts, and worker receipt are verified"
        }
    };
    let run_id = run_id.into();
    let mut receipt = QwenLegalCheckpointSettlementGateReceipt {
        schema_version: String::from(QWEN_LEGAL_CHECKPOINT_SETTLEMENT_GATE_SCHEMA_VERSION),
        gate_id: format!("gate.{run_id}.checkpoint_settlement"),
        run_id,
        checkpoint_verified,
        worker_receipt_verified,
        transfer_receipts_verified,
        status,
        reason: String::from(reason),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn artifact_pointer(
    artifact_id: impl Into<String>,
    family: QwenLegalCheckpointArtifactFamily,
    path: &Path,
) -> Result<QwenLegalCheckpointArtifactPointer, QwenLegalCheckpointRecoveryError> {
    let bytes = fs::read(path).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(QwenLegalCheckpointArtifactPointer {
        artifact_id: artifact_id.into(),
        family,
        path: path.display().to_string(),
        sha256: sha256_hex(bytes.as_slice()),
        byte_len: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        secret_free: artifact_is_secret_free(path, bytes.as_slice()),
    })
}

fn artifact_is_secret_free(path: &Path, bytes: &[u8]) -> bool {
    let path_text = path.display().to_string().to_ascii_lowercase();
    let body = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    ![
        "password",
        "private_key",
        "wallet",
        "mnemonic",
        "api_key",
        "bearer",
    ]
    .iter()
    .any(|needle| path_text.contains(needle) || body.contains(needle))
}

fn deterministic_adapter_state(base_state: &str, step: u64) -> String {
    resume_adapter_state(base_state, 1, step)
}

fn resume_adapter_state(start_state: &str, start_step: u64, final_step: u64) -> String {
    let mut state = String::from(start_state);
    for step in start_step..=final_step {
        state = sha256_hex(format!("{state}|qwen-legal-step|{step}").as_bytes());
    }
    state
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), QwenLegalCheckpointRecoveryError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    fs::write(path, bytes).map_err(|error| QwenLegalCheckpointRecoveryError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), QwenLegalCheckpointRecoveryError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| QwenLegalCheckpointRecoveryError::Serialization(error.to_string()))?;
    write_bytes(path, bytes.as_slice())
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("stable digest serialization");
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen_checkpoint_recovery_rehearsal_proves_exact_resume_and_late_join()
    -> Result<(), QwenLegalCheckpointRecoveryError> {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = write_qwen_legal_checkpoint_recovery_rehearsal(dir.path())?;
        assert!(report.exact_resume_match);
        assert!(report.accepted_divergence_reason.is_none());
        assert_eq!(report.last_accepted_step, 2);
        assert!(report.stream_verifications.iter().all(|row| row.verified));
        assert!(report.late_join_bootstrap.bootstrap_accepted);
        assert_eq!(
            report.settlement_gate_before_receipt_verification.status,
            QwenLegalCheckpointSettlementStatus::WithheldWorkerReceiptUnverified
        );
        assert_eq!(
            report.settlement_gate_after_receipt_verification.status,
            QwenLegalCheckpointSettlementStatus::Payable
        );
        assert!(Path::new(report.report_path.as_str()).is_file());
        Ok(())
    }

    #[test]
    fn qwen_checkpoint_stream_verifier_catches_truncated_upload()
    -> Result<(), QwenLegalCheckpointRecoveryError> {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.bin");
        let dest = dir.path().join("dest.bin");
        write_bytes(source.as_path(), b"abcdefghijklmnopqrstuvwxyz")?;
        let receipt = stream_qwen_legal_checkpoint_artifact(
            source.as_path(),
            dest.as_path(),
            "artifact.truncated",
            QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
            5,
            0,
        )?;
        write_bytes(dest.as_path(), b"abcdefghijklmnopqrstuvwxy")?;
        let error = verify_qwen_legal_checkpoint_stream_receipt(&receipt)
            .expect_err("truncated destination should fail");
        assert!(error.to_string().contains("byte length drifted"));
        Ok(())
    }

    #[test]
    fn qwen_checkpoint_stream_verifier_catches_reordered_chunks()
    -> Result<(), QwenLegalCheckpointRecoveryError> {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.bin");
        let dest = dir.path().join("dest.bin");
        write_bytes(source.as_path(), b"abcdefghijklmnopqrstuvwxyz")?;
        let mut receipt = stream_qwen_legal_checkpoint_artifact(
            source.as_path(),
            dest.as_path(),
            "artifact.reordered",
            QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
            5,
            0,
        )?;
        receipt.chunks.swap(0, 1);
        receipt.stream_receipt_digest = receipt.stable_digest();
        let error = verify_qwen_legal_checkpoint_stream_receipt(&receipt)
            .expect_err("reordered chunks should fail");
        assert!(error.to_string().contains("reordered"));
        Ok(())
    }

    #[test]
    fn qwen_checkpoint_stream_verifier_catches_chunk_hash_drift()
    -> Result<(), QwenLegalCheckpointRecoveryError> {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.bin");
        let dest = dir.path().join("dest.bin");
        write_bytes(source.as_path(), b"abcdefghijklmnopqrstuvwxyz")?;
        let mut receipt = stream_qwen_legal_checkpoint_artifact(
            source.as_path(),
            dest.as_path(),
            "artifact.hash-drift",
            QwenLegalCheckpointArtifactFamily::AdapterCheckpoint,
            5,
            0,
        )?;
        receipt.chunks[0].sha256 = sha256_hex(b"not the first chunk");
        receipt.stream_receipt_digest = receipt.stable_digest();
        let error = verify_qwen_legal_checkpoint_stream_receipt(&receipt)
            .expect_err("chunk hash drift should fail");
        assert!(error.to_string().contains("hash drifted"));
        Ok(())
    }

    #[test]
    fn qwen_checkpoint_settlement_blocks_until_checkpoint_and_worker_pass() {
        let no_checkpoint = qwen_legal_checkpoint_settlement_gate("run", false, true, true);
        assert_eq!(
            no_checkpoint.status,
            QwenLegalCheckpointSettlementStatus::WithheldCheckpointUnverified
        );
        let no_worker = qwen_legal_checkpoint_settlement_gate("run", true, false, true);
        assert_eq!(
            no_worker.status,
            QwenLegalCheckpointSettlementStatus::WithheldWorkerReceiptUnverified
        );
        let payable = qwen_legal_checkpoint_settlement_gate("run", true, true, true);
        assert_eq!(payable.status, QwenLegalCheckpointSettlementStatus::Payable);
    }
}
