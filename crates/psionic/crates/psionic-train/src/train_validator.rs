use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    load_psionic_train_grouped_stage_execution_summary, load_psionic_train_grouped_stage_transport,
    maybe_record_psionic_train_weak_device_validation_replay_proof,
    persist_psionic_train_grouped_stage_replay_evidence,
    psionic_train_resolved_artifact_cache_candidates, PsionicTrainCheckpointSurface,
    PsionicTrainContributionArtifact, PsionicTrainContributionArtifactManifest,
    PsionicTrainContributionReceipt, PsionicTrainGroupedReplicaEvidenceError,
    PsionicTrainGroupedReplicaStageExecutionSummary, PsionicTrainGroupedReplicaStageReplayEvidence,
    PsionicTrainInvocationManifest, PsionicTrainOutcomeKind, PsionicTrainWorkClass,
    TrainingExecutionValidatorDisposition, PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR,
};

pub const PSIONIC_TRAIN_VALIDATOR_SCORE_ARTIFACT_SCHEMA_VERSION: &str =
    "psionic.train.validator_score_artifact.v1";
pub const PSIONIC_TRAIN_VALIDATOR_SCORE_RECEIPT_SCHEMA_VERSION: &str =
    "psionic.train.validator_score_receipt.v1";
pub const PSIONIC_TRAIN_VALIDATOR_QUALITY_DRIFT_SIGNAL_SCHEMA_VERSION: &str =
    "psionic.train.validator_quality_drift_signal.v1";
pub const PSIONIC_TRAIN_VALIDATOR_ROLLBACK_SIGNAL_SCHEMA_VERSION: &str =
    "psionic.train.validator_rollback_signal.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainValidatorReplayReasonCode {
    ContributionOutcomeRefused,
    PrimaryCheckpointAccepted,
    CheckpointRecovered,
    CheckpointReplayRequired,
    GroupedStageEvidenceVerified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainValidatorHook {
    AssignmentCorrectness,
    CheckpointLineage,
    WorkExecutionPlausibility,
    UpdateIntegrity,
    GroupedStageIntegrity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainValidatorQualityDriftState {
    Baseline,
    Stable,
    Improved,
    Regressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainValidatorRollbackPosture {
    Hold,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainValidatorScoreArtifact {
    pub schema_version: String,
    pub lane_id: String,
    pub validator_work_class: PsionicTrainWorkClass,
    pub challenged_work_class: PsionicTrainWorkClass,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub challenged_run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub challenge_id: String,
    pub validator_node_pubkey: String,
    pub challenged_node_pubkey: String,
    pub contribution_id: String,
    pub contribution_digest: String,
    pub artifact_manifest_digest: String,
    pub artifact_count: usize,
    pub grouped_stage_execution_summary_path: Option<String>,
    pub grouped_stage_execution_summary_digest: Option<String>,
    pub grouped_stage_replay_evidence_path: Option<String>,
    pub grouped_stage_replay_evidence_digest: Option<String>,
    pub checkpoint_pointer_state: Option<String>,
    pub checkpoint_manifest_digest: Option<String>,
    pub checkpoint_object_digest: Option<String>,
    pub validation_index: u64,
    pub verified_hooks: Vec<PsionicTrainValidatorHook>,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub reason_codes: Vec<PsionicTrainValidatorReplayReasonCode>,
    pub score_bps: u16,
    pub detail: String,
    pub score_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainValidatorScoreReceipt {
    pub schema_version: String,
    pub lane_id: String,
    pub validator_work_class: PsionicTrainWorkClass,
    pub challenged_work_class: PsionicTrainWorkClass,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub challenged_run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub challenge_id: String,
    pub validator_node_pubkey: String,
    pub challenged_node_pubkey: String,
    pub contribution_id: String,
    pub contribution_digest: String,
    pub artifact_manifest_digest: String,
    pub grouped_stage_execution_summary_path: Option<String>,
    pub grouped_stage_execution_summary_digest: Option<String>,
    pub grouped_stage_replay_evidence_path: Option<String>,
    pub grouped_stage_replay_evidence_digest: Option<String>,
    pub validation_index: u64,
    pub verified_hooks: Vec<PsionicTrainValidatorHook>,
    pub disposition: TrainingExecutionValidatorDisposition,
    pub reason_codes: Vec<PsionicTrainValidatorReplayReasonCode>,
    pub score_bps: u16,
    pub score_artifact_path: String,
    pub score_artifact_digest: String,
    pub detail: String,
    pub score_receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainValidatorQualityDriftSignal {
    pub schema_version: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub challenged_run_id: String,
    pub challenged_work_class: PsionicTrainWorkClass,
    pub current_window_id: String,
    pub current_assignment_id: String,
    pub current_challenge_id: String,
    pub validation_index: u64,
    pub validator_score_receipt_path: String,
    pub validator_score_receipt_digest: String,
    pub previous_validation_index: Option<u64>,
    pub previous_window_id: Option<String>,
    pub previous_assignment_id: Option<String>,
    pub previous_challenge_id: Option<String>,
    pub previous_score_bps: Option<u16>,
    pub previous_disposition: Option<TrainingExecutionValidatorDisposition>,
    pub current_score_bps: u16,
    pub current_disposition: TrainingExecutionValidatorDisposition,
    pub score_bps_delta: Option<i32>,
    pub trailing_window_count: usize,
    pub degraded_window_count: usize,
    pub non_accepted_window_count: usize,
    pub drift_state: PsionicTrainValidatorQualityDriftState,
    pub detail: String,
    pub drift_signal_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainValidatorRollbackSignal {
    pub schema_version: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub challenged_run_id: String,
    pub challenged_work_class: PsionicTrainWorkClass,
    pub current_window_id: String,
    pub current_assignment_id: String,
    pub current_challenge_id: String,
    pub validation_index: u64,
    pub validator_score_receipt_path: String,
    pub validator_score_receipt_digest: String,
    pub quality_drift_signal_path: String,
    pub quality_drift_signal_digest: String,
    pub rollback_posture: PsionicTrainValidatorRollbackPosture,
    pub degraded_window_count: usize,
    pub consecutive_non_accepted_window_count: usize,
    pub rollback_baseline_validation_index: Option<u64>,
    pub rollback_baseline_window_id: Option<String>,
    pub rollback_baseline_assignment_id: Option<String>,
    pub rollback_baseline_challenge_id: Option<String>,
    pub rollback_baseline_score_bps: Option<u16>,
    pub rollback_baseline_disposition: Option<TrainingExecutionValidatorDisposition>,
    pub detail: String,
    pub rollback_signal_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainValidatorArtifactOutputs {
    pub validator_score_receipt_path: String,
    pub grouped_stage_replay_evidence_path: Option<String>,
    pub validator_quality_drift_signal_path: String,
    pub validator_rollback_signal_path: String,
    pub weak_device_validation_replay_proof_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PsionicTrainValidatorReplayExecution {
    pub artifacts: PsionicTrainValidatorArtifactOutputs,
    pub score_receipt: PsionicTrainValidatorScoreReceipt,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum PsionicTrainValidatorReplayError {
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("validator replay input is stale: {detail}")]
    StaleAssignment { detail: String },
    #[error("validator replay input drifted: {detail}")]
    ArtifactDigestMismatch { detail: String },
    #[error("validator replay input is incomplete: {detail}")]
    ArtifactIncomplete { detail: String },
    #[error("validator replay is missing checkpoint state: {detail}")]
    CheckpointMissing { detail: String },
}

pub fn execute_psionic_train_validator_replay(
    manifest: &PsionicTrainInvocationManifest,
    run_root: &Path,
) -> Result<PsionicTrainValidatorReplayExecution, PsionicTrainValidatorReplayError> {
    let challenged_work_class = manifest
        .validator_target_work_class
        .expect("validated validator manifests always carry validator_target_work_class");
    let contribution_receipt_binding = manifest
        .validator_target_contribution_receipt
        .as_ref()
        .expect("validated validator manifests always carry target contribution receipt binding");
    let contribution_receipt_path = materialize_validator_artifact_binding(
        contribution_receipt_binding,
        run_root,
        "invocation_manifest.validator_target_contribution_receipt",
    )?;
    let contribution_receipt: PsionicTrainContributionReceipt =
        read_json(contribution_receipt_path.as_path())?;
    if contribution_receipt.contribution_digest != contribution_receipt.stable_contribution_digest()
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: format!(
                "contribution receipt `{}` digest does not match its canonical contents",
                contribution_receipt_path.display()
            ),
        });
    }

    let contribution_artifact_manifest_binding = manifest
        .validator_target_contribution_artifact_manifest
        .as_ref()
        .expect("validated validator manifests always carry target contribution artifact manifest binding");
    let contribution_artifact_manifest_path = materialize_validator_artifact_binding(
        contribution_artifact_manifest_binding,
        run_root,
        "invocation_manifest.validator_target_contribution_artifact_manifest",
    )?;
    let contribution_artifact_manifest: PsionicTrainContributionArtifactManifest =
        read_json(contribution_artifact_manifest_path.as_path())?;
    if contribution_artifact_manifest.artifact_manifest_digest
        != contribution_artifact_manifest.stable_artifact_manifest_digest()
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: format!(
                "contribution artifact manifest `{}` digest does not match its canonical contents",
                contribution_artifact_manifest_path.display()
            ),
        });
    }
    let contribution_artifact_manifest =
        materialize_contribution_artifact_manifest(&contribution_artifact_manifest, run_root)?;
    if contribution_receipt
        .artifact_manifest
        .artifact_ref
        .artifact_id
        != contribution_artifact_manifest_binding
            .artifact_ref
            .artifact_id
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: format!(
                "contribution receipt references artifact `{}` but validator replay targeted `{}`",
                contribution_receipt
                    .artifact_manifest
                    .artifact_ref
                    .artifact_id,
                contribution_artifact_manifest_binding
                    .artifact_ref
                    .artifact_id
            ),
        });
    }
    if contribution_receipt
        .artifact_manifest
        .artifact_ref
        .artifact_digest
        .as_deref()
        != contribution_artifact_manifest_binding
            .artifact_ref
            .artifact_digest
            .as_deref()
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "contribution receipt artifact-manifest binding digest does not match the replay target binding",
            ),
        });
    }
    if contribution_receipt.artifact_manifest_digest
        != contribution_artifact_manifest.artifact_manifest_digest
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "contribution receipt artifact-manifest digest does not match the replayed manifest",
            ),
        });
    }
    if contribution_receipt.work_class != challenged_work_class
        || contribution_artifact_manifest.work_class != challenged_work_class
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: format!(
                "validator replay targeted work_class={} but the challenged contribution retained receipt/manifests for work_class={}/{}",
                challenged_work_class.label(),
                contribution_receipt.work_class.label(),
                contribution_artifact_manifest.work_class.label()
            ),
        });
    }
    if contribution_receipt.lane_id != manifest.lane_id
        || contribution_artifact_manifest.lane_id != manifest.lane_id
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "validator replay lane id does not match the challenged contribution lane",
            ),
        });
    }
    if contribution_receipt.window_id != contribution_artifact_manifest.window_id
        || contribution_receipt.assignment_id != contribution_artifact_manifest.assignment_id
        || contribution_receipt.contribution_id != contribution_artifact_manifest.contribution_id
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "challenged contribution receipt drifted from the challenged artifact manifest",
            ),
        });
    }
    if manifest.coordination.window_id.as_deref() != Some(contribution_receipt.window_id.as_str())
        || manifest.coordination.assignment_id.as_deref()
            != Some(contribution_receipt.assignment_id.as_str())
    {
        return Err(PsionicTrainValidatorReplayError::StaleAssignment {
            detail: format!(
                "validator replay targets window `{}` assignment `{}` but the challenged contribution belongs to window `{}` assignment `{}`",
                manifest.coordination.window_id.as_deref().unwrap_or(""),
                manifest.coordination.assignment_id.as_deref().unwrap_or(""),
                contribution_receipt.window_id,
                contribution_receipt.assignment_id
            ),
        });
    }
    let grouped_stage_execution_summary = load_grouped_stage_execution_summary(
        challenged_work_class,
        &contribution_receipt,
        &contribution_artifact_manifest,
    )?;
    let verified_hooks = validator_hooks_for_target_work_class(challenged_work_class);
    let grouped_stage_execution_summary_path = grouped_stage_execution_summary
        .as_ref()
        .map(|_| {
            require_artifact(
                &contribution_artifact_manifest,
                "grouped_stage_execution_summary",
            )
            .and_then(materialized_artifact_path)
        })
        .transpose()?;

    let checkpoint_surface = load_checkpoint_surface(&contribution_artifact_manifest)?;
    let (disposition, reason_codes, score_bps, detail, checkpoint_pointer_state) =
        classify_validator_result(
            &contribution_receipt,
            checkpoint_surface.as_ref(),
            grouped_stage_execution_summary.is_some(),
        )?;

    let challenge_id = manifest
        .coordination
        .challenge_id
        .as_deref()
        .expect("validated validator manifests always carry challenge_id");
    let validator_node_pubkey = manifest
        .coordination
        .node_pubkey
        .as_deref()
        .expect("validated validator manifests always carry validator node_pubkey");
    let validator_run_id = manifest
        .run_id
        .clone()
        .or_else(|| {
            run_root
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| String::from("validator_run"));
    let validator_root = run_root
        .join("windows")
        .join(contribution_receipt.window_id.as_str())
        .join("validators")
        .join(validator_challenge_dir_name(challenge_id));
    fs::create_dir_all(&validator_root).map_err(|error| {
        PsionicTrainValidatorReplayError::Write {
            path: validator_root.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    let prior_score_receipts = load_prior_validator_score_receipts(
        run_root,
        manifest.lane_id.as_str(),
        contribution_receipt.run_id.as_str(),
        challenged_work_class,
        contribution_receipt.window_id.as_str(),
        contribution_receipt.assignment_id.as_str(),
        challenge_id,
    )?;
    let validation_index = prior_score_receipts
        .iter()
        .map(|receipt| receipt.validation_index)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let grouped_stage_replay_evidence = grouped_stage_execution_summary
        .as_ref()
        .map(|execution_summary| {
            persist_grouped_stage_replay_evidence(
                manifest,
                &contribution_receipt,
                &contribution_artifact_manifest,
                execution_summary,
                &validator_root,
                challenge_id,
                validator_node_pubkey,
                validator_run_id.as_str(),
                disposition,
                &reason_codes,
                score_bps,
                detail.as_str(),
            )
        })
        .transpose()?;
    let score_artifact_path = validator_root.join("validator_score_artifact.json");
    let mut score_artifact = PsionicTrainValidatorScoreArtifact {
        schema_version: String::from(PSIONIC_TRAIN_VALIDATOR_SCORE_ARTIFACT_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        validator_work_class: manifest.work_class,
        challenged_work_class,
        network_id: manifest.coordination.network_id.clone(),
        validator_run_id: validator_run_id.clone(),
        challenged_run_id: contribution_receipt.run_id.clone(),
        window_id: contribution_receipt.window_id.clone(),
        assignment_id: contribution_receipt.assignment_id.clone(),
        challenge_id: String::from(challenge_id),
        validator_node_pubkey: String::from(validator_node_pubkey),
        challenged_node_pubkey: contribution_receipt.node_pubkey.clone(),
        contribution_id: contribution_receipt.contribution_id.clone(),
        contribution_digest: contribution_receipt.contribution_digest.clone(),
        artifact_manifest_digest: contribution_artifact_manifest
            .artifact_manifest_digest
            .clone(),
        artifact_count: contribution_artifact_manifest.artifact_count,
        grouped_stage_execution_summary_path: grouped_stage_execution_summary_path.clone(),
        grouped_stage_execution_summary_digest: grouped_stage_execution_summary
            .as_ref()
            .map(|value| value.execution_summary_digest.clone()),
        grouped_stage_replay_evidence_path: grouped_stage_replay_evidence
            .as_ref()
            .map(|value| value.grouped_stage_replay_evidence_path.clone()),
        grouped_stage_replay_evidence_digest: grouped_stage_replay_evidence
            .as_ref()
            .map(|value| value.grouped_stage_replay_evidence_digest.clone()),
        checkpoint_pointer_state,
        checkpoint_manifest_digest: checkpoint_surface
            .as_ref()
            .and_then(|value| value.checkpoint_manifest_digest.clone()),
        checkpoint_object_digest: checkpoint_surface
            .as_ref()
            .and_then(|value| value.checkpoint_object_digest.clone()),
        validation_index,
        verified_hooks: verified_hooks.clone(),
        disposition,
        reason_codes: reason_codes.clone(),
        score_bps,
        detail: detail.clone(),
        score_digest: String::new(),
    };
    score_artifact.score_digest =
        stable_digest(b"psionic_train_validator_score_artifact|", &score_artifact);
    write_json(score_artifact_path.as_path(), &score_artifact)?;

    let score_receipt_path = validator_root.join("validator_score_receipt.json");
    let mut score_receipt = PsionicTrainValidatorScoreReceipt {
        schema_version: String::from(PSIONIC_TRAIN_VALIDATOR_SCORE_RECEIPT_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        validator_work_class: manifest.work_class,
        challenged_work_class,
        network_id: manifest.coordination.network_id.clone(),
        validator_run_id,
        challenged_run_id: contribution_receipt.run_id.clone(),
        window_id: contribution_receipt.window_id.clone(),
        assignment_id: contribution_receipt.assignment_id.clone(),
        challenge_id: String::from(challenge_id),
        validator_node_pubkey: String::from(validator_node_pubkey),
        challenged_node_pubkey: contribution_receipt.node_pubkey.clone(),
        contribution_id: contribution_receipt.contribution_id.clone(),
        contribution_digest: contribution_receipt.contribution_digest.clone(),
        artifact_manifest_digest: contribution_artifact_manifest
            .artifact_manifest_digest
            .clone(),
        grouped_stage_execution_summary_path: grouped_stage_execution_summary_path,
        grouped_stage_execution_summary_digest: grouped_stage_execution_summary
            .as_ref()
            .map(|value| value.execution_summary_digest.clone()),
        grouped_stage_replay_evidence_path: grouped_stage_replay_evidence
            .as_ref()
            .map(|value| value.grouped_stage_replay_evidence_path.clone()),
        grouped_stage_replay_evidence_digest: grouped_stage_replay_evidence
            .as_ref()
            .map(|value| value.grouped_stage_replay_evidence_digest.clone()),
        validation_index,
        verified_hooks,
        disposition,
        reason_codes,
        score_bps,
        score_artifact_path: score_artifact_path.display().to_string(),
        score_artifact_digest: score_artifact.score_digest.clone(),
        detail: detail.clone(),
        score_receipt_digest: String::new(),
    };
    score_receipt.score_receipt_digest =
        stable_digest(b"psionic_train_validator_score_receipt|", &score_receipt);
    write_json(score_receipt_path.as_path(), &score_receipt)?;

    let quality_drift_signal_path = validator_root.join("validator_quality_drift_signal.json");
    let mut quality_drift_signal = build_validator_quality_drift_signal(
        manifest,
        &score_receipt,
        score_receipt_path.as_path(),
        &prior_score_receipts,
    );
    quality_drift_signal.drift_signal_digest = stable_digest(
        b"psionic_train_validator_quality_drift_signal|",
        &quality_drift_signal,
    );
    write_json(quality_drift_signal_path.as_path(), &quality_drift_signal)?;

    let rollback_signal_path = validator_root.join("validator_rollback_signal.json");
    let mut rollback_signal = build_validator_rollback_signal(
        manifest,
        &score_receipt,
        score_receipt_path.as_path(),
        quality_drift_signal_path.as_path(),
        &quality_drift_signal,
        &prior_score_receipts,
    );
    rollback_signal.rollback_signal_digest = stable_digest(
        b"psionic_train_validator_rollback_signal|",
        &rollback_signal,
    );
    write_json(rollback_signal_path.as_path(), &rollback_signal)?;

    let weak_device_validation_replay_proof_path =
        validator_root.join("weak_device_validation_replay_proof.json");
    let weak_device_validation_replay_proof =
        maybe_record_psionic_train_weak_device_validation_replay_proof(
            weak_device_validation_replay_proof_path.as_path(),
            manifest,
            contribution_receipt_path.as_path(),
            &contribution_receipt,
            contribution_artifact_manifest_path.as_path(),
            &contribution_artifact_manifest,
            score_artifact_path.as_path(),
            &score_artifact,
            score_receipt_path.as_path(),
            &score_receipt,
            quality_drift_signal_path.as_path(),
            &quality_drift_signal,
            rollback_signal_path.as_path(),
            &rollback_signal,
        )
        .map_err(
            |error| PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: error.to_string(),
            },
        )?;

    Ok(PsionicTrainValidatorReplayExecution {
        artifacts: PsionicTrainValidatorArtifactOutputs {
            validator_score_receipt_path: score_receipt_path.display().to_string(),
            grouped_stage_replay_evidence_path: grouped_stage_replay_evidence
                .map(|value| value.grouped_stage_replay_evidence_path),
            validator_quality_drift_signal_path: quality_drift_signal_path.display().to_string(),
            validator_rollback_signal_path: rollback_signal_path.display().to_string(),
            weak_device_validation_replay_proof_path: weak_device_validation_replay_proof.map(
                |_| {
                    weak_device_validation_replay_proof_path
                        .display()
                        .to_string()
                },
            ),
        },
        score_receipt,
        detail,
    })
}

fn load_prior_validator_score_receipts(
    run_root: &Path,
    lane_id: &str,
    challenged_run_id: &str,
    challenged_work_class: PsionicTrainWorkClass,
    current_window_id: &str,
    current_assignment_id: &str,
    current_challenge_id: &str,
) -> Result<Vec<PsionicTrainValidatorScoreReceipt>, PsionicTrainValidatorReplayError> {
    let windows_root = run_root.join("windows");
    if !windows_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut receipts = Vec::new();
    for window_entry in
        fs::read_dir(&windows_root).map_err(|error| PsionicTrainValidatorReplayError::Read {
            path: windows_root.display().to_string(),
            detail: error.to_string(),
        })?
    {
        let window_entry =
            window_entry.map_err(|error| PsionicTrainValidatorReplayError::Read {
                path: windows_root.display().to_string(),
                detail: error.to_string(),
            })?;
        let validators_root = window_entry.path().join("validators");
        if !validators_root.is_dir() {
            continue;
        }
        for validator_entry in fs::read_dir(&validators_root).map_err(|error| {
            PsionicTrainValidatorReplayError::Read {
                path: validators_root.display().to_string(),
                detail: error.to_string(),
            }
        })? {
            let validator_entry =
                validator_entry.map_err(|error| PsionicTrainValidatorReplayError::Read {
                    path: validators_root.display().to_string(),
                    detail: error.to_string(),
                })?;
            let score_receipt_path = validator_entry.path().join("validator_score_receipt.json");
            if !score_receipt_path.is_file() {
                continue;
            }
            let receipt: PsionicTrainValidatorScoreReceipt =
                read_json(score_receipt_path.as_path())?;
            if receipt.lane_id != lane_id
                || receipt.challenged_run_id != challenged_run_id
                || receipt.challenged_work_class != challenged_work_class
                || (receipt.window_id == current_window_id
                    && receipt.assignment_id == current_assignment_id
                    && receipt.challenge_id == current_challenge_id)
            {
                continue;
            }
            receipts.push(receipt);
        }
    }
    receipts.sort_by_key(|receipt| receipt.validation_index);
    Ok(receipts)
}

fn build_validator_quality_drift_signal(
    manifest: &PsionicTrainInvocationManifest,
    score_receipt: &PsionicTrainValidatorScoreReceipt,
    score_receipt_path: &Path,
    prior_score_receipts: &[PsionicTrainValidatorScoreReceipt],
) -> PsionicTrainValidatorQualityDriftSignal {
    let previous_receipt = prior_score_receipts.last();
    let previous_score_bps = previous_receipt.map(|receipt| receipt.score_bps);
    let previous_disposition = previous_receipt.map(|receipt| receipt.disposition);
    let score_bps_delta =
        previous_score_bps.map(|score| i32::from(score_receipt.score_bps) - i32::from(score));
    let drift_state = previous_receipt.map_or(
        PsionicTrainValidatorQualityDriftState::Baseline,
        |previous_receipt| compare_validator_quality(previous_receipt, score_receipt),
    );
    let trailing_window_count = prior_score_receipts.len() + 1;
    let degraded_window_count = count_degraded_windows(prior_score_receipts, score_receipt);
    let non_accepted_window_count = prior_score_receipts
        .iter()
        .chain(std::iter::once(score_receipt))
        .filter(|receipt| receipt.disposition != TrainingExecutionValidatorDisposition::Accepted)
        .count();
    let detail = match previous_receipt {
        None => String::from(
            "validator quality drift signal initialized the first retained score for this challenged run and work-class scope",
        ),
        Some(previous_receipt) => match drift_state {
            PsionicTrainValidatorQualityDriftState::Baseline => String::from(
                "validator quality drift signal initialized the first retained score for this challenged run and work-class scope",
            ),
            PsionicTrainValidatorQualityDriftState::Stable => format!(
                "validator quality remained stable across windows: {} {}bps -> {} {}bps",
                validator_disposition_label(previous_receipt.disposition),
                previous_receipt.score_bps,
                validator_disposition_label(score_receipt.disposition),
                score_receipt.score_bps
            ),
            PsionicTrainValidatorQualityDriftState::Improved => format!(
                "validator quality improved across windows: {} {}bps -> {} {}bps",
                validator_disposition_label(previous_receipt.disposition),
                previous_receipt.score_bps,
                validator_disposition_label(score_receipt.disposition),
                score_receipt.score_bps
            ),
            PsionicTrainValidatorQualityDriftState::Regressed => format!(
                "validator quality regressed across windows: {} {}bps -> {} {}bps",
                validator_disposition_label(previous_receipt.disposition),
                previous_receipt.score_bps,
                validator_disposition_label(score_receipt.disposition),
                score_receipt.score_bps
            ),
        },
    };
    PsionicTrainValidatorQualityDriftSignal {
        schema_version: String::from(PSIONIC_TRAIN_VALIDATOR_QUALITY_DRIFT_SIGNAL_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        network_id: manifest.coordination.network_id.clone(),
        validator_run_id: score_receipt.validator_run_id.clone(),
        challenged_run_id: score_receipt.challenged_run_id.clone(),
        challenged_work_class: score_receipt.challenged_work_class,
        current_window_id: score_receipt.window_id.clone(),
        current_assignment_id: score_receipt.assignment_id.clone(),
        current_challenge_id: score_receipt.challenge_id.clone(),
        validation_index: score_receipt.validation_index,
        validator_score_receipt_path: score_receipt_path.display().to_string(),
        validator_score_receipt_digest: score_receipt.score_receipt_digest.clone(),
        previous_validation_index: previous_receipt.map(|receipt| receipt.validation_index),
        previous_window_id: previous_receipt.map(|receipt| receipt.window_id.clone()),
        previous_assignment_id: previous_receipt.map(|receipt| receipt.assignment_id.clone()),
        previous_challenge_id: previous_receipt.map(|receipt| receipt.challenge_id.clone()),
        previous_score_bps,
        previous_disposition,
        current_score_bps: score_receipt.score_bps,
        current_disposition: score_receipt.disposition,
        score_bps_delta,
        trailing_window_count,
        degraded_window_count,
        non_accepted_window_count,
        drift_state,
        detail,
        drift_signal_digest: String::new(),
    }
}

fn build_validator_rollback_signal(
    manifest: &PsionicTrainInvocationManifest,
    score_receipt: &PsionicTrainValidatorScoreReceipt,
    score_receipt_path: &Path,
    quality_drift_signal_path: &Path,
    quality_drift_signal: &PsionicTrainValidatorQualityDriftSignal,
    prior_score_receipts: &[PsionicTrainValidatorScoreReceipt],
) -> PsionicTrainValidatorRollbackSignal {
    let rollback_baseline = prior_score_receipts
        .iter()
        .rev()
        .find(|receipt| receipt.disposition == TrainingExecutionValidatorDisposition::Accepted);
    let consecutive_non_accepted_window_count = prior_score_receipts
        .iter()
        .rev()
        .take_while(|receipt| {
            receipt.disposition != TrainingExecutionValidatorDisposition::Accepted
        })
        .count()
        + usize::from(score_receipt.disposition != TrainingExecutionValidatorDisposition::Accepted);
    let rollback_posture = if rollback_baseline.is_some()
        && quality_drift_signal.drift_state == PsionicTrainValidatorQualityDriftState::Regressed
    {
        PsionicTrainValidatorRollbackPosture::Candidate
    } else {
        PsionicTrainValidatorRollbackPosture::Hold
    };
    let detail = match (rollback_posture, rollback_baseline) {
        (PsionicTrainValidatorRollbackPosture::Hold, None) => String::from(
            "rollback posture remains hold because no prior accepted validation baseline exists for this challenged run and work-class scope",
        ),
        (PsionicTrainValidatorRollbackPosture::Hold, Some(_)) => String::from(
            "rollback posture remains hold because the current window did not regress below the retained accepted baseline",
        ),
        (PsionicTrainValidatorRollbackPosture::Candidate, Some(baseline)) => format!(
            "rollback candidate raised because validation window `{}` fell below accepted baseline window `{}` ({} {}bps -> {} {}bps)",
            score_receipt.window_id,
            baseline.window_id,
            validator_disposition_label(baseline.disposition),
            baseline.score_bps,
            validator_disposition_label(score_receipt.disposition),
            score_receipt.score_bps
        ),
        (PsionicTrainValidatorRollbackPosture::Candidate, None) => String::from(
            "rollback candidate raised because the current window regressed and no explicit accepted baseline was retained",
        ),
    };
    PsionicTrainValidatorRollbackSignal {
        schema_version: String::from(PSIONIC_TRAIN_VALIDATOR_ROLLBACK_SIGNAL_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        network_id: manifest.coordination.network_id.clone(),
        validator_run_id: score_receipt.validator_run_id.clone(),
        challenged_run_id: score_receipt.challenged_run_id.clone(),
        challenged_work_class: score_receipt.challenged_work_class,
        current_window_id: score_receipt.window_id.clone(),
        current_assignment_id: score_receipt.assignment_id.clone(),
        current_challenge_id: score_receipt.challenge_id.clone(),
        validation_index: score_receipt.validation_index,
        validator_score_receipt_path: score_receipt_path.display().to_string(),
        validator_score_receipt_digest: score_receipt.score_receipt_digest.clone(),
        quality_drift_signal_path: quality_drift_signal_path.display().to_string(),
        quality_drift_signal_digest: quality_drift_signal.drift_signal_digest.clone(),
        rollback_posture,
        degraded_window_count: quality_drift_signal.degraded_window_count,
        consecutive_non_accepted_window_count,
        rollback_baseline_validation_index: rollback_baseline
            .map(|receipt| receipt.validation_index),
        rollback_baseline_window_id: rollback_baseline.map(|receipt| receipt.window_id.clone()),
        rollback_baseline_assignment_id: rollback_baseline
            .map(|receipt| receipt.assignment_id.clone()),
        rollback_baseline_challenge_id: rollback_baseline
            .map(|receipt| receipt.challenge_id.clone()),
        rollback_baseline_score_bps: rollback_baseline.map(|receipt| receipt.score_bps),
        rollback_baseline_disposition: rollback_baseline.map(|receipt| receipt.disposition),
        detail,
        rollback_signal_digest: String::new(),
    }
}

fn compare_validator_quality(
    previous_receipt: &PsionicTrainValidatorScoreReceipt,
    current_receipt: &PsionicTrainValidatorScoreReceipt,
) -> PsionicTrainValidatorQualityDriftState {
    use PsionicTrainValidatorQualityDriftState::{Improved, Regressed, Stable};

    if current_receipt.score_bps > previous_receipt.score_bps {
        return Improved;
    }
    if current_receipt.score_bps < previous_receipt.score_bps {
        return Regressed;
    }
    let previous_severity = validator_disposition_severity(previous_receipt.disposition);
    let current_severity = validator_disposition_severity(current_receipt.disposition);
    if current_severity < previous_severity {
        Improved
    } else if current_severity > previous_severity {
        Regressed
    } else {
        Stable
    }
}

fn count_degraded_windows(
    prior_score_receipts: &[PsionicTrainValidatorScoreReceipt],
    current_receipt: &PsionicTrainValidatorScoreReceipt,
) -> usize {
    let mut degraded_window_count = 0;
    let mut previous_receipt = None;
    for receipt in prior_score_receipts
        .iter()
        .chain(std::iter::once(current_receipt))
    {
        if let Some(previous) = previous_receipt {
            if compare_validator_quality(previous, receipt)
                == PsionicTrainValidatorQualityDriftState::Regressed
            {
                degraded_window_count += 1;
            }
        }
        previous_receipt = Some(receipt);
    }
    degraded_window_count
}

fn validator_disposition_severity(disposition: TrainingExecutionValidatorDisposition) -> u8 {
    match disposition {
        TrainingExecutionValidatorDisposition::Accepted => 0,
        TrainingExecutionValidatorDisposition::Quarantined => 1,
        TrainingExecutionValidatorDisposition::ReplayRequired => 2,
        TrainingExecutionValidatorDisposition::Rejected => 3,
    }
}

fn validator_disposition_label(disposition: TrainingExecutionValidatorDisposition) -> &'static str {
    match disposition {
        TrainingExecutionValidatorDisposition::Accepted => "accepted",
        TrainingExecutionValidatorDisposition::Quarantined => "quarantined",
        TrainingExecutionValidatorDisposition::Rejected => "rejected",
        TrainingExecutionValidatorDisposition::ReplayRequired => "replay_required",
    }
}

fn load_checkpoint_surface(
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
) -> Result<Option<PsionicTrainCheckpointSurface>, PsionicTrainValidatorReplayError> {
    let Some(checkpoint_surface_artifact) = contribution_artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == "checkpoint_surface")
    else {
        return Ok(None);
    };
    read_json(Path::new(
        materialized_artifact_path(checkpoint_surface_artifact)?.as_str(),
    ))
    .map(Some)
}

fn load_grouped_stage_execution_summary(
    challenged_work_class: PsionicTrainWorkClass,
    contribution_receipt: &PsionicTrainContributionReceipt,
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
) -> Result<Option<PsionicTrainGroupedReplicaStageExecutionSummary>, PsionicTrainValidatorReplayError>
{
    match challenged_work_class {
        PsionicTrainWorkClass::GroupedReplicaStageExecution => {}
        PsionicTrainWorkClass::AdapterTraining
        | PsionicTrainWorkClass::SmallModelLocalTraining
        | PsionicTrainWorkClass::FullIslandLocalUpdateTraining => {
            if contribution_receipt.grouped_stage_assignment.is_some()
                || contribution_artifact_manifest
                    .grouped_stage_assignment
                    .is_some()
            {
                return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                    detail: format!(
                        "validator replay targeted work_class={} but the challenged contribution still retained grouped-stage worker state",
                        challenged_work_class.label()
                    ),
                });
            }
            return Ok(None);
        }
        PsionicTrainWorkClass::ValidationReplay
        | PsionicTrainWorkClass::Evaluation
        | PsionicTrainWorkClass::Aggregation
        | PsionicTrainWorkClass::CheckpointPromotion => {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: format!(
                    "validator replay does not yet implement replay hooks for work_class={}",
                    challenged_work_class.label()
                ),
            });
        }
    }

    let Some(grouped_stage_assignment) = contribution_receipt.grouped_stage_assignment.as_ref()
    else {
        return Err(PsionicTrainValidatorReplayError::ArtifactIncomplete {
            detail: String::from(
                "grouped stage validator replay requires grouped_stage_assignment on the challenged contribution receipt",
            ),
        });
    };
    if contribution_artifact_manifest
        .grouped_stage_assignment
        .as_ref()
        != Some(grouped_stage_assignment)
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "grouped stage validator replay observed grouped_stage_assignment drift between the challenged receipt and artifact manifest",
            ),
        });
    }
    let execution_summary_artifact = require_artifact(
        contribution_artifact_manifest,
        "grouped_stage_execution_summary",
    )?;
    let execution_summary = load_psionic_train_grouped_stage_execution_summary(Path::new(
        materialized_artifact_path(&execution_summary_artifact)?.as_str(),
    ))
    .map_err(map_grouped_stage_evidence_error)?;
    if execution_summary.lane_id != contribution_receipt.lane_id
        || execution_summary.run_id != contribution_receipt.run_id
        || execution_summary.window_id != contribution_receipt.window_id
        || execution_summary.assignment_id != contribution_receipt.assignment_id
        || execution_summary.contribution_id != contribution_receipt.contribution_id
        || execution_summary.node_pubkey != contribution_receipt.node_pubkey
        || execution_summary.grouped_stage_assignment != *grouped_stage_assignment
    {
        return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
            detail: String::from(
                "grouped stage execution summary drifted from the challenged contribution receipt",
            ),
        });
    }
    validate_grouped_stage_execution_summary_artifacts(
        contribution_artifact_manifest,
        &execution_summary,
    )?;
    Ok(Some(execution_summary))
}

fn validate_grouped_stage_execution_summary_artifacts(
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
    execution_summary: &PsionicTrainGroupedReplicaStageExecutionSummary,
) -> Result<(), PsionicTrainValidatorReplayError> {
    if let Some(path) = execution_summary.input_transport_path.as_deref() {
        let input_transport_artifact = require_artifact(
            contribution_artifact_manifest,
            "grouped_stage_input_transport",
        )?;
        if input_transport_artifact
            .binding
            .materialized_path
            .as_deref()
            != Some(path)
        {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary input transport path drifted from the artifact manifest",
                ),
            });
        }
        let loaded_transport = load_psionic_train_grouped_stage_transport(Path::new(path))
            .map_err(map_grouped_stage_transport_error)?;
        if execution_summary.input_transport_digest.as_deref()
            != Some(loaded_transport.envelope.transport_digest.as_str())
            || execution_summary.input_payload_digest.as_deref()
                != Some(loaded_transport.payload.payload_digest.as_str())
        {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary input transport digests drifted from the retained transport envelope",
                ),
            });
        }
    }
    if let Some(path) = execution_summary.output_transport_path.as_deref() {
        let output_transport_artifact = require_artifact(
            contribution_artifact_manifest,
            "grouped_stage_output_transport",
        )?;
        if output_transport_artifact
            .binding
            .materialized_path
            .as_deref()
            != Some(path)
        {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary output transport path drifted from the artifact manifest",
                ),
            });
        }
        let output_payload_path = execution_summary.output_payload_path.as_deref().ok_or_else(|| {
            PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary declared one output transport without one output payload path",
                ),
            }
        })?;
        let output_payload_artifact = require_artifact(
            contribution_artifact_manifest,
            "grouped_stage_output_payload",
        )?;
        if output_payload_artifact.binding.materialized_path.as_deref() != Some(output_payload_path)
        {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary output payload path drifted from the artifact manifest",
                ),
            });
        }
        let loaded_transport = load_psionic_train_grouped_stage_transport(Path::new(path))
            .map_err(map_grouped_stage_transport_error)?;
        if execution_summary.output_transport_digest.as_deref()
            != Some(loaded_transport.envelope.transport_digest.as_str())
            || execution_summary.output_payload_digest.as_deref()
                != Some(loaded_transport.payload.payload_digest.as_str())
        {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary output transport digests drifted from the retained transport envelope",
                ),
            });
        }
        if loaded_transport.envelope.payload_path != output_payload_path {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: String::from(
                    "grouped stage execution summary output payload path drifted from the retained transport envelope",
                ),
            });
        }
    }
    Ok(())
}

fn persist_grouped_stage_replay_evidence(
    manifest: &PsionicTrainInvocationManifest,
    contribution_receipt: &PsionicTrainContributionReceipt,
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
    execution_summary: &PsionicTrainGroupedReplicaStageExecutionSummary,
    validator_root: &Path,
    challenge_id: &str,
    validator_node_pubkey: &str,
    validator_run_id: &str,
    disposition: TrainingExecutionValidatorDisposition,
    reason_codes: &[PsionicTrainValidatorReplayReasonCode],
    score_bps: u16,
    detail: &str,
) -> Result<
    crate::PsionicTrainGroupedReplicaStageReplayEvidenceArtifacts,
    PsionicTrainValidatorReplayError,
> {
    let path = validator_root.join("grouped_stage_replay_evidence.json");
    persist_psionic_train_grouped_stage_replay_evidence(
        path.as_path(),
        PsionicTrainGroupedReplicaStageReplayEvidence {
            schema_version: String::from(
                crate::PSIONIC_TRAIN_GROUPED_STAGE_REPLAY_EVIDENCE_SCHEMA_VERSION,
            ),
            lane_id: manifest.lane_id.clone(),
            network_id: manifest.coordination.network_id.clone(),
            validator_run_id: String::from(validator_run_id),
            challenged_run_id: contribution_receipt.run_id.clone(),
            window_id: contribution_receipt.window_id.clone(),
            assignment_id: contribution_receipt.assignment_id.clone(),
            challenge_id: String::from(challenge_id),
            validator_node_pubkey: String::from(validator_node_pubkey),
            challenged_node_pubkey: contribution_receipt.node_pubkey.clone(),
            contribution_id: contribution_receipt.contribution_id.clone(),
            contribution_digest: contribution_receipt.contribution_digest.clone(),
            artifact_manifest_digest: contribution_artifact_manifest
                .artifact_manifest_digest
                .clone(),
            grouped_stage_assignment: execution_summary.grouped_stage_assignment.clone(),
            execution_summary_path: require_artifact(
                contribution_artifact_manifest,
                "grouped_stage_execution_summary",
            )
            .and_then(materialized_artifact_path)?,
            execution_summary_digest: execution_summary.execution_summary_digest.clone(),
            input_transport_digest: execution_summary.input_transport_digest.clone(),
            output_transport_digest: execution_summary.output_transport_digest.clone(),
            disposition,
            reason_codes: reason_codes
                .iter()
                .map(|reason_code| validator_reason_code_label(*reason_code).to_string())
                .collect(),
            score_bps,
            detail: detail.to_string(),
            replay_evidence_digest: String::new(),
        },
    )
    .map_err(map_grouped_stage_evidence_error)
}

fn require_artifact<'a>(
    contribution_artifact_manifest: &'a PsionicTrainContributionArtifactManifest,
    artifact_kind: &str,
) -> Result<&'a PsionicTrainContributionArtifact, PsionicTrainValidatorReplayError> {
    contribution_artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == artifact_kind)
        .ok_or_else(|| PsionicTrainValidatorReplayError::ArtifactIncomplete {
            detail: format!(
                "challenged contribution artifact manifest is missing required artifact kind `{artifact_kind}`"
            ),
        })
}

fn materialized_artifact_path(
    artifact: &PsionicTrainContributionArtifact,
) -> Result<String, PsionicTrainValidatorReplayError> {
    artifact
        .binding
        .require_materialized_path(
            format!(
                "contribution_artifact_manifest.artifacts[{}]",
                artifact.artifact_kind
            )
            .as_str(),
        )
        .map(String::from)
        .map_err(|detail| PsionicTrainValidatorReplayError::ArtifactIncomplete { detail })
}

fn materialize_contribution_artifact_manifest(
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
    run_root: &Path,
) -> Result<PsionicTrainContributionArtifactManifest, PsionicTrainValidatorReplayError> {
    let mut localized = contribution_artifact_manifest.clone();
    for artifact in &mut localized.artifacts {
        let field = format!(
            "contribution_artifact_manifest.artifacts[{}]",
            artifact.artifact_kind
        );
        let materialized_path =
            materialize_validator_artifact_binding(&artifact.binding, run_root, field.as_str())?;
        artifact.binding.materialized_path = Some(materialized_path.display().to_string());
    }
    Ok(localized)
}

fn materialize_validator_artifact_binding(
    binding: &crate::PsionicTrainArtifactBinding,
    run_root: &Path,
    field: &str,
) -> Result<PathBuf, PsionicTrainValidatorReplayError> {
    let mut attempted_paths = Vec::new();
    let desired_path = binding.materialized_path.as_deref().map(|value| {
        let candidate = PathBuf::from(value);
        if candidate.is_absolute() {
            candidate
        } else {
            run_root.join(candidate)
        }
    });
    let mut last_validation_error = None;

    if let Some(desired_path) = desired_path.as_ref() {
        attempted_paths.push(desired_path.display().to_string());
        if desired_path.is_file() {
            match validate_artifact_binding_candidate(binding, desired_path.as_path(), field) {
                Ok(()) => return Ok(desired_path.clone()),
                Err(error) => last_validation_error = Some(error),
            }
        }
        if let Some(rebased_path) = rebase_validator_artifact_path(desired_path.as_path(), run_root)
        {
            let rebased_display = rebased_path.display().to_string();
            if !attempted_paths.contains(&rebased_display) {
                attempted_paths.push(rebased_display);
            }
            if rebased_path.is_file() {
                match validate_artifact_binding_candidate(binding, rebased_path.as_path(), field) {
                    Ok(()) => return Ok(rebased_path),
                    Err(error) => last_validation_error = Some(error),
                }
            }
        }
    }

    let mut valid_cache_candidate = None;
    for candidate in psionic_train_resolved_artifact_cache_candidates(
        run_root,
        binding.artifact_ref.artifact_id.as_str(),
    ) {
        let candidate_display = candidate.display().to_string();
        if !attempted_paths.contains(&candidate_display) {
            attempted_paths.push(candidate_display);
        }
        if !candidate.is_file() {
            continue;
        }
        match validate_artifact_binding_candidate(binding, candidate.as_path(), field) {
            Ok(()) => {
                valid_cache_candidate = Some(candidate);
                break;
            }
            Err(error) => last_validation_error = Some(error),
        }
    }

    for candidate in validator_local_checkpoint_artifact_candidates(run_root, binding, field) {
        let candidate_display = candidate.display().to_string();
        if !attempted_paths.contains(&candidate_display) {
            attempted_paths.push(candidate_display);
        }
        if !candidate.is_file() {
            continue;
        }
        match validate_artifact_binding_candidate(binding, candidate.as_path(), field) {
            Ok(()) => {
                valid_cache_candidate = Some(candidate);
                break;
            }
            Err(error) => last_validation_error = Some(error),
        }
    }

    if let Some(desired_path) = desired_path {
        if let Some(cache_candidate) = valid_cache_candidate {
            if cache_candidate == desired_path {
                return Ok(desired_path);
            }
            if desired_path.starts_with(run_root) {
                copy_validator_artifact(cache_candidate.as_path(), desired_path.as_path())?;
                validate_artifact_binding_candidate(binding, desired_path.as_path(), field)?;
                return Ok(desired_path);
            }
            return Ok(cache_candidate);
        }
    } else if let Some(cache_candidate) = valid_cache_candidate {
        return Ok(cache_candidate);
    }

    if let Some(error) = last_validation_error {
        return Err(error);
    }
    Err(missing_validator_artifact_error(
        binding,
        run_root,
        field,
        attempted_paths,
    ))
}

fn rebase_validator_artifact_path(desired_path: &Path, run_root: &Path) -> Option<PathBuf> {
    let desired = desired_path.to_str()?;
    let run_id = run_root.file_name()?.to_str()?;
    let marker = format!("/training/runs/{run_id}/");
    let (_, suffix) = desired.split_once(marker.as_str())?;
    Some(run_root.join(suffix))
}

fn validator_local_checkpoint_artifact_candidates(
    run_root: &Path,
    binding: &crate::PsionicTrainArtifactBinding,
    field: &str,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let checkpoint_surface_path = run_root.join("status/checkpoint_surface.json");
    if let Ok(surface) = load_validator_checkpoint_surface(checkpoint_surface_path.as_path()) {
        let artifact_id = binding.artifact_ref.artifact_id.as_str();
        if field.contains("checkpoint_pointer") || artifact_id.contains("checkpoint_pointer") {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_pointer_path.as_deref(),
            );
            push_validator_candidate(
                &mut candidates,
                run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
            );
        }
        if field.contains("checkpoint_manifest") || artifact_id.contains("checkpoint_manifest") {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_manifest_path.as_deref(),
            );
            push_validator_candidate(
                &mut candidates,
                run_root.join("checkpoints/step-0/checkpoint_manifest.json"),
            );
            let checkpoints_manifest_dir = run_root.join("checkpoints/manifests");
            if let Ok(entries) = fs::read_dir(checkpoints_manifest_dir) {
                for entry in entries.flatten() {
                    push_validator_candidate(&mut candidates, entry.path());
                }
            }
        }
        if field.contains("checkpoint_backup_receipt")
            || artifact_id.contains("checkpoint_backup_receipt")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_backup_receipt_path.as_deref(),
            );
        }
        if field.contains("checkpoint_backup_pointer")
            || artifact_id.contains("checkpoint_backup_pointer")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_backup_pointer_path.as_deref(),
            );
        }
        if field.contains("checkpoint_backup_manifest")
            || artifact_id.contains("checkpoint_backup_manifest")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_backup_manifest_path.as_deref(),
            );
        }
        if field.contains("recovery_receipt")
            || artifact_id.contains("auto_resume")
            || artifact_id.contains("grouped_stage_recovery")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.auto_resume_receipt_path.as_deref(),
            );
            push_optional_validator_candidate(
                &mut candidates,
                surface
                    .artifacts
                    .grouped_stage_recovery_receipt_path
                    .as_deref(),
            );
        }
        if field.contains("checkpoint_handoff_receipt")
            || artifact_id.contains("checkpoint_handoff_receipt")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface
                    .artifacts
                    .peer_checkpoint_handoff_receipt_path
                    .as_deref(),
            );
        }
        if field.contains("checkpoint_failure_drill")
            || artifact_id.contains("checkpoint_failure_drill")
        {
            push_optional_validator_candidate(
                &mut candidates,
                surface.artifacts.checkpoint_failure_drill_path.as_deref(),
            );
        }
    }
    candidates
}

fn load_validator_checkpoint_surface(
    path: &Path,
) -> Result<PsionicTrainCheckpointSurface, PsionicTrainValidatorReplayError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainValidatorReplayError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(bytes.as_slice()).map_err(|error| {
        PsionicTrainValidatorReplayError::ArtifactIncomplete {
            detail: format!(
                "validator local checkpoint surface `{}` failed to parse: {error}",
                path.display()
            ),
        }
    })
}

fn push_optional_validator_candidate(candidates: &mut Vec<PathBuf>, candidate: Option<&str>) {
    if let Some(candidate) = candidate {
        push_validator_candidate(candidates, PathBuf::from(candidate));
    }
}

fn push_validator_candidate(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn validate_artifact_binding_candidate(
    binding: &crate::PsionicTrainArtifactBinding,
    candidate: &Path,
    field: &str,
) -> Result<(), PsionicTrainValidatorReplayError> {
    let bytes = fs::read(candidate).map_err(|error| PsionicTrainValidatorReplayError::Read {
        path: candidate.display().to_string(),
        detail: error.to_string(),
    })?;
    if let Some(expected_bytes) = binding.artifact_ref.artifact_bytes {
        let actual_bytes = u64::try_from(bytes.len()).map_err(|error| {
            PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: format!(
                    "{field} artifact `{}` could not measure candidate `{}`: {error}",
                    binding.artifact_ref.artifact_id,
                    candidate.display(),
                ),
            }
        })?;
        if actual_bytes != expected_bytes {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: format!(
                    "{field} artifact `{}` expected {expected_bytes} bytes but `{}` had {actual_bytes}",
                    binding.artifact_ref.artifact_id,
                    candidate.display(),
                ),
            });
        }
    }
    if let Some(expected_digest) = binding.artifact_ref.artifact_digest.as_deref() {
        let actual_digest = sha256_hex(bytes.as_slice());
        if actual_digest != expected_digest {
            return Err(PsionicTrainValidatorReplayError::ArtifactDigestMismatch {
                detail: format!(
                    "{field} artifact `{}` expected digest `{expected_digest}` but `{}` had `{actual_digest}`",
                    binding.artifact_ref.artifact_id,
                    candidate.display(),
                ),
            });
        }
    }
    Ok(())
}

fn copy_validator_artifact(
    source_path: &Path,
    target_path: &Path,
) -> Result<(), PsionicTrainValidatorReplayError> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionicTrainValidatorReplayError::Write {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    fs::copy(source_path, target_path).map_err(|error| {
        PsionicTrainValidatorReplayError::Write {
            path: target_path.display().to_string(),
            detail: format!(
                "failed to copy `{}` into the validator replay materialization target: {error}",
                source_path.display()
            ),
        }
    })?;
    Ok(())
}

fn missing_validator_artifact_error(
    binding: &crate::PsionicTrainArtifactBinding,
    run_root: &Path,
    field: &str,
    attempted_paths: Vec<String>,
) -> PsionicTrainValidatorReplayError {
    let detail = format!(
        "{field} requires one local copy of artifact `{}`; automatic replay expects resolver-backed materialization under `{}`; checked {}",
        binding.artifact_ref.artifact_id,
        run_root
            .join(PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR)
            .display(),
        attempted_paths.join(", "),
    );
    if field.contains("checkpoint") || binding.artifact_ref.artifact_id.contains("checkpoint") {
        PsionicTrainValidatorReplayError::CheckpointMissing { detail }
    } else {
        PsionicTrainValidatorReplayError::ArtifactIncomplete { detail }
    }
}

fn validator_hooks_for_target_work_class(
    work_class: PsionicTrainWorkClass,
) -> Vec<PsionicTrainValidatorHook> {
    match work_class {
        PsionicTrainWorkClass::GroupedReplicaStageExecution => vec![
            PsionicTrainValidatorHook::AssignmentCorrectness,
            PsionicTrainValidatorHook::CheckpointLineage,
            PsionicTrainValidatorHook::WorkExecutionPlausibility,
            PsionicTrainValidatorHook::UpdateIntegrity,
            PsionicTrainValidatorHook::GroupedStageIntegrity,
        ],
        PsionicTrainWorkClass::AdapterTraining
        | PsionicTrainWorkClass::SmallModelLocalTraining
        | PsionicTrainWorkClass::FullIslandLocalUpdateTraining => vec![
            PsionicTrainValidatorHook::AssignmentCorrectness,
            PsionicTrainValidatorHook::CheckpointLineage,
            PsionicTrainValidatorHook::WorkExecutionPlausibility,
            PsionicTrainValidatorHook::UpdateIntegrity,
        ],
        PsionicTrainWorkClass::ValidationReplay
        | PsionicTrainWorkClass::Evaluation
        | PsionicTrainWorkClass::Aggregation
        | PsionicTrainWorkClass::CheckpointPromotion => Vec::new(),
    }
}

fn map_grouped_stage_evidence_error(
    error: PsionicTrainGroupedReplicaEvidenceError,
) -> PsionicTrainValidatorReplayError {
    match error {
        PsionicTrainGroupedReplicaEvidenceError::Read { path, detail } => {
            PsionicTrainValidatorReplayError::Read { path, detail }
        }
        PsionicTrainGroupedReplicaEvidenceError::Write { path, detail } => {
            PsionicTrainValidatorReplayError::Write { path, detail }
        }
        PsionicTrainGroupedReplicaEvidenceError::Parse { path, detail } => {
            PsionicTrainValidatorReplayError::Parse { path, detail }
        }
        PsionicTrainGroupedReplicaEvidenceError::Invalid { detail }
        | PsionicTrainGroupedReplicaEvidenceError::ArtifactDigestMismatch { detail } => {
            PsionicTrainValidatorReplayError::ArtifactDigestMismatch { detail }
        }
        PsionicTrainGroupedReplicaEvidenceError::StaleAssignment { detail } => {
            PsionicTrainValidatorReplayError::StaleAssignment { detail }
        }
    }
}

fn map_grouped_stage_transport_error(
    error: crate::PsionicTrainGroupedReplicaTransportError,
) -> PsionicTrainValidatorReplayError {
    match error {
        crate::PsionicTrainGroupedReplicaTransportError::Read { path, detail } => {
            PsionicTrainValidatorReplayError::Read { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Write { path, detail } => {
            PsionicTrainValidatorReplayError::Write { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Parse { path, detail } => {
            PsionicTrainValidatorReplayError::Parse { path, detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::Invalid { detail }
        | crate::PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch { detail } => {
            PsionicTrainValidatorReplayError::ArtifactDigestMismatch { detail }
        }
        crate::PsionicTrainGroupedReplicaTransportError::StaleAssignment { detail } => {
            PsionicTrainValidatorReplayError::StaleAssignment { detail }
        }
    }
}

fn validator_reason_code_label(reason_code: PsionicTrainValidatorReplayReasonCode) -> &'static str {
    match reason_code {
        PsionicTrainValidatorReplayReasonCode::ContributionOutcomeRefused => {
            "contribution_outcome_refused"
        }
        PsionicTrainValidatorReplayReasonCode::PrimaryCheckpointAccepted => {
            "primary_checkpoint_accepted"
        }
        PsionicTrainValidatorReplayReasonCode::CheckpointRecovered => "checkpoint_recovered",
        PsionicTrainValidatorReplayReasonCode::CheckpointReplayRequired => {
            "checkpoint_replay_required"
        }
        PsionicTrainValidatorReplayReasonCode::GroupedStageEvidenceVerified => {
            "grouped_stage_evidence_verified"
        }
    }
}

fn classify_validator_result(
    contribution_receipt: &PsionicTrainContributionReceipt,
    checkpoint_surface: Option<&PsionicTrainCheckpointSurface>,
    grouped_stage_evidence_verified: bool,
) -> Result<
    (
        TrainingExecutionValidatorDisposition,
        Vec<PsionicTrainValidatorReplayReasonCode>,
        u16,
        String,
        Option<String>,
    ),
    PsionicTrainValidatorReplayError,
> {
    if contribution_receipt.outcome == PsionicTrainOutcomeKind::Refused {
        return Ok((
            TrainingExecutionValidatorDisposition::Rejected,
            vec![PsionicTrainValidatorReplayReasonCode::ContributionOutcomeRefused],
            0,
            String::from(
                "validator replay rejected the challenged contribution because the worker contribution itself ended in refused posture",
            ),
            None,
        ));
    }

    let checkpoint_surface = checkpoint_surface.ok_or_else(|| {
        PsionicTrainValidatorReplayError::CheckpointMissing {
            detail: String::from(
                "successful challenged contribution did not retain a checkpoint surface artifact for replay",
            ),
        }
    })?;
    let checkpoint_pointer_state = checkpoint_surface.pointer_state.clone();
    let clean_recovery_posture = matches!(
        checkpoint_surface.recovery_source_kind.as_deref(),
        None | Some("none")
    );
    let accepted_pointer_state = matches!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted") | Some("accepted_primary")
    );

    if checkpoint_surface.upload_outcome.as_deref() == Some("refused")
        || checkpoint_surface.recovery_resolution_state.as_deref() == Some("refused")
    {
        let mut reason_codes =
            vec![PsionicTrainValidatorReplayReasonCode::CheckpointReplayRequired];
        if grouped_stage_evidence_verified {
            reason_codes.push(PsionicTrainValidatorReplayReasonCode::GroupedStageEvidenceVerified);
        }
        return Ok((
            TrainingExecutionValidatorDisposition::ReplayRequired,
            reason_codes,
            5_000,
            String::from(
                "validator replay kept the challenged contribution in replay-required posture because the retained checkpoint surface still records a refused upload or refused recovery path",
            ),
            checkpoint_pointer_state,
        ));
    }

    if checkpoint_surface.restored_primary_pointer == Some(true)
        || !clean_recovery_posture
        || !accepted_pointer_state
    {
        let mut reason_codes = vec![PsionicTrainValidatorReplayReasonCode::CheckpointRecovered];
        if grouped_stage_evidence_verified {
            reason_codes.push(PsionicTrainValidatorReplayReasonCode::GroupedStageEvidenceVerified);
        }
        return Ok((
            TrainingExecutionValidatorDisposition::Quarantined,
            reason_codes,
            7_500,
            String::from(
                "validator replay quarantined the challenged contribution because it depends on recovered or non-primary checkpoint posture rather than a clean accepted-primary pointer",
            ),
            checkpoint_pointer_state,
        ));
    }

    let mut reason_codes = vec![PsionicTrainValidatorReplayReasonCode::PrimaryCheckpointAccepted];
    if grouped_stage_evidence_verified {
        reason_codes.push(PsionicTrainValidatorReplayReasonCode::GroupedStageEvidenceVerified);
    }
    Ok((
        TrainingExecutionValidatorDisposition::Accepted,
        reason_codes,
        10_000,
        String::from(
            "validator replay accepted the challenged contribution because the retained artifact manifest, contribution receipt, and accepted-primary checkpoint surface all match the declared replay target",
        ),
        checkpoint_pointer_state,
    ))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, PsionicTrainValidatorReplayError> {
    let bytes = fs::read(path).map_err(|error| PsionicTrainValidatorReplayError::Read {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|error| PsionicTrainValidatorReplayError::Parse {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn write_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionicTrainValidatorReplayError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PsionicTrainValidatorReplayError::Write {
            path: parent.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionicTrainValidatorReplayError::Write {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })?;
    fs::write(path, bytes).map_err(|error| PsionicTrainValidatorReplayError::Write {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("validator replay score surfaces must serialize for stable digest"),
    );
    format!("{:x}", hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn validator_challenge_dir_name(challenge_id: &str) -> String {
    let digest = sha256_hex(challenge_id.as_bytes());
    format!("challenge-{}", &digest[..32])
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        build_validator_quality_drift_signal, build_validator_rollback_signal,
        classify_validator_result, execute_psionic_train_validator_replay,
        materialize_validator_artifact_binding, stable_digest, validator_challenge_dir_name,
        PsionicTrainValidatorHook, PsionicTrainValidatorQualityDriftState,
        PsionicTrainValidatorReplayReasonCode, PsionicTrainValidatorRollbackPosture,
        PsionicTrainValidatorScoreReceipt, TrainingExecutionValidatorDisposition,
        PSIONIC_TRAIN_VALIDATOR_SCORE_RECEIPT_SCHEMA_VERSION,
    };
    use crate::{
        build_psionic_train_artifact_binding_from_path,
        load_psionic_train_grouped_stage_replay_evidence, persist_psionic_train_window_artifacts,
        PsionicTrainAdmissionIdentity, PsionicTrainArtifactBinding, PsionicTrainAuthorityOwner,
        PsionicTrainCapabilityProjection, PsionicTrainCheckpointArtifactPaths,
        PsionicTrainCheckpointSurface, PsionicTrainContributionArtifact,
        PsionicTrainContributionArtifactManifest, PsionicTrainContributionReceipt,
        PsionicTrainCoordinationContext, PsionicTrainGroupedReplicaStageAssignment,
        PsionicTrainGroupedReplicaStageRole, PsionicTrainInvocationManifest, PsionicTrainOperation,
        PsionicTrainOutcomeKind, PsionicTrainRole, PsionicTrainRuntimeAttestation,
        PsionicTrainWeakDevicePublicCountClass, PsionicTrainWeakDeviceValidationReplayProof,
        PsionicTrainWindowArtifactInputRefs, PsionicTrainWorkClass,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
        PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS,
        PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
        PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID,
        PSIONIC_TRAIN_CHECKPOINT_SURFACE_SCHEMA_VERSION,
        PSIONIC_TRAIN_CONTRIBUTION_ARTIFACT_MANIFEST_SCHEMA_VERSION,
        PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION,
        PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
    };

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = env::temp_dir().join(format!("psionic-validator-{label}-{unique}"));
        if path.exists() {
            fs::remove_dir_all(&path).expect("temp dir should clear");
        }
        fs::create_dir_all(&path).expect("temp dir should create");
        path
    }

    fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
        fs::create_dir_all(path.parent().expect("parent should exist"))
            .expect("parent dir should create");
        fs::write(
            path,
            serde_json::to_vec_pretty(value).expect("json should serialize"),
        )
        .expect("json should write");
    }

    fn artifact_binding(path: &str) -> PsionicTrainArtifactBinding {
        let source = Path::new(path);
        let file_name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("validator_test_artifact.json");
        let artifact_role = match file_name {
            "artifact_manifest.json" => "contribution_artifact_manifest",
            "contribution_receipt.json" => "contribution_receipt",
            "peer_checkpoint_handoff_receipt.json" => "checkpoint_handoff_receipt",
            _ => "validator_test_artifact",
        };
        let resolved = if source.exists() {
            source.to_path_buf()
        } else {
            // Fabricate the artifact in a per-test temp file instead of
            // depending on stray absolute files such as
            // `/tmp/artifact_manifest.json` existing on the machine.
            static FABRICATED_ARTIFACT_COUNTER: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let unique = FABRICATED_ARTIFACT_COUNTER
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let root = env::temp_dir().join(format!(
                "psionic-validator-fabricated-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("fabricated artifact dir should create");
            let fabricated = root.join(file_name);
            fs::write(
                &fabricated,
                format!("psionic validator test artifact placeholder for `{path}`\n"),
            )
            .expect("fabricated artifact should write");
            fabricated
        };
        build_psionic_train_artifact_binding_from_path(artifact_role, resolved.as_path())
            .expect("artifact binding should build from path")
    }

    fn runtime_attestation() -> PsionicTrainRuntimeAttestation {
        PsionicTrainRuntimeAttestation::new(
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            "sha256:test-build",
            "1111222233334444555566667777888899990000",
            "clean",
            None,
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        )
    }

    fn capability_projection() -> PsionicTrainCapabilityProjection {
        PsionicTrainCapabilityProjection {
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            backend_family: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY),
            topology_class: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS),
            environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
        }
    }

    fn grouped_worker_manifest(run_root: &Path) -> PsionicTrainInvocationManifest {
        PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            work_class: PsionicTrainWorkClass::GroupedReplicaStageExecution,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psionic.validator-test")),
                window_id: Some(String::from("window-0001")),
                assignment_id: Some(String::from("assignment-0001")),
                challenge_id: None,
                node_pubkey: Some(String::from("npub1-grouped-worker")),
                membership_revision: Some(7),
            },
            grouped_stage_assignment: Some(
                PsionicTrainGroupedReplicaStageAssignment::new(
                    "replica-01",
                    "stage-01",
                    0,
                    2,
                    PsionicTrainGroupedReplicaStageRole::Ingress,
                    None,
                    Some(String::from("stage-02")),
                )
                .expect("grouped stage assignment should build"),
            ),
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: String::from("sha256:test-build"),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(String::from("worker-run")),
            output_root: Some(run_root.display().to_string()),
            run_root: None,
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            validator_target_contribution_receipt: None,
            validator_target_contribution_artifact_manifest: None,
            validator_target_work_class: None,
            grouped_stage_input_transport: None,
            selected_git_ref: Some(String::from("HEAD")),
            hardware_observation_path: None,
            run_shape_observation_path: None,
            allow_dirty_tree: false,
            dry_run: true,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
            manifest_digest: None,
        }
    }

    fn validator_manifest(
        run_root: &Path,
        contribution_receipt_path: String,
        artifact_manifest_path: String,
    ) -> PsionicTrainInvocationManifest {
        PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Validator,
            operation: PsionicTrainOperation::ValidateContribution,
            work_class: PsionicTrainWorkClass::ValidationReplay,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psionic.validator-test")),
                window_id: Some(String::from("window-0001")),
                assignment_id: Some(String::from("assignment-0001")),
                challenge_id: Some(String::from("challenge-0001")),
                node_pubkey: Some(String::from("npub1-validator")),
                membership_revision: Some(9),
            },
            grouped_stage_assignment: None,
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: String::from("sha256:test-build"),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(String::from("validator-run")),
            output_root: None,
            run_root: Some(run_root.display().to_string()),
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            validator_target_contribution_receipt: Some(artifact_binding(
                contribution_receipt_path.as_str(),
            )),
            validator_target_contribution_artifact_manifest: Some(artifact_binding(
                artifact_manifest_path.as_str(),
            )),
            validator_target_work_class: Some(PsionicTrainWorkClass::GroupedReplicaStageExecution),
            grouped_stage_input_transport: None,
            selected_git_ref: Some(String::from("HEAD")),
            hardware_observation_path: None,
            run_shape_observation_path: None,
            allow_dirty_tree: false,
            dry_run: true,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
            manifest_digest: None,
        }
    }

    fn apple_validator_manifest(
        run_root: &Path,
        contribution_receipt_path: String,
        artifact_manifest_path: String,
        challenged_work_class: PsionicTrainWorkClass,
    ) -> PsionicTrainInvocationManifest {
        let mut manifest =
            validator_manifest(run_root, contribution_receipt_path, artifact_manifest_path);
        manifest.lane_id = String::from(PSION_APPLE_WINDOWED_TRAINING_LANE_ID);
        manifest.run_id = Some(String::from("apple-validator-run"));
        manifest.coordination.node_pubkey = Some(String::from("npub1-apple-validator"));
        manifest.admission_identity = PsionicTrainAdmissionIdentity {
            release_id: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID),
            build_digest: String::from("sha256:apple-test-build"),
            environment_ref: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF),
        };
        manifest.validator_target_work_class = Some(challenged_work_class);
        manifest
    }

    fn base_validator_manifest(
        window_id: &str,
        assignment_id: &str,
        challenge_id: &str,
    ) -> PsionicTrainInvocationManifest {
        let mut manifest = validator_manifest(
            Path::new("/tmp/validator-run"),
            String::from("/tmp/contribution_receipt.json"),
            String::from("/tmp/artifact_manifest.json"),
        );
        manifest.coordination.window_id = Some(String::from(window_id));
        manifest.coordination.assignment_id = Some(String::from(assignment_id));
        manifest.coordination.challenge_id = Some(String::from(challenge_id));
        manifest
    }

    fn succeeded_contribution() -> PsionicTrainContributionReceipt {
        let mut receipt = PsionicTrainContributionReceipt {
            schema_version: String::from(PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            run_id: String::from("worker-run"),
            window_id: String::from("window-0001"),
            window_execution_id: String::from("window-execution-1"),
            assignment_id: String::from("assignment-0001"),
            contribution_id: String::from("contribution-0001"),
            node_pubkey: String::from("npub1-worker"),
            grouped_stage_assignment: None,
            role: PsionicTrainRole::Worker,
            operation: String::from("record-checkpoint"),
            outcome: PsionicTrainOutcomeKind::Succeeded,
            exit_code: 0,
            retryable: false,
            authority_owner: PsionicTrainAuthorityOwner::Pylon,
            refusal_class: None,
            artifact_manifest: artifact_binding("/tmp/artifact_manifest.json"),
            artifact_manifest_digest: String::from("artifact-digest-1"),
            artifact_count: 4,
            contribution_digest: String::new(),
            detail: String::from("successful contribution"),
        };
        receipt.contribution_digest = receipt.stable_contribution_digest();
        receipt
    }

    fn refused_contribution() -> PsionicTrainContributionReceipt {
        let mut receipt = succeeded_contribution();
        receipt.outcome = PsionicTrainOutcomeKind::Refused;
        receipt.detail = String::from("refused contribution");
        receipt.contribution_digest = receipt.stable_contribution_digest();
        receipt
    }

    fn checkpoint_surface(
        pointer_state: Option<&str>,
        upload_outcome: Option<&str>,
        recovery_source_kind: Option<&str>,
        restored_primary_pointer: Option<bool>,
        recovery_resolution_state: Option<&str>,
    ) -> PsionicTrainCheckpointSurface {
        PsionicTrainCheckpointSurface {
            schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_SURFACE_SCHEMA_VERSION),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            operation: crate::PsionicTrainOperation::RecordCheckpoint,
            run_id: String::from("worker-run"),
            window_id: None,
            assignment_id: None,
            grouped_stage_assignment: None,
            run_root: String::from("/tmp/worker-run"),
            current_phase: Some(String::from("checkpoint_evaluated")),
            pointer_state: pointer_state.map(String::from),
            checkpoint_label: Some(String::from("accepted")),
            optimizer_step: Some(4_096),
            checkpoint_ref: Some(String::from("checkpoint://psion/test")),
            checkpoint_manifest_digest: Some(String::from("manifest-digest")),
            checkpoint_object_digest: Some(String::from("object-digest")),
            checkpoint_total_bytes: Some(2_048),
            backup_state: Some(String::from("backed_up")),
            upload_outcome: upload_outcome.map(String::from),
            upload_failure_reason: None,
            recovery_resolution_state: recovery_resolution_state.map(String::from),
            recovery_source_kind: recovery_source_kind.map(String::from),
            restored_primary_pointer,
            artifacts: PsionicTrainCheckpointArtifactPaths::default(),
        }
    }

    fn score_receipt(
        window_id: &str,
        assignment_id: &str,
        challenge_id: &str,
        validation_index: u64,
        score_bps: u16,
        disposition: TrainingExecutionValidatorDisposition,
    ) -> PsionicTrainValidatorScoreReceipt {
        let mut receipt = PsionicTrainValidatorScoreReceipt {
            schema_version: String::from(PSIONIC_TRAIN_VALIDATOR_SCORE_RECEIPT_SCHEMA_VERSION),
            lane_id: String::from(crate::PSION_ACTUAL_PRETRAINING_LANE_ID),
            validator_work_class: PsionicTrainWorkClass::ValidationReplay,
            challenged_work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            network_id: Some(String::from("network.psionic.test")),
            validator_run_id: String::from("validator-run"),
            challenged_run_id: String::from("worker-run"),
            window_id: String::from(window_id),
            assignment_id: String::from(assignment_id),
            challenge_id: String::from(challenge_id),
            validator_node_pubkey: String::from("npub1-validator"),
            challenged_node_pubkey: String::from("npub1-worker"),
            contribution_id: format!("contribution-{window_id}"),
            contribution_digest: format!("digest-{window_id}"),
            artifact_manifest_digest: format!("manifest-{window_id}"),
            grouped_stage_execution_summary_path: None,
            grouped_stage_execution_summary_digest: None,
            grouped_stage_replay_evidence_path: None,
            grouped_stage_replay_evidence_digest: None,
            validation_index,
            verified_hooks: vec![PsionicTrainValidatorHook::CheckpointLineage],
            disposition,
            reason_codes: vec![PsionicTrainValidatorReplayReasonCode::PrimaryCheckpointAccepted],
            score_bps,
            score_artifact_path: format!("/tmp/{window_id}/validator_score_artifact.json"),
            score_artifact_digest: format!("score-artifact-{window_id}"),
            detail: String::from("validator test receipt"),
            score_receipt_digest: format!("score-receipt-{window_id}"),
        };
        receipt.score_receipt_digest =
            stable_digest(b"psionic_train_validator_score_receipt|", &receipt);
        receipt
    }

    #[test]
    fn validator_challenge_dir_name_bounds_long_sample_challenge_ids() {
        let challenge_id = format!(
            "challenge.training.{}.window.{}.sample.{}.a1",
            "run.cs336.a1.audit_twentyk_refresh_20260508161709_20260508161710_f23e539b_0001.20260508161710.e3b22028",
            "window.cs336.a1.audit_twentyk_refresh_20260508161709_20260508161710_f23e539b_0001.20260508161710.e3b22028.0001",
            "assign-cdea3f71864649df8f546340"
        );

        let dir_name = validator_challenge_dir_name(challenge_id.as_str());

        assert!(dir_name.starts_with("challenge-"));
        assert_eq!(dir_name.len(), "challenge-".len() + 32);
        assert!(dir_name.len() < 255);
        assert_eq!(
            dir_name,
            validator_challenge_dir_name(challenge_id.as_str())
        );
    }

    #[test]
    fn clean_accepted_checkpoint_is_accepted() {
        let receipt = succeeded_contribution();
        let surface =
            checkpoint_surface(Some("accepted"), Some("succeeded"), None, Some(false), None);
        let (disposition, reason_codes, score_bps, _, _) =
            classify_validator_result(&receipt, Some(&surface), false)
                .expect("classification should work");
        assert_eq!(disposition, TrainingExecutionValidatorDisposition::Accepted);
        assert_eq!(
            reason_codes,
            vec![PsionicTrainValidatorReplayReasonCode::PrimaryCheckpointAccepted]
        );
        assert_eq!(score_bps, 10_000);
    }

    #[test]
    fn refused_upload_requires_replay() {
        let receipt = succeeded_contribution();
        let surface =
            checkpoint_surface(Some("accepted"), Some("refused"), None, Some(false), None);
        let (disposition, reason_codes, score_bps, _, _) =
            classify_validator_result(&receipt, Some(&surface), false)
                .expect("classification should work");
        assert_eq!(
            disposition,
            TrainingExecutionValidatorDisposition::ReplayRequired
        );
        assert_eq!(
            reason_codes,
            vec![PsionicTrainValidatorReplayReasonCode::CheckpointReplayRequired]
        );
        assert_eq!(score_bps, 5_000);
    }

    #[test]
    fn recovered_checkpoint_is_quarantined() {
        let receipt = succeeded_contribution();
        let surface = checkpoint_surface(
            Some("accepted"),
            Some("succeeded"),
            Some("backup_receipt"),
            Some(true),
            Some("restored"),
        );
        let (disposition, reason_codes, score_bps, _, _) =
            classify_validator_result(&receipt, Some(&surface), false)
                .expect("classification should work");
        assert_eq!(
            disposition,
            TrainingExecutionValidatorDisposition::Quarantined
        );
        assert_eq!(
            reason_codes,
            vec![PsionicTrainValidatorReplayReasonCode::CheckpointRecovered]
        );
        assert_eq!(score_bps, 7_500);
    }

    #[test]
    fn refused_worker_contribution_is_rejected_without_checkpoint() {
        let receipt = refused_contribution();
        let (disposition, reason_codes, score_bps, _, _) =
            classify_validator_result(&receipt, None, false).expect("classification should work");
        assert_eq!(disposition, TrainingExecutionValidatorDisposition::Rejected);
        assert_eq!(
            reason_codes,
            vec![PsionicTrainValidatorReplayReasonCode::ContributionOutcomeRefused]
        );
        assert_eq!(score_bps, 0);
    }

    #[test]
    fn quality_drift_signal_marks_multi_window_regression() {
        let manifest = base_validator_manifest("window-0002", "assignment-0002", "challenge-0002");
        let previous_receipt = score_receipt(
            "window-0001",
            "assignment-0001",
            "challenge-0001",
            1,
            10_000,
            TrainingExecutionValidatorDisposition::Accepted,
        );
        let current_receipt = score_receipt(
            "window-0002",
            "assignment-0002",
            "challenge-0002",
            2,
            5_000,
            TrainingExecutionValidatorDisposition::ReplayRequired,
        );
        let signal = build_validator_quality_drift_signal(
            &manifest,
            &current_receipt,
            Path::new("/tmp/window-0002/validator_score_receipt.json"),
            &[previous_receipt],
        );
        assert_eq!(
            signal.drift_state,
            PsionicTrainValidatorQualityDriftState::Regressed
        );
        assert_eq!(signal.score_bps_delta, Some(-5_000));
        assert_eq!(signal.degraded_window_count, 1);
        assert_eq!(signal.non_accepted_window_count, 1);
        assert_eq!(signal.previous_window_id.as_deref(), Some("window-0001"));
    }

    #[test]
    fn rollback_signal_uses_last_accepted_baseline() {
        let manifest = base_validator_manifest("window-0003", "assignment-0003", "challenge-0003");
        let baseline_receipt = score_receipt(
            "window-0001",
            "assignment-0001",
            "challenge-0001",
            1,
            10_000,
            TrainingExecutionValidatorDisposition::Accepted,
        );
        let degraded_receipt = score_receipt(
            "window-0002",
            "assignment-0002",
            "challenge-0002",
            2,
            7_500,
            TrainingExecutionValidatorDisposition::Quarantined,
        );
        let current_receipt = score_receipt(
            "window-0003",
            "assignment-0003",
            "challenge-0003",
            3,
            5_000,
            TrainingExecutionValidatorDisposition::ReplayRequired,
        );
        let mut drift_signal = build_validator_quality_drift_signal(
            &manifest,
            &current_receipt,
            Path::new("/tmp/window-0003/validator_score_receipt.json"),
            &[baseline_receipt.clone(), degraded_receipt.clone()],
        );
        drift_signal.drift_signal_digest = stable_digest(
            b"psionic_train_validator_quality_drift_signal|",
            &drift_signal,
        );
        let rollback_signal = build_validator_rollback_signal(
            &manifest,
            &current_receipt,
            Path::new("/tmp/window-0003/validator_score_receipt.json"),
            Path::new("/tmp/window-0003/validator_quality_drift_signal.json"),
            &drift_signal,
            &[baseline_receipt, degraded_receipt],
        );
        assert_eq!(
            rollback_signal.rollback_posture,
            PsionicTrainValidatorRollbackPosture::Candidate
        );
        assert_eq!(
            rollback_signal.rollback_baseline_window_id.as_deref(),
            Some("window-0001")
        );
        assert_eq!(rollback_signal.consecutive_non_accepted_window_count, 2);
    }

    #[test]
    fn grouped_stage_validator_replay_emits_replay_evidence() {
        let run_root = temp_root("grouped-stage-replay");
        let mut worker_manifest = grouped_worker_manifest(&run_root);
        worker_manifest
            .populate_manifest_digest()
            .expect("worker manifest digest should populate");
        let invocation_manifest_path = run_root.join("manifest/worker_invocation_manifest.json");
        write_json(invocation_manifest_path.as_path(), &worker_manifest);

        let checkpoint_surface_path = run_root.join("status/checkpoint_surface.json");
        write_json(
            checkpoint_surface_path.as_path(),
            &checkpoint_surface(Some("accepted"), Some("succeeded"), None, Some(false), None),
        );

        let window_artifacts = persist_psionic_train_window_artifacts(
            &worker_manifest,
            &runtime_attestation(),
            &capability_projection(),
            "worker-run",
            &run_root,
            &PsionicTrainWindowArtifactInputRefs {
                invocation_manifest_path: invocation_manifest_path.display().to_string(),
                launch_manifest_path: None,
                membership_revision_path: None,
                grouped_stage_input_transport_path: None,
                checkpoint_surface_path: Some(checkpoint_surface_path.display().to_string()),
                checkpoint_pointer_path: None,
                checkpoint_manifest_path: None,
                checkpoint_backup_receipt_path: None,
                checkpoint_handoff_receipt_path: None,
                recovery_receipt_path: None,
                current_status_path: None,
                retained_summary_path: None,
                launcher_log_path: None,
                final_closeout_bundle_path: None,
            },
            PsionicTrainOutcomeKind::Succeeded,
            0,
            false,
            PsionicTrainAuthorityOwner::Pylon,
            None,
            "grouped stage worker contribution completed",
        )
        .expect("worker window artifacts should persist")
        .expect("worker should emit window artifacts");
        let validator_manifest = validator_manifest(
            &run_root,
            window_artifacts.contribution_receipt_path.clone(),
            window_artifacts.contribution_artifact_manifest_path.clone(),
        );
        let execution = execute_psionic_train_validator_replay(&validator_manifest, &run_root)
            .expect("validator replay should succeed");
        assert_eq!(
            execution.score_receipt.validator_work_class,
            PsionicTrainWorkClass::ValidationReplay
        );
        assert_eq!(
            execution.score_receipt.challenged_work_class,
            PsionicTrainWorkClass::GroupedReplicaStageExecution
        );
        assert!(execution
            .score_receipt
            .verified_hooks
            .contains(&PsionicTrainValidatorHook::GroupedStageIntegrity));
        let contribution_receipt: PsionicTrainContributionReceipt = serde_json::from_slice(
            &fs::read(&window_artifacts.contribution_receipt_path)
                .expect("contribution receipt should read"),
        )
        .expect("contribution receipt should parse");
        let replay_evidence_path = execution
            .artifacts
            .grouped_stage_replay_evidence_path
            .as_deref()
            .expect("grouped stage replay evidence should be emitted");
        let replay_evidence =
            load_psionic_train_grouped_stage_replay_evidence(Path::new(replay_evidence_path))
                .expect("grouped stage replay evidence should parse");
        assert_eq!(
            replay_evidence.contribution_id,
            contribution_receipt.contribution_id
        );
        assert_eq!(
            replay_evidence.grouped_stage_assignment.stage_id,
            "stage-01"
        );
        assert!(replay_evidence
            .reason_codes
            .iter()
            .any(|reason_code| reason_code == "grouped_stage_evidence_verified"));
        assert_eq!(
            execution
                .score_receipt
                .grouped_stage_execution_summary_path
                .as_deref(),
            window_artifacts
                .grouped_stage_execution_summary_path
                .as_deref()
        );
        assert_eq!(
            execution
                .score_receipt
                .grouped_stage_replay_evidence_path
                .as_deref(),
            Some(replay_evidence_path)
        );
        assert!(execution
            .artifacts
            .weak_device_validation_replay_proof_path
            .is_none());
    }

    #[test]
    fn validator_replay_falls_back_to_local_checkpoint_surface_artifacts() {
        let run_root = temp_root("validator-local-checkpoint-surface");
        let checkpoint_manifest_path =
            run_root.join("checkpoints/manifests/checkpoint_manifest_step-000004.json");
        fs::create_dir_all(
            checkpoint_manifest_path
                .parent()
                .expect("checkpoint manifest parent should exist"),
        )
        .expect("checkpoint manifest dir should create");
        fs::write(
            &checkpoint_manifest_path,
            br#"{"schema_version":"psionic.train.checkpoint_manifest.v1","step":4}"#,
        )
        .expect("checkpoint manifest should write");

        let mut surface =
            checkpoint_surface(Some("accepted"), Some("succeeded"), None, Some(false), None);
        surface.run_root = run_root.display().to_string();
        surface.artifacts.checkpoint_manifest_path =
            Some(checkpoint_manifest_path.display().to_string());
        write_json(
            run_root.join("status/checkpoint_surface.json").as_path(),
            &surface,
        );

        let mut binding = build_psionic_train_artifact_binding_from_path(
            "checkpoint_manifest",
            checkpoint_manifest_path.as_path(),
        )
        .expect("checkpoint manifest binding should build");
        binding.materialized_path = Some(String::from(
            "/Users/sasquatchorwell/.openagents/pylon/training/runs/run-x/windows/window-y/contributions/contribution-z/retained_artifacts/checkpoint_manifest/stale-checkpoint_manifest_step-000004.json",
        ));

        let resolved = materialize_validator_artifact_binding(
            &binding,
            &run_root,
            "contribution_artifact_manifest.artifacts[checkpoint_manifest]",
        )
        .expect("validator replay should resolve local checkpoint manifest");
        assert_eq!(resolved, checkpoint_manifest_path);
    }

    #[test]
    fn validator_replay_rebases_stale_absolute_paths_into_local_run_root() {
        let run_root = temp_root("run.cs336.a1.validator-replay-rebases-stale-absolute-paths");
        let rebased_path = run_root.join(
            "windows/window-0001/contributions/contribution-0001/retained_artifacts/current_status/current_run_status.json",
        );
        fs::create_dir_all(
            rebased_path
                .parent()
                .expect("rebased artifact parent should exist"),
        )
        .expect("rebased artifact parent should create");
        fs::write(&rebased_path, br#"{"status":"ok"}"#).expect("rebased artifact should write");

        let mut binding = build_psionic_train_artifact_binding_from_path(
            "current_status",
            rebased_path.as_path(),
        )
        .expect("current status binding should build");
        let stale_path = format!(
            "/Users/sasquatchorwell/.openagents/pylon/training/runs/{}/windows/window-0001/contributions/contribution-0001/retained_artifacts/current_status/current_run_status.json",
            run_root
                .file_name()
                .expect("run root should have file name")
                .to_string_lossy(),
        );
        binding.materialized_path = Some(stale_path);

        let resolved = materialize_validator_artifact_binding(
            &binding,
            &run_root,
            "contribution_artifact_manifest.artifacts[current_status]",
        )
        .expect("validator replay should rebase stale absolute paths into the local run root");
        assert_eq!(resolved, rebased_path);
    }

    #[test]
    fn apple_validation_replay_emits_weak_device_proof() {
        let run_root = temp_root("apple-weak-device-validation");
        let checkpoint_surface_path = run_root.join("status/checkpoint_surface.json");
        write_json(
            checkpoint_surface_path.as_path(),
            &checkpoint_surface(Some("accepted"), Some("succeeded"), None, Some(false), None),
        );

        let contribution_root = run_root.join("challenged");
        let artifact_manifest_path = contribution_root.join("artifact_manifest.json");
        let contribution_receipt_path = contribution_root.join("contribution_receipt.json");
        let mut artifact_manifest = PsionicTrainContributionArtifactManifest {
            schema_version: String::from(
                PSIONIC_TRAIN_CONTRIBUTION_ARTIFACT_MANIFEST_SCHEMA_VERSION,
            ),
            lane_id: String::from(PSION_APPLE_WINDOWED_TRAINING_LANE_ID),
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            run_id: String::from("worker-run"),
            window_id: String::from("window-0001"),
            assignment_id: String::from("assignment-0001"),
            contribution_id: String::from("contribution-0001"),
            node_pubkey: String::from("npub1-worker"),
            grouped_stage_assignment: None,
            artifact_count: 1,
            artifacts: vec![PsionicTrainContributionArtifact {
                artifact_kind: String::from("checkpoint_surface"),
                binding: build_psionic_train_artifact_binding_from_path(
                    "checkpoint_surface",
                    checkpoint_surface_path.as_path(),
                )
                .expect("checkpoint surface binding should build"),
            }],
            artifact_manifest_digest: String::new(),
        };
        artifact_manifest.artifact_manifest_digest =
            artifact_manifest.stable_artifact_manifest_digest();
        write_json(artifact_manifest_path.as_path(), &artifact_manifest);

        let mut contribution_receipt = PsionicTrainContributionReceipt {
            schema_version: String::from(PSIONIC_TRAIN_CONTRIBUTION_RECEIPT_SCHEMA_VERSION),
            lane_id: String::from(PSION_APPLE_WINDOWED_TRAINING_LANE_ID),
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            run_id: String::from("worker-run"),
            window_id: String::from("window-0001"),
            window_execution_id: String::from("window-execution-1"),
            assignment_id: String::from("assignment-0001"),
            contribution_id: String::from("contribution-0001"),
            node_pubkey: String::from("npub1-worker"),
            grouped_stage_assignment: None,
            role: PsionicTrainRole::Worker,
            operation: String::from("record-checkpoint"),
            outcome: PsionicTrainOutcomeKind::Succeeded,
            exit_code: 0,
            retryable: false,
            authority_owner: PsionicTrainAuthorityOwner::Pylon,
            refusal_class: None,
            artifact_manifest: artifact_binding(
                artifact_manifest_path.display().to_string().as_str(),
            ),
            artifact_manifest_digest: artifact_manifest.artifact_manifest_digest.clone(),
            artifact_count: artifact_manifest.artifact_count,
            contribution_digest: String::new(),
            detail: String::from("successful contribution"),
        };
        contribution_receipt.contribution_digest =
            contribution_receipt.stable_contribution_digest();
        write_json(contribution_receipt_path.as_path(), &contribution_receipt);

        let manifest = apple_validator_manifest(
            &run_root,
            contribution_receipt_path.display().to_string(),
            artifact_manifest_path.display().to_string(),
            PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
        );
        let execution = execute_psionic_train_validator_replay(&manifest, &run_root)
            .expect("apple validator replay should succeed");
        let proof_path = execution
            .artifacts
            .weak_device_validation_replay_proof_path
            .as_deref()
            .expect("apple validator replay should emit weak-device proof");
        let proof: PsionicTrainWeakDeviceValidationReplayProof =
            serde_json::from_slice(&fs::read(proof_path).expect("proof bytes should read"))
                .expect("proof json should parse");
        proof.validate().expect("proof should validate");
        assert_eq!(
            proof.public_count_class,
            PsionicTrainWeakDevicePublicCountClass::ValidatorRecognizedParticipation
        );
        assert_eq!(
            proof.challenged_work_class,
            PsionicTrainWorkClass::FullIslandLocalUpdateTraining
        );
        assert_eq!(proof.validator_node_pubkey, "npub1-apple-validator");
        assert_eq!(
            proof.validator_disposition,
            TrainingExecutionValidatorDisposition::Accepted
        );
    }
}
