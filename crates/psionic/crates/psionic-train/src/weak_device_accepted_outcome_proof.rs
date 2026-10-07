use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    PSION_APPLE_WINDOWED_TRAINING_LANE_ID, PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY,
    PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS, PsionicTrainCheckpointManifest,
    PsionicTrainCheckpointPointer, PsionicTrainContributionArtifactManifest,
    PsionicTrainContributionReceipt, PsionicTrainGroupedReplicaStageAssignment,
    PsionicTrainInvocationManifest, PsionicTrainOutcomeKind, PsionicTrainRunStatusPacket,
    PsionicTrainSealedWindowBundle, PsionicTrainValidatorHook,
    PsionicTrainValidatorQualityDriftSignal, PsionicTrainValidatorQualityDriftState,
    PsionicTrainValidatorRollbackPosture, PsionicTrainValidatorRollbackSignal,
    PsionicTrainValidatorScoreArtifact, PsionicTrainValidatorScoreReceipt,
    PsionicTrainWindowExecution, PsionicTrainWorkClass, TrainingExecutionValidatorDisposition,
    inspect_psionic_train_checkpoint_surface, load_psionic_train_grouped_stage_execution_summary,
    load_psionic_train_grouped_stage_replay_evidence,
};

pub const PSIONIC_TRAIN_WEAK_DEVICE_ACCEPTED_OUTCOME_PROOF_SCHEMA_VERSION: &str =
    "psionic.train.weak_device_accepted_outcome_proof.v1";
pub const PSIONIC_TRAIN_WEAK_DEVICE_VALIDATION_REPLAY_PROOF_SCHEMA_VERSION: &str =
    "psionic.train.weak_device_validation_replay_proof.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef {
    pub artifact_role: String,
    pub artifact_path: String,
    pub artifact_digest: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainWeakDevicePublicCountClass {
    ValidatorRecognizedParticipation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceValidationReplayProof {
    pub schema_version: String,
    pub proof_id: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub validator_run_id: String,
    pub validator_node_pubkey: String,
    pub backend_family: String,
    pub topology_class: String,
    pub weak_device_bearing: bool,
    pub challenged_run_id: String,
    pub challenged_node_pubkey: String,
    pub challenged_work_class: PsionicTrainWorkClass,
    pub window_id: String,
    pub assignment_id: String,
    pub challenge_id: String,
    pub contribution_id: String,
    pub contribution_digest: String,
    pub artifact_manifest_digest: String,
    pub public_count_class: PsionicTrainWeakDevicePublicCountClass,
    pub validator_disposition: TrainingExecutionValidatorDisposition,
    pub validator_score_bps: u16,
    pub verified_hooks: Vec<PsionicTrainValidatorHook>,
    pub quality_drift_state: PsionicTrainValidatorQualityDriftState,
    pub rollback_posture: PsionicTrainValidatorRollbackPosture,
    pub cited_artifacts: Vec<PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef>,
    pub claim_boundary: String,
    pub detail: String,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceCarrier {
    pub contributor_run_id: String,
    pub contributor_node_pubkey: String,
    pub backend_family: String,
    pub topology_class: String,
    pub work_class: PsionicTrainWorkClass,
    pub grouped_stage_assignment: PsionicTrainGroupedReplicaStageAssignment,
    pub weak_device_bearing: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceWindowProof {
    pub network_id: Option<String>,
    pub window_id: String,
    pub assignment_id: String,
    pub window_execution_id: String,
    pub window_digest: String,
    pub sealed_window_digest: String,
    pub contribution_id: String,
    pub contribution_digest: String,
    pub artifact_manifest_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceValidatorProof {
    pub validator_run_id: String,
    pub challenge_id: String,
    pub validation_index: u64,
    pub validator_disposition: TrainingExecutionValidatorDisposition,
    pub validator_score_bps: u16,
    pub verified_hooks: Vec<PsionicTrainValidatorHook>,
    pub quality_drift_state: PsionicTrainValidatorQualityDriftState,
    pub rollback_posture: PsionicTrainValidatorRollbackPosture,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceCheckpointProof {
    pub checkpoint_label: String,
    pub checkpoint_ref: String,
    pub checkpoint_pointer_state: String,
    pub optimizer_step: u64,
    pub checkpoint_manifest_digest: String,
    pub checkpoint_object_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWeakDeviceAcceptedOutcomeProof {
    pub schema_version: String,
    pub proof_id: String,
    pub lane_id: String,
    pub network_id: Option<String>,
    pub carrier: PsionicTrainWeakDeviceCarrier,
    pub window: PsionicTrainWeakDeviceWindowProof,
    pub validator: PsionicTrainWeakDeviceValidatorProof,
    pub checkpoint: PsionicTrainWeakDeviceCheckpointProof,
    pub cited_artifacts: Vec<PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef>,
    pub claim_boundary: String,
    pub detail: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum PsionicTrainWeakDeviceAcceptedOutcomeProofError {
    #[error("failed to read `{path}`: {detail}")]
    Read { path: String, detail: String },
    #[error("failed to write `{path}`: {detail}")]
    Write { path: String, detail: String },
    #[error("failed to parse `{path}`: {detail}")]
    Parse { path: String, detail: String },
    #[error("weak-device accepted-outcome proof is invalid: {detail}")]
    Invalid { detail: String },
}

impl PsionicTrainWeakDeviceAcceptedOutcomeProof {
    #[must_use]
    pub fn stable_bundle_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.bundle_digest.clear();
        stable_digest(
            b"psionic_train_weak_device_accepted_outcome_proof|",
            &digest_basis,
        )
    }

    pub fn validate(&self) -> Result<(), PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
        require_nonempty(self.schema_version.as_str(), "proof.schema_version")?;
        if self.schema_version != PSIONIC_TRAIN_WEAK_DEVICE_ACCEPTED_OUTCOME_PROOF_SCHEMA_VERSION {
            return Err(invalid(format!(
                "proof schema version must stay `{}` but was `{}`",
                PSIONIC_TRAIN_WEAK_DEVICE_ACCEPTED_OUTCOME_PROOF_SCHEMA_VERSION,
                self.schema_version
            )));
        }
        require_nonempty(self.proof_id.as_str(), "proof.proof_id")?;
        require_nonempty(self.lane_id.as_str(), "proof.lane_id")?;
        require_nonempty(
            self.carrier.contributor_run_id.as_str(),
            "proof.carrier.contributor_run_id",
        )?;
        require_nonempty(
            self.carrier.contributor_node_pubkey.as_str(),
            "proof.carrier.contributor_node_pubkey",
        )?;
        require_nonempty(
            self.carrier.backend_family.as_str(),
            "proof.carrier.backend_family",
        )?;
        require_nonempty(
            self.carrier.topology_class.as_str(),
            "proof.carrier.topology_class",
        )?;
        require_nonempty(self.carrier.detail.as_str(), "proof.carrier.detail")?;
        if !self.carrier.weak_device_bearing {
            return Err(invalid(String::from(
                "proof carrier must stay explicitly marked as weak-device-bearing",
            )));
        }
        if !matches!(
            self.carrier.work_class,
            PsionicTrainWorkClass::AdapterTraining
                | PsionicTrainWorkClass::SmallModelLocalTraining
                | PsionicTrainWorkClass::GroupedReplicaStageExecution
        ) {
            return Err(invalid(format!(
                "proof carrier work class `{}` is not admitted as weak-device-bearing",
                self.carrier.work_class.label()
            )));
        }
        self.carrier
            .grouped_stage_assignment
            .validate("proof.carrier.grouped_stage_assignment")
            .map_err(|error| invalid(error.to_string()))?;
        if self.carrier.work_class == PsionicTrainWorkClass::GroupedReplicaStageExecution
            && self.carrier.grouped_stage_assignment.stage_count < 2
        {
            return Err(invalid(String::from(
                "grouped weak-device proof must cite one multi-stage grouped replica assignment",
            )));
        }

        require_nonempty(self.window.window_id.as_str(), "proof.window.window_id")?;
        require_nonempty(
            self.window.assignment_id.as_str(),
            "proof.window.assignment_id",
        )?;
        require_nonempty(
            self.window.window_execution_id.as_str(),
            "proof.window.window_execution_id",
        )?;
        require_nonempty(
            self.window.window_digest.as_str(),
            "proof.window.window_digest",
        )?;
        require_nonempty(
            self.window.sealed_window_digest.as_str(),
            "proof.window.sealed_window_digest",
        )?;
        require_nonempty(
            self.window.contribution_id.as_str(),
            "proof.window.contribution_id",
        )?;
        require_nonempty(
            self.window.contribution_digest.as_str(),
            "proof.window.contribution_digest",
        )?;
        require_nonempty(
            self.window.artifact_manifest_digest.as_str(),
            "proof.window.artifact_manifest_digest",
        )?;
        require_nonempty(self.window.detail.as_str(), "proof.window.detail")?;

        require_nonempty(
            self.validator.validator_run_id.as_str(),
            "proof.validator.validator_run_id",
        )?;
        require_nonempty(
            self.validator.challenge_id.as_str(),
            "proof.validator.challenge_id",
        )?;
        require_nonempty(self.validator.detail.as_str(), "proof.validator.detail")?;
        if self.validator.validation_index == 0 {
            return Err(invalid(String::from(
                "proof validator validation_index must be non-zero",
            )));
        }
        if self.validator.validator_disposition != TrainingExecutionValidatorDisposition::Accepted {
            return Err(invalid(String::from(
                "proof validator disposition must stay `accepted`",
            )));
        }
        if self.validator.validator_score_bps != 10_000 {
            return Err(invalid(String::from(
                "proof validator score must stay at the accepted 10000 bps ceiling",
            )));
        }
        if self.validator.rollback_posture != PsionicTrainValidatorRollbackPosture::Hold {
            return Err(invalid(String::from(
                "proof rollback posture must stay `hold` for the accepted weak-device lane",
            )));
        }
        if self.validator.verified_hooks.is_empty() {
            return Err(invalid(String::from(
                "proof validator hooks must not be empty",
            )));
        }
        let hook_set = normalized_hook_labels(&self.validator.verified_hooks);
        let expected_hook_set = normalized_hook_labels(&[
            PsionicTrainValidatorHook::AssignmentCorrectness,
            PsionicTrainValidatorHook::CheckpointLineage,
            PsionicTrainValidatorHook::WorkExecutionPlausibility,
            PsionicTrainValidatorHook::UpdateIntegrity,
            PsionicTrainValidatorHook::GroupedStageIntegrity,
        ]);
        if hook_set != expected_hook_set {
            return Err(invalid(format!(
                "proof validator hooks drifted from the grouped-stage accepted set: expected {:?}, got {:?}",
                expected_hook_set, hook_set
            )));
        }

        require_nonempty(
            self.checkpoint.checkpoint_label.as_str(),
            "proof.checkpoint.checkpoint_label",
        )?;
        require_nonempty(
            self.checkpoint.checkpoint_ref.as_str(),
            "proof.checkpoint.checkpoint_ref",
        )?;
        require_nonempty(
            self.checkpoint.checkpoint_pointer_state.as_str(),
            "proof.checkpoint.checkpoint_pointer_state",
        )?;
        require_nonempty(
            self.checkpoint.checkpoint_manifest_digest.as_str(),
            "proof.checkpoint.checkpoint_manifest_digest",
        )?;
        require_nonempty(
            self.checkpoint.checkpoint_object_digest.as_str(),
            "proof.checkpoint.checkpoint_object_digest",
        )?;
        require_nonempty(self.checkpoint.detail.as_str(), "proof.checkpoint.detail")?;
        if self.checkpoint.optimizer_step == 0 {
            return Err(invalid(String::from(
                "proof checkpoint optimizer_step must be non-zero",
            )));
        }
        if !matches!(
            self.checkpoint.checkpoint_pointer_state.as_str(),
            "accepted" | "accepted_primary"
        ) {
            return Err(invalid(format!(
                "proof checkpoint pointer state `{}` is not admitted",
                self.checkpoint.checkpoint_pointer_state
            )));
        }

        if self.cited_artifacts.is_empty() {
            return Err(invalid(String::from(
                "proof cited_artifacts must not be empty",
            )));
        }
        let mut artifact_roles = BTreeSet::new();
        for artifact in &self.cited_artifacts {
            require_nonempty(
                artifact.artifact_role.as_str(),
                "proof.cited_artifacts[].artifact_role",
            )?;
            require_nonempty(
                artifact.artifact_path.as_str(),
                "proof.cited_artifacts[].artifact_path",
            )?;
            require_nonempty(
                artifact.artifact_digest.as_str(),
                "proof.cited_artifacts[].artifact_digest",
            )?;
            require_nonempty(artifact.detail.as_str(), "proof.cited_artifacts[].detail")?;
            artifact_roles.insert(artifact.artifact_role.as_str());
        }
        for required_role in [
            "window_execution",
            "contribution_receipt",
            "contribution_artifact_manifest",
            "grouped_stage_execution_summary",
            "grouped_stage_replay_evidence",
            "checkpoint_surface",
            "checkpoint_pointer",
            "checkpoint_manifest",
            "sealed_window_bundle",
            "validator_score_artifact",
            "validator_score_receipt",
            "validator_quality_drift_signal",
            "validator_rollback_signal",
        ] {
            if !artifact_roles.contains(required_role) {
                return Err(invalid(format!(
                    "proof cited_artifacts is missing required role `{required_role}`",
                )));
            }
        }

        require_nonempty(self.claim_boundary.as_str(), "proof.claim_boundary")?;
        require_nonempty(self.detail.as_str(), "proof.detail")?;
        require_nonempty(self.bundle_digest.as_str(), "proof.bundle_digest")?;
        if self.bundle_digest != self.stable_bundle_digest() {
            return Err(invalid(String::from(
                "proof bundle_digest drifted from canonical contents",
            )));
        }
        Ok(())
    }
}

impl PsionicTrainWeakDeviceValidationReplayProof {
    #[must_use]
    pub fn stable_bundle_digest(&self) -> String {
        let mut digest_basis = self.clone();
        digest_basis.bundle_digest.clear();
        stable_digest(
            b"psionic_train_weak_device_validation_replay_proof|",
            &digest_basis,
        )
    }

    pub fn validate(&self) -> Result<(), PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
        require_nonempty(self.schema_version.as_str(), "proof.schema_version")?;
        if self.schema_version != PSIONIC_TRAIN_WEAK_DEVICE_VALIDATION_REPLAY_PROOF_SCHEMA_VERSION {
            return Err(invalid(format!(
                "proof schema version must stay `{}` but was `{}`",
                PSIONIC_TRAIN_WEAK_DEVICE_VALIDATION_REPLAY_PROOF_SCHEMA_VERSION,
                self.schema_version
            )));
        }
        require_nonempty(self.proof_id.as_str(), "proof.proof_id")?;
        require_nonempty(self.lane_id.as_str(), "proof.lane_id")?;
        require_nonempty(self.validator_run_id.as_str(), "proof.validator_run_id")?;
        require_nonempty(
            self.validator_node_pubkey.as_str(),
            "proof.validator_node_pubkey",
        )?;
        require_nonempty(self.backend_family.as_str(), "proof.backend_family")?;
        require_nonempty(self.topology_class.as_str(), "proof.topology_class")?;
        require_nonempty(self.challenged_run_id.as_str(), "proof.challenged_run_id")?;
        require_nonempty(
            self.challenged_node_pubkey.as_str(),
            "proof.challenged_node_pubkey",
        )?;
        require_nonempty(self.window_id.as_str(), "proof.window_id")?;
        require_nonempty(self.assignment_id.as_str(), "proof.assignment_id")?;
        require_nonempty(self.challenge_id.as_str(), "proof.challenge_id")?;
        require_nonempty(self.contribution_id.as_str(), "proof.contribution_id")?;
        require_nonempty(
            self.contribution_digest.as_str(),
            "proof.contribution_digest",
        )?;
        require_nonempty(
            self.artifact_manifest_digest.as_str(),
            "proof.artifact_manifest_digest",
        )?;
        if !self.weak_device_bearing {
            return Err(invalid(String::from(
                "validation replay proof must stay explicitly marked as weak-device-bearing",
            )));
        }
        if self.backend_family != PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY
            || self.topology_class != PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS
        {
            return Err(invalid(format!(
                "validation replay proof capability projection `{}` / `{}` is not the admitted Apple weak-device surface",
                self.backend_family, self.topology_class
            )));
        }
        if !self.challenged_work_class.is_validator_target_admitted() {
            return Err(invalid(format!(
                "validation replay proof challenged work class `{}` is not admitted for validator replay",
                self.challenged_work_class.label()
            )));
        }
        if self.validator_disposition != TrainingExecutionValidatorDisposition::Accepted {
            return Err(invalid(String::from(
                "validation replay proof requires one accepted validator disposition",
            )));
        }
        if self.validator_score_bps != 10_000 {
            return Err(invalid(String::from(
                "validation replay proof requires the accepted 10000 bps score ceiling",
            )));
        }
        if self.verified_hooks.is_empty() {
            return Err(invalid(String::from(
                "validation replay proof requires one non-empty verified hook set",
            )));
        }
        if self.quality_drift_state == PsionicTrainValidatorQualityDriftState::Regressed {
            return Err(invalid(String::from(
                "validation replay proof does not admit regressed quality drift state",
            )));
        }
        if self.rollback_posture != PsionicTrainValidatorRollbackPosture::Hold {
            return Err(invalid(String::from(
                "validation replay proof requires rollback posture `hold`",
            )));
        }
        if self.cited_artifacts.is_empty() {
            return Err(invalid(String::from(
                "validation replay proof cited_artifacts must not be empty",
            )));
        }
        let mut artifact_roles = BTreeSet::new();
        for artifact in &self.cited_artifacts {
            require_nonempty(
                artifact.artifact_role.as_str(),
                "proof.cited_artifacts[].artifact_role",
            )?;
            require_nonempty(
                artifact.artifact_path.as_str(),
                "proof.cited_artifacts[].artifact_path",
            )?;
            require_nonempty(
                artifact.artifact_digest.as_str(),
                "proof.cited_artifacts[].artifact_digest",
            )?;
            require_nonempty(artifact.detail.as_str(), "proof.cited_artifacts[].detail")?;
            artifact_roles.insert(artifact.artifact_role.as_str());
        }
        for required_role in [
            "contribution_receipt",
            "contribution_artifact_manifest",
            "validator_score_artifact",
            "validator_score_receipt",
            "validator_quality_drift_signal",
            "validator_rollback_signal",
        ] {
            if !artifact_roles.contains(required_role) {
                return Err(invalid(format!(
                    "validation replay proof cited_artifacts is missing required role `{required_role}`",
                )));
            }
        }
        require_nonempty(self.claim_boundary.as_str(), "proof.claim_boundary")?;
        require_nonempty(self.detail.as_str(), "proof.detail")?;
        require_nonempty(self.bundle_digest.as_str(), "proof.bundle_digest")?;
        if self.bundle_digest != self.stable_bundle_digest() {
            return Err(invalid(String::from(
                "validation replay proof bundle_digest drifted from canonical contents",
            )));
        }
        Ok(())
    }
}

pub fn maybe_record_psionic_train_weak_device_validation_replay_proof(
    output_path: &Path,
    manifest: &PsionicTrainInvocationManifest,
    contribution_receipt_path: &Path,
    contribution_receipt: &PsionicTrainContributionReceipt,
    contribution_artifact_manifest_path: &Path,
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
    score_artifact_path: &Path,
    score_artifact: &PsionicTrainValidatorScoreArtifact,
    score_receipt_path: &Path,
    score_receipt: &PsionicTrainValidatorScoreReceipt,
    quality_drift_signal_path: &Path,
    quality_drift_signal: &PsionicTrainValidatorQualityDriftSignal,
    rollback_signal_path: &Path,
    rollback_signal: &PsionicTrainValidatorRollbackSignal,
) -> Result<
    Option<PsionicTrainWeakDeviceValidationReplayProof>,
    PsionicTrainWeakDeviceAcceptedOutcomeProofError,
> {
    if manifest.lane_id != PSION_APPLE_WINDOWED_TRAINING_LANE_ID
        || manifest.work_class != PsionicTrainWorkClass::ValidationReplay
    {
        return Ok(None);
    }

    if score_receipt.disposition != TrainingExecutionValidatorDisposition::Accepted
        || score_artifact.disposition != TrainingExecutionValidatorDisposition::Accepted
        || score_receipt.score_bps != 10_000
        || score_artifact.score_bps != 10_000
        || rollback_signal.rollback_posture != PsionicTrainValidatorRollbackPosture::Hold
        || quality_drift_signal.drift_state == PsionicTrainValidatorQualityDriftState::Regressed
    {
        return Ok(None);
    }

    let validator_run_id = manifest.run_id.as_deref().ok_or_else(|| {
        invalid(String::from(
            "weak-device validator manifest is missing run_id",
        ))
    })?;
    let validator_node_pubkey = manifest
        .coordination
        .node_pubkey
        .as_deref()
        .ok_or_else(|| {
            invalid(String::from(
                "weak-device validator manifest is missing coordination.node_pubkey",
            ))
        })?;
    require_eq(
        validator_run_id,
        score_receipt.validator_run_id.as_str(),
        "validation replay proof validator_run_id",
    )?;
    require_eq(
        validator_node_pubkey,
        score_receipt.validator_node_pubkey.as_str(),
        "validation replay proof validator_node_pubkey",
    )?;
    require_eq(
        contribution_receipt.run_id.as_str(),
        score_receipt.challenged_run_id.as_str(),
        "validation replay proof challenged_run_id",
    )?;
    require_eq(
        contribution_receipt.node_pubkey.as_str(),
        score_receipt.challenged_node_pubkey.as_str(),
        "validation replay proof challenged_node_pubkey",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        score_receipt.window_id.as_str(),
        "validation replay proof window_id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        score_receipt.assignment_id.as_str(),
        "validation replay proof assignment_id",
    )?;
    require_eq(
        contribution_receipt.contribution_id.as_str(),
        score_receipt.contribution_id.as_str(),
        "validation replay proof contribution_id",
    )?;
    require_eq(
        contribution_receipt.contribution_digest.as_str(),
        score_receipt.contribution_digest.as_str(),
        "validation replay proof contribution_digest",
    )?;
    require_eq(
        contribution_artifact_manifest
            .artifact_manifest_digest
            .as_str(),
        score_receipt.artifact_manifest_digest.as_str(),
        "validation replay proof artifact_manifest_digest",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        quality_drift_signal.current_window_id.as_str(),
        "validation replay proof quality_drift_signal.window_id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        quality_drift_signal.current_assignment_id.as_str(),
        "validation replay proof quality_drift_signal.assignment_id",
    )?;
    require_eq(
        score_receipt.challenge_id.as_str(),
        quality_drift_signal.current_challenge_id.as_str(),
        "validation replay proof quality_drift_signal.challenge_id",
    )?;
    require_eq(
        score_receipt.challenge_id.as_str(),
        rollback_signal.current_challenge_id.as_str(),
        "validation replay proof rollback_signal.challenge_id",
    )?;
    require_eq(
        score_receipt.score_receipt_digest.as_str(),
        quality_drift_signal.validator_score_receipt_digest.as_str(),
        "validation replay proof quality_drift_signal.validator_score_receipt_digest",
    )?;
    require_eq(
        score_receipt.score_receipt_digest.as_str(),
        rollback_signal.validator_score_receipt_digest.as_str(),
        "validation replay proof rollback_signal.validator_score_receipt_digest",
    )?;
    if score_receipt.challenged_work_class != contribution_receipt.work_class
        || score_receipt.challenged_work_class != contribution_artifact_manifest.work_class
    {
        return Err(invalid(String::from(
            "validation replay proof challenged work class drifted across retained replay artifacts",
        )));
    }

    let mut cited_artifacts = vec![
        artifact_ref(
            "contribution_receipt",
            contribution_receipt_path.display().to_string().as_str(),
            contribution_receipt.contribution_digest.as_str(),
            "Accepted challenged contribution receipt cited by the weak-device validator replay proof.",
        )?,
        artifact_ref(
            "contribution_artifact_manifest",
            contribution_artifact_manifest_path
                .display()
                .to_string()
                .as_str(),
            contribution_artifact_manifest
                .artifact_manifest_digest
                .as_str(),
            "Accepted challenged contribution artifact manifest cited by the weak-device validator replay proof.",
        )?,
        artifact_ref(
            "validator_score_artifact",
            score_artifact_path.display().to_string().as_str(),
            score_artifact.score_digest.as_str(),
            "Accepted validator score artifact that records the bounded replay hooks for the weak-device lane.",
        )?,
        artifact_ref(
            "validator_score_receipt",
            score_receipt_path.display().to_string().as_str(),
            score_receipt.score_receipt_digest.as_str(),
            "Accepted validator score receipt retained for the weak-device validation_replay assignment.",
        )?,
        artifact_ref(
            "validator_quality_drift_signal",
            quality_drift_signal_path.display().to_string().as_str(),
            quality_drift_signal.drift_signal_digest.as_str(),
            "Quality-drift signal proving the accepted weak-device validator replay did not regress below the retained validator baseline.",
        )?,
        artifact_ref(
            "validator_rollback_signal",
            rollback_signal_path.display().to_string().as_str(),
            rollback_signal.rollback_signal_digest.as_str(),
            "Rollback signal proving the accepted weak-device validator replay remains in hold posture.",
        )?,
    ];
    if let (Some(path), Some(digest)) = (
        score_receipt.grouped_stage_replay_evidence_path.as_deref(),
        score_receipt
            .grouped_stage_replay_evidence_digest
            .as_deref(),
    ) {
        cited_artifacts.push(artifact_ref(
            "grouped_stage_replay_evidence",
            path,
            digest,
            "Grouped-stage replay evidence carried through the weak-device validator proof because the challenged contribution used grouped stage execution.",
        )?);
    }
    if let (Some(path), Some(digest)) = (
        score_receipt
            .grouped_stage_execution_summary_path
            .as_deref(),
        score_receipt
            .grouped_stage_execution_summary_digest
            .as_deref(),
    ) {
        cited_artifacts.push(artifact_ref(
            "grouped_stage_execution_summary",
            path,
            digest,
            "Grouped-stage execution summary cited when the weak-device validator replay challenged a grouped-stage contribution.",
        )?);
    }

    let mut proof = PsionicTrainWeakDeviceValidationReplayProof {
        schema_version: String::from(
            PSIONIC_TRAIN_WEAK_DEVICE_VALIDATION_REPLAY_PROOF_SCHEMA_VERSION,
        ),
        proof_id: format!(
            "weak-device-validation-replay:{}:{}:{}",
            manifest.lane_id, score_receipt.window_id, score_receipt.challenge_id
        ),
        lane_id: manifest.lane_id.clone(),
        network_id: manifest.coordination.network_id.clone(),
        validator_run_id: String::from(validator_run_id),
        validator_node_pubkey: String::from(validator_node_pubkey),
        backend_family: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY),
        topology_class: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS),
        weak_device_bearing: true,
        challenged_run_id: contribution_receipt.run_id.clone(),
        challenged_node_pubkey: contribution_receipt.node_pubkey.clone(),
        challenged_work_class: contribution_receipt.work_class,
        window_id: contribution_receipt.window_id.clone(),
        assignment_id: contribution_receipt.assignment_id.clone(),
        challenge_id: score_receipt.challenge_id.clone(),
        contribution_id: contribution_receipt.contribution_id.clone(),
        contribution_digest: contribution_receipt.contribution_digest.clone(),
        artifact_manifest_digest: contribution_artifact_manifest
            .artifact_manifest_digest
            .clone(),
        public_count_class:
            PsionicTrainWeakDevicePublicCountClass::ValidatorRecognizedParticipation,
        validator_disposition: score_receipt.disposition,
        validator_score_bps: score_receipt.score_bps,
        verified_hooks: score_receipt.verified_hooks.clone(),
        quality_drift_state: quality_drift_signal.drift_state,
        rollback_posture: rollback_signal.rollback_posture,
        cited_artifacts,
        claim_boundary: String::from(
            "This proof bundle cites one accepted weak-device validation_replay assignment. It counts as validator-recognized participation on the public weak-device lane. It does not claim direct model-progress credit, checkpoint promotion authority, payout closeout, or network-wide finality beyond the retained Psionic validator surfaces cited here.",
        ),
        detail: String::from(
            "Weak-device validation replay proof keeps the launch claim narrow and machine-readable: one weaker Apple/Metal node completed one accepted validation_replay assignment, the bounded replay hooks stayed explicit, quality did not regress, and rollback posture remained hold.",
        ),
        bundle_digest: String::new(),
    };
    proof.bundle_digest = proof.stable_bundle_digest();
    proof.validate()?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
                path: parent.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&proof).map_err(|error| {
            PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
                path: output_path.display().to_string(),
                detail: error.to_string(),
            }
        })?,
    )
    .map_err(
        |error| PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
            path: output_path.display().to_string(),
            detail: error.to_string(),
        },
    )?;
    Ok(Some(proof))
}

pub fn record_psionic_train_weak_device_accepted_outcome_proof(
    output_path: &Path,
    contributor_status: &PsionicTrainRunStatusPacket,
    validator_status: &PsionicTrainRunStatusPacket,
) -> Result<
    PsionicTrainWeakDeviceAcceptedOutcomeProof,
    PsionicTrainWeakDeviceAcceptedOutcomeProofError,
> {
    if contributor_status.outcome != PsionicTrainOutcomeKind::Succeeded {
        return Err(invalid(String::from(
            "contributor status must represent one succeeded contribution",
        )));
    }
    if validator_status.outcome != PsionicTrainOutcomeKind::Succeeded {
        return Err(invalid(String::from(
            "validator status must represent one succeeded replay",
        )));
    }
    if contributor_status.work_class != PsionicTrainWorkClass::GroupedReplicaStageExecution {
        return Err(invalid(format!(
            "contributor work class must stay `grouped_replica_stage_execution` but was `{}`",
            contributor_status.work_class.label()
        )));
    }
    if validator_status.work_class != PsionicTrainWorkClass::ValidationReplay {
        return Err(invalid(format!(
            "validator work class must stay `validation_replay` but was `{}`",
            validator_status.work_class.label()
        )));
    }
    if validator_status.validator_target_work_class
        != Some(PsionicTrainWorkClass::GroupedReplicaStageExecution)
    {
        return Err(invalid(String::from(
            "validator status must target one grouped-replica stage contribution",
        )));
    }
    if contributor_status.capability_projection.backend_family
        != PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY
        || contributor_status.capability_projection.topology_class
            != PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS
    {
        return Err(invalid(format!(
            "contributor capability projection `{}` / `{}` is not the admitted weak-device Apple surface",
            contributor_status.capability_projection.backend_family,
            contributor_status.capability_projection.topology_class
        )));
    }
    let grouped_stage_assignment = contributor_status
        .grouped_stage_assignment
        .clone()
        .ok_or_else(|| {
            invalid(String::from(
                "contributor status is missing grouped_stage_assignment",
            ))
        })?;

    let contributor_run_id = contributor_status
        .run_id
        .clone()
        .ok_or_else(|| invalid(String::from("contributor status is missing run_id")))?;
    let validator_run_id = validator_status
        .run_id
        .clone()
        .ok_or_else(|| invalid(String::from("validator status is missing run_id")))?;
    let contributor_run_root = PathBuf::from(
        contributor_status
            .run_root
            .as_deref()
            .ok_or_else(|| invalid(String::from("contributor status is missing run_root")))?,
    );

    let window_execution_path = required_path(
        contributor_status
            .artifacts
            .window_execution_path
            .as_deref(),
        "window_execution_path",
    )?;
    let contribution_receipt_path = required_path(
        contributor_status
            .artifacts
            .contribution_receipt_path
            .as_deref(),
        "contribution_receipt_path",
    )?;
    let contribution_artifact_manifest_path = required_path(
        contributor_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_deref(),
        "contribution_artifact_manifest_path",
    )?;
    let grouped_stage_execution_summary_path = required_path(
        contributor_status
            .artifacts
            .grouped_stage_execution_summary_path
            .as_deref(),
        "grouped_stage_execution_summary_path",
    )?;
    let checkpoint_surface_path = required_path(
        contributor_status
            .artifacts
            .checkpoint_surface_path
            .as_deref(),
        "checkpoint_surface_path",
    )?;
    let checkpoint_pointer_path = required_path(
        contributor_status
            .artifacts
            .checkpoint_pointer_path
            .as_deref(),
        "checkpoint_pointer_path",
    )?;
    let checkpoint_manifest_path = required_path(
        contributor_status
            .artifacts
            .checkpoint_manifest_path
            .as_deref(),
        "checkpoint_manifest_path",
    )?;
    let sealed_window_bundle_path = required_path(
        contributor_status
            .artifacts
            .sealed_window_bundle_path
            .as_deref(),
        "sealed_window_bundle_path",
    )?;
    let validator_score_receipt_path = required_path(
        validator_status
            .artifacts
            .validator_score_receipt_path
            .as_deref(),
        "validator_score_receipt_path",
    )?;
    let grouped_stage_replay_evidence_path = required_path(
        validator_status
            .artifacts
            .grouped_stage_replay_evidence_path
            .as_deref(),
        "grouped_stage_replay_evidence_path",
    )?;
    let validator_quality_drift_signal_path = required_path(
        validator_status
            .artifacts
            .validator_quality_drift_signal_path
            .as_deref(),
        "validator_quality_drift_signal_path",
    )?;
    let validator_rollback_signal_path = required_path(
        validator_status
            .artifacts
            .validator_rollback_signal_path
            .as_deref(),
        "validator_rollback_signal_path",
    )?;

    let window_execution: PsionicTrainWindowExecution = read_json(window_execution_path)?;
    if window_execution.window_digest != stable_window_execution_digest(&window_execution) {
        return Err(invalid(String::from(
            "window execution digest drifted from canonical contents",
        )));
    }
    let contribution_receipt: PsionicTrainContributionReceipt =
        read_json(contribution_receipt_path)?;
    if contribution_receipt.contribution_digest != contribution_receipt.stable_contribution_digest()
    {
        return Err(invalid(String::from(
            "contribution receipt digest drifted from canonical contents",
        )));
    }
    let contribution_artifact_manifest: PsionicTrainContributionArtifactManifest =
        read_json(contribution_artifact_manifest_path)?;
    if contribution_artifact_manifest.artifact_manifest_digest
        != contribution_artifact_manifest.stable_artifact_manifest_digest()
    {
        return Err(invalid(String::from(
            "contribution artifact manifest digest drifted from canonical contents",
        )));
    }
    let grouped_stage_execution_summary = load_psionic_train_grouped_stage_execution_summary(
        Path::new(grouped_stage_execution_summary_path),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let grouped_stage_replay_evidence = load_psionic_train_grouped_stage_replay_evidence(
        Path::new(grouped_stage_replay_evidence_path),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let checkpoint_surface = inspect_psionic_train_checkpoint_surface(
        contributor_run_root.as_path(),
        contributor_status.role,
        contributor_status.operation,
    )
    .map_err(|error| invalid(error.to_string()))?
    .ok_or_else(|| {
        invalid(String::from(
            "checkpoint surface inspection returned no surface",
        ))
    })?;
    let checkpoint_pointer: PsionicTrainCheckpointPointer = read_json(checkpoint_pointer_path)?;
    checkpoint_pointer
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    let checkpoint_manifest: PsionicTrainCheckpointManifest = read_json(checkpoint_manifest_path)?;
    checkpoint_manifest
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    let sealed_window_bundle: PsionicTrainSealedWindowBundle =
        read_json(sealed_window_bundle_path)?;
    if sealed_window_bundle.sealed_window_digest
        != stable_sealed_window_bundle_digest(&sealed_window_bundle)
    {
        return Err(invalid(String::from(
            "sealed window bundle digest drifted from canonical contents",
        )));
    }
    let validator_score_receipt: PsionicTrainValidatorScoreReceipt =
        read_json(validator_score_receipt_path)?;
    if validator_score_receipt.score_receipt_digest
        != stable_validator_score_receipt_digest(&validator_score_receipt)
    {
        return Err(invalid(String::from(
            "validator score receipt digest drifted from canonical contents",
        )));
    }
    let validator_score_artifact: PsionicTrainValidatorScoreArtifact =
        read_json(validator_score_receipt.score_artifact_path.as_str())?;
    if validator_score_artifact.score_digest
        != stable_validator_score_artifact_digest(&validator_score_artifact)
    {
        return Err(invalid(String::from(
            "validator score artifact digest drifted from canonical contents",
        )));
    }
    let validator_quality_drift_signal: PsionicTrainValidatorQualityDriftSignal =
        read_json(validator_quality_drift_signal_path)?;
    if validator_quality_drift_signal.drift_signal_digest
        != stable_validator_quality_drift_signal_digest(&validator_quality_drift_signal)
    {
        return Err(invalid(String::from(
            "validator quality drift signal digest drifted from canonical contents",
        )));
    }
    let validator_rollback_signal: PsionicTrainValidatorRollbackSignal =
        read_json(validator_rollback_signal_path)?;
    if validator_rollback_signal.rollback_signal_digest
        != stable_validator_rollback_signal_digest(&validator_rollback_signal)
    {
        return Err(invalid(String::from(
            "validator rollback signal digest drifted from canonical contents",
        )));
    }

    let network_id = contributor_status.coordination.network_id.clone();
    if validator_status.coordination.network_id != network_id {
        return Err(invalid(String::from(
            "contributor and validator network ids must stay aligned",
        )));
    }
    require_eq(
        contributor_status.lane_id.as_str(),
        validator_status.lane_id.as_str(),
        "status lane ids",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        contribution_receipt.lane_id.as_str(),
        "contribution receipt lane_id",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        window_execution.lane_id.as_str(),
        "window execution lane_id",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        checkpoint_manifest.lane_id.as_str(),
        "checkpoint manifest lane_id",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        validator_score_receipt.lane_id.as_str(),
        "validator score receipt lane_id",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        validator_quality_drift_signal.lane_id.as_str(),
        "validator quality drift signal lane_id",
    )?;
    require_eq(
        contributor_status.lane_id.as_str(),
        validator_rollback_signal.lane_id.as_str(),
        "validator rollback signal lane_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        contribution_receipt.run_id.as_str(),
        "contribution receipt run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        window_execution.run_id.as_str(),
        "window execution run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        checkpoint_manifest.run_id.as_str(),
        "checkpoint manifest run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        checkpoint_pointer.run_id.as_str(),
        "checkpoint pointer run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        grouped_stage_execution_summary.run_id.as_str(),
        "grouped stage execution summary run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        grouped_stage_replay_evidence.challenged_run_id.as_str(),
        "grouped stage replay evidence challenged_run_id",
    )?;
    require_eq(
        contributor_run_id.as_str(),
        validator_score_receipt.challenged_run_id.as_str(),
        "validator score receipt challenged_run_id",
    )?;
    require_eq(
        validator_run_id.as_str(),
        validator_score_receipt.validator_run_id.as_str(),
        "validator score receipt validator_run_id",
    )?;
    require_eq(
        validator_run_id.as_str(),
        validator_quality_drift_signal.validator_run_id.as_str(),
        "validator quality drift signal validator_run_id",
    )?;
    require_eq(
        validator_run_id.as_str(),
        validator_rollback_signal.validator_run_id.as_str(),
        "validator rollback signal validator_run_id",
    )?;

    if contribution_receipt.work_class != PsionicTrainWorkClass::GroupedReplicaStageExecution
        || contribution_artifact_manifest.work_class
            != PsionicTrainWorkClass::GroupedReplicaStageExecution
        || validator_score_receipt.challenged_work_class
            != PsionicTrainWorkClass::GroupedReplicaStageExecution
        || validator_quality_drift_signal.challenged_work_class
            != PsionicTrainWorkClass::GroupedReplicaStageExecution
        || validator_rollback_signal.challenged_work_class
            != PsionicTrainWorkClass::GroupedReplicaStageExecution
    {
        return Err(invalid(String::from(
            "all retained artifacts must stay scoped to one grouped-replica stage work class",
        )));
    }

    if contribution_receipt.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment)
        || contribution_artifact_manifest
            .grouped_stage_assignment
            .as_ref()
            != Some(&grouped_stage_assignment)
        || window_execution.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment)
        || grouped_stage_execution_summary.grouped_stage_assignment != grouped_stage_assignment
        || grouped_stage_replay_evidence.grouped_stage_assignment != grouped_stage_assignment
        || checkpoint_surface.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment)
        || checkpoint_pointer.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment)
        || checkpoint_manifest.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment)
    {
        return Err(invalid(String::from(
            "grouped-stage assignment drifted across retained weak-device artifacts",
        )));
    }

    require_eq(
        contribution_receipt.window_id.as_str(),
        window_execution.window_id.as_str(),
        "window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        grouped_stage_execution_summary.window_id.as_str(),
        "grouped stage execution summary window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        grouped_stage_replay_evidence.window_id.as_str(),
        "grouped stage replay evidence window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        validator_score_receipt.window_id.as_str(),
        "validator score receipt window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        validator_quality_drift_signal.current_window_id.as_str(),
        "validator quality drift signal window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        validator_rollback_signal.current_window_id.as_str(),
        "validator rollback signal window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        checkpoint_pointer.window_id.as_deref().ok_or_else(|| {
            invalid(String::from(
                "checkpoint pointer is missing grouped window_id",
            ))
        })?,
        "checkpoint pointer window id",
    )?;
    require_eq(
        contribution_receipt.window_id.as_str(),
        checkpoint_manifest.window_id.as_deref().ok_or_else(|| {
            invalid(String::from(
                "checkpoint manifest is missing grouped window_id",
            ))
        })?,
        "checkpoint manifest window id",
    )?;

    require_eq(
        contribution_receipt.assignment_id.as_str(),
        window_execution.current_assignment.assignment_id.as_str(),
        "window assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        grouped_stage_execution_summary.assignment_id.as_str(),
        "grouped stage execution summary assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        grouped_stage_replay_evidence.assignment_id.as_str(),
        "grouped stage replay evidence assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        validator_score_receipt.assignment_id.as_str(),
        "validator score receipt assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        validator_quality_drift_signal
            .current_assignment_id
            .as_str(),
        "validator quality drift signal assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        validator_rollback_signal.current_assignment_id.as_str(),
        "validator rollback signal assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        checkpoint_pointer.assignment_id.as_deref().ok_or_else(|| {
            invalid(String::from(
                "checkpoint pointer is missing grouped assignment_id",
            ))
        })?,
        "checkpoint pointer assignment id",
    )?;
    require_eq(
        contribution_receipt.assignment_id.as_str(),
        checkpoint_manifest
            .assignment_id
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "checkpoint manifest is missing grouped assignment_id",
                ))
            })?,
        "checkpoint manifest assignment id",
    )?;

    require_eq(
        contribution_receipt.contribution_id.as_str(),
        grouped_stage_execution_summary.contribution_id.as_str(),
        "grouped stage execution summary contribution id",
    )?;
    require_eq(
        contribution_receipt.contribution_id.as_str(),
        grouped_stage_replay_evidence.contribution_id.as_str(),
        "grouped stage replay evidence contribution id",
    )?;
    require_eq(
        contribution_receipt.contribution_id.as_str(),
        validator_score_receipt.contribution_id.as_str(),
        "validator score receipt contribution id",
    )?;
    require_eq(
        contribution_receipt.contribution_digest.as_str(),
        grouped_stage_replay_evidence.contribution_digest.as_str(),
        "grouped stage replay evidence contribution digest",
    )?;
    require_eq(
        contribution_receipt.contribution_digest.as_str(),
        validator_score_receipt.contribution_digest.as_str(),
        "validator score receipt contribution digest",
    )?;
    require_eq(
        contribution_artifact_manifest
            .artifact_manifest_digest
            .as_str(),
        grouped_stage_replay_evidence
            .artifact_manifest_digest
            .as_str(),
        "grouped stage replay evidence artifact manifest digest",
    )?;
    require_eq(
        contribution_artifact_manifest
            .artifact_manifest_digest
            .as_str(),
        validator_score_receipt.artifact_manifest_digest.as_str(),
        "validator score receipt artifact manifest digest",
    )?;

    if validator_score_receipt.disposition != TrainingExecutionValidatorDisposition::Accepted
        || validator_score_artifact.disposition != TrainingExecutionValidatorDisposition::Accepted
        || validator_quality_drift_signal.current_disposition
            != TrainingExecutionValidatorDisposition::Accepted
    {
        return Err(invalid(String::from(
            "weak-device proof requires one accepted validator replay path",
        )));
    }
    if validator_score_receipt.score_bps != 10_000 || validator_score_artifact.score_bps != 10_000 {
        return Err(invalid(String::from(
            "weak-device proof requires one full-score validator acceptance",
        )));
    }
    if validator_rollback_signal.rollback_posture != PsionicTrainValidatorRollbackPosture::Hold {
        return Err(invalid(String::from(
            "weak-device proof requires rollback posture `hold`",
        )));
    }
    if !matches!(
        validator_quality_drift_signal.drift_state,
        PsionicTrainValidatorQualityDriftState::Baseline
            | PsionicTrainValidatorQualityDriftState::Stable
            | PsionicTrainValidatorQualityDriftState::Improved
    ) {
        return Err(invalid(String::from(
            "weak-device proof requires a non-regressed quality drift posture",
        )));
    }
    require_eq(
        validator_score_receipt.challenge_id.as_str(),
        grouped_stage_replay_evidence.challenge_id.as_str(),
        "grouped stage replay evidence challenge id",
    )?;
    require_eq(
        validator_score_receipt.challenge_id.as_str(),
        validator_quality_drift_signal.current_challenge_id.as_str(),
        "validator quality drift signal challenge id",
    )?;
    require_eq(
        validator_score_receipt.challenge_id.as_str(),
        validator_rollback_signal.current_challenge_id.as_str(),
        "validator rollback signal challenge id",
    )?;
    require_eq(
        validator_score_receipt.score_artifact_digest.as_str(),
        validator_score_artifact.score_digest.as_str(),
        "validator score artifact digest",
    )?;
    require_eq(
        grouped_stage_execution_summary
            .execution_summary_digest
            .as_str(),
        validator_score_receipt
            .grouped_stage_execution_summary_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score receipt is missing grouped stage execution summary digest",
                ))
            })?,
        "validator score receipt grouped stage execution summary digest",
    )?;
    require_eq(
        grouped_stage_execution_summary
            .execution_summary_digest
            .as_str(),
        validator_score_artifact
            .grouped_stage_execution_summary_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score artifact is missing grouped stage execution summary digest",
                ))
            })?,
        "validator score artifact grouped stage execution summary digest",
    )?;
    require_eq(
        grouped_stage_replay_evidence
            .replay_evidence_digest
            .as_str(),
        validator_score_receipt
            .grouped_stage_replay_evidence_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score receipt is missing grouped stage replay evidence digest",
                ))
            })?,
        "validator score receipt grouped stage replay evidence digest",
    )?;
    require_eq(
        grouped_stage_replay_evidence
            .replay_evidence_digest
            .as_str(),
        validator_score_artifact
            .grouped_stage_replay_evidence_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score artifact is missing grouped stage replay evidence digest",
                ))
            })?,
        "validator score artifact grouped stage replay evidence digest",
    )?;
    if validator_score_receipt.verified_hooks != validator_score_artifact.verified_hooks {
        return Err(invalid(String::from(
            "validator score receipt and artifact verified_hooks drifted",
        )));
    }

    let expected_hooks = normalized_hook_labels(&[
        PsionicTrainValidatorHook::AssignmentCorrectness,
        PsionicTrainValidatorHook::CheckpointLineage,
        PsionicTrainValidatorHook::WorkExecutionPlausibility,
        PsionicTrainValidatorHook::UpdateIntegrity,
        PsionicTrainValidatorHook::GroupedStageIntegrity,
    ]);
    let actual_hooks = normalized_hook_labels(&validator_score_receipt.verified_hooks);
    if actual_hooks != expected_hooks {
        return Err(invalid(String::from(
            "validator score receipt hooks drifted from the grouped-stage accepted hook set",
        )));
    }

    if checkpoint_surface.upload_outcome.as_deref() == Some("refused") {
        return Err(invalid(String::from(
            "checkpoint surface upload_outcome cannot be refused for the accepted weak-device proof",
        )));
    }
    require_eq(
        checkpoint_pointer.pointer_state.as_str(),
        checkpoint_surface
            .pointer_state
            .as_deref()
            .ok_or_else(|| invalid(String::from("checkpoint surface is missing pointer_state")))?,
        "checkpoint surface pointer_state",
    )?;
    require_eq(
        checkpoint_manifest.manifest_digest.as_str(),
        checkpoint_surface
            .checkpoint_manifest_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "checkpoint surface is missing checkpoint_manifest_digest",
                ))
            })?,
        "checkpoint surface manifest digest",
    )?;
    require_eq(
        checkpoint_manifest.checkpoint_object_digest.as_str(),
        checkpoint_surface
            .checkpoint_object_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "checkpoint surface is missing checkpoint_object_digest",
                ))
            })?,
        "checkpoint surface checkpoint object digest",
    )?;
    require_eq(
        checkpoint_manifest.manifest_digest.as_str(),
        validator_score_artifact
            .checkpoint_manifest_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score artifact is missing checkpoint manifest digest",
                ))
            })?,
        "validator score artifact checkpoint manifest digest",
    )?;
    require_eq(
        checkpoint_manifest.checkpoint_object_digest.as_str(),
        validator_score_artifact
            .checkpoint_object_digest
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score artifact is missing checkpoint object digest",
                ))
            })?,
        "validator score artifact checkpoint object digest",
    )?;
    require_eq(
        checkpoint_pointer.pointer_state.as_str(),
        validator_score_artifact
            .checkpoint_pointer_state
            .as_deref()
            .ok_or_else(|| {
                invalid(String::from(
                    "validator score artifact is missing checkpoint pointer state",
                ))
            })?,
        "validator score artifact checkpoint pointer state",
    )?;

    let manifest_checkpoint_surface = require_manifest_artifact(
        &contribution_artifact_manifest,
        "checkpoint_surface",
        checkpoint_surface_path,
    )?;
    let manifest_checkpoint_pointer = require_manifest_artifact(
        &contribution_artifact_manifest,
        "checkpoint_pointer",
        checkpoint_pointer_path,
    )?;
    require_manifest_artifact(
        &contribution_artifact_manifest,
        "checkpoint_manifest",
        checkpoint_manifest_path,
    )?;
    require_manifest_artifact(
        &contribution_artifact_manifest,
        "grouped_stage_execution_summary",
        grouped_stage_execution_summary_path,
    )?;
    let window_contribution = sealed_window_bundle
        .contributions
        .iter()
        .find(|contribution| contribution.contribution_id == contribution_receipt.contribution_id)
        .ok_or_else(|| {
            invalid(String::from(
                "sealed window bundle is missing the accepted contribution",
            ))
        })?
        .clone();
    require_eq(
        contribution_receipt.contribution_digest.as_str(),
        window_contribution.contribution_digest.as_str(),
        "sealed window bundle contribution digest",
    )?;
    require_eq(
        contribution_artifact_manifest
            .artifact_manifest_digest
            .as_str(),
        window_contribution.artifact_manifest_digest.as_str(),
        "sealed window bundle artifact manifest digest",
    )?;
    if window_contribution.grouped_stage_assignment.as_ref() != Some(&grouped_stage_assignment) {
        return Err(invalid(String::from(
            "sealed window bundle grouped stage assignment drifted from the contribution receipt",
        )));
    }

    let mut proof = PsionicTrainWeakDeviceAcceptedOutcomeProof {
        schema_version: String::from(
            PSIONIC_TRAIN_WEAK_DEVICE_ACCEPTED_OUTCOME_PROOF_SCHEMA_VERSION,
        ),
        proof_id: format!(
            "weak-device:{}:{}:{}",
            contributor_status.lane_id,
            contribution_receipt.window_id,
            contribution_receipt.contribution_id
        ),
        lane_id: contributor_status.lane_id.clone(),
        network_id,
        carrier: PsionicTrainWeakDeviceCarrier {
            contributor_run_id: contributor_run_id.clone(),
            contributor_node_pubkey: contribution_receipt.node_pubkey.clone(),
            backend_family: contributor_status
                .capability_projection
                .backend_family
                .clone(),
            topology_class: contributor_status
                .capability_projection
                .topology_class
                .clone(),
            work_class: contributor_status.work_class,
            grouped_stage_assignment: grouped_stage_assignment.clone(),
            weak_device_bearing: true,
            detail: String::from(
                "This proof binds one Apple/Metal grouped-replica stage to an accepted contribution instead of flattening weak-device work into a generic worker window.",
            ),
        },
        window: PsionicTrainWeakDeviceWindowProof {
            network_id: contributor_status.coordination.network_id.clone(),
            window_id: contribution_receipt.window_id.clone(),
            assignment_id: contribution_receipt.assignment_id.clone(),
            window_execution_id: window_execution.window_execution_id.clone(),
            window_digest: window_execution.window_digest.clone(),
            sealed_window_digest: sealed_window_bundle.sealed_window_digest.clone(),
            contribution_id: contribution_receipt.contribution_id.clone(),
            contribution_digest: contribution_receipt.contribution_digest.clone(),
            artifact_manifest_digest: contribution_artifact_manifest
                .artifact_manifest_digest
                .clone(),
            detail: String::from(
                "Window proof ties one grouped weak-device contribution to the retained window execution and sealed-window rollup.",
            ),
        },
        validator: PsionicTrainWeakDeviceValidatorProof {
            validator_run_id,
            challenge_id: validator_score_receipt.challenge_id.clone(),
            validation_index: validator_score_receipt.validation_index,
            validator_disposition: validator_score_receipt.disposition,
            validator_score_bps: validator_score_receipt.score_bps,
            verified_hooks: validator_score_receipt.verified_hooks.clone(),
            quality_drift_state: validator_quality_drift_signal.drift_state,
            rollback_posture: validator_rollback_signal.rollback_posture,
            detail: String::from(
                "Validator proof keeps the accepted replay, non-regressed quality signal, and hold posture together for the weak-device stage.",
            ),
        },
        checkpoint: PsionicTrainWeakDeviceCheckpointProof {
            checkpoint_label: checkpoint_manifest.checkpoint_label.clone(),
            checkpoint_ref: checkpoint_manifest.checkpoint_ref.clone(),
            checkpoint_pointer_state: checkpoint_pointer.pointer_state.clone(),
            optimizer_step: checkpoint_manifest.optimizer_step,
            checkpoint_manifest_digest: checkpoint_manifest.manifest_digest.clone(),
            checkpoint_object_digest: checkpoint_manifest.checkpoint_object_digest.clone(),
            detail: String::from(
                "Checkpoint proof preserves one accepted grouped-stage checkpoint family so the accepted contribution remains lineage-safe and replayable.",
            ),
        },
        cited_artifacts: vec![
            artifact_ref(
                "window_execution",
                window_execution_path,
                window_execution.window_digest.as_str(),
                "Retained per-window execution surface for the grouped weak-device contribution.",
            )?,
            artifact_ref(
                "contribution_receipt",
                contribution_receipt_path,
                contribution_receipt.contribution_digest.as_str(),
                "Accepted contribution receipt for the weak-device grouped stage.",
            )?,
            artifact_ref(
                "contribution_artifact_manifest",
                contribution_artifact_manifest_path,
                contribution_artifact_manifest
                    .artifact_manifest_digest
                    .as_str(),
                "Artifact manifest retained beside the accepted weak-device contribution.",
            )?,
            artifact_ref(
                "grouped_stage_execution_summary",
                grouped_stage_execution_summary_path,
                grouped_stage_execution_summary
                    .execution_summary_digest
                    .as_str(),
                "Grouped-stage execution summary proving the stage assignment and retained transport posture.",
            )?,
            artifact_ref(
                "grouped_stage_replay_evidence",
                grouped_stage_replay_evidence_path,
                grouped_stage_replay_evidence
                    .replay_evidence_digest
                    .as_str(),
                "Validator replay evidence over the grouped-stage summary and contribution digest.",
            )?,
            artifact_ref_with_digest(
                "checkpoint_surface",
                checkpoint_surface_path,
                manifest_checkpoint_surface
                    .binding
                    .artifact_ref
                    .artifact_digest
                    .as_deref()
                    .ok_or_else(|| {
                        invalid(String::from(
                            "checkpoint surface artifact binding is missing one digest",
                        ))
                    })?,
                "Checkpoint surface cited by the contribution manifest and inspected from the retained run root.",
            )?,
            artifact_ref_with_digest(
                "checkpoint_pointer",
                checkpoint_pointer_path,
                manifest_checkpoint_pointer
                    .binding
                    .artifact_ref
                    .artifact_digest
                    .as_deref()
                    .ok_or_else(|| {
                        invalid(String::from(
                            "checkpoint pointer artifact binding is missing one digest",
                        ))
                    })?,
                "Accepted grouped-stage checkpoint pointer retained under the run root.",
            )?,
            artifact_ref(
                "checkpoint_manifest",
                checkpoint_manifest_path,
                checkpoint_manifest.manifest_digest.as_str(),
                "Accepted grouped-stage checkpoint manifest retained under the run root.",
            )?,
            artifact_ref(
                "sealed_window_bundle",
                sealed_window_bundle_path,
                sealed_window_bundle.sealed_window_digest.as_str(),
                "Sealed-window rollup that includes the accepted weak-device contribution.",
            )?,
            artifact_ref(
                "validator_score_artifact",
                validator_score_receipt.score_artifact_path.as_str(),
                validator_score_artifact.score_digest.as_str(),
                "Validator score artifact binding hooks, checkpoint lineage, and grouped-stage replay evidence.",
            )?,
            artifact_ref(
                "validator_score_receipt",
                validator_score_receipt_path,
                validator_score_receipt.score_receipt_digest.as_str(),
                "Validator score receipt for the accepted grouped-stage contribution.",
            )?,
            artifact_ref(
                "validator_quality_drift_signal",
                validator_quality_drift_signal_path,
                validator_quality_drift_signal.drift_signal_digest.as_str(),
                "Quality drift signal proving the accepted weak-device contribution did not regress the validator posture.",
            )?,
            artifact_ref(
                "validator_rollback_signal",
                validator_rollback_signal_path,
                validator_rollback_signal.rollback_signal_digest.as_str(),
                "Rollback signal proving the accepted weak-device contribution remains in hold rather than rollback-candidate posture.",
            )?,
        ],
        claim_boundary: String::from(
            "This proof bundle cites one Apple/Metal grouped-replica stage contribution, one accepted validator replay path, and one accepted grouped-stage checkpoint family. It does not claim network-wide aggregate finality, payout closeout, or cross-stage completion beyond the retained Psionic surfaces cited here.",
        ),
        detail: String::from(
            "Weak-device accepted-outcome proof keeps the consumer-compute claim narrow and machine-readable: one weaker device carried one grouped-replica stage, the validator accepted it, quality stayed non-regressed, rollback stayed on hold, and checkpoint lineage remained scoped to the same grouped stage.",
        ),
        bundle_digest: String::new(),
    };
    proof.bundle_digest = proof.stable_bundle_digest();
    proof.validate()?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
                path: parent.display().to_string(),
                detail: error.to_string(),
            }
        })?;
    }
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&proof).map_err(|error| {
            PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
                path: output_path.display().to_string(),
                detail: error.to_string(),
            }
        })?,
    )
    .map_err(
        |error| PsionicTrainWeakDeviceAcceptedOutcomeProofError::Write {
            path: output_path.display().to_string(),
            detail: error.to_string(),
        },
    )?;
    Ok(proof)
}

fn artifact_ref(
    artifact_role: &str,
    artifact_path: &str,
    artifact_digest: &str,
    detail: &str,
) -> Result<
    PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef,
    PsionicTrainWeakDeviceAcceptedOutcomeProofError,
> {
    artifact_ref_with_digest(artifact_role, artifact_path, artifact_digest, detail)
}

fn artifact_ref_with_digest(
    artifact_role: &str,
    artifact_path: &str,
    artifact_digest: &str,
    detail: &str,
) -> Result<
    PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef,
    PsionicTrainWeakDeviceAcceptedOutcomeProofError,
> {
    require_nonempty(artifact_role, "artifact_ref.artifact_role")?;
    require_nonempty(artifact_path, "artifact_ref.artifact_path")?;
    require_nonempty(artifact_digest, "artifact_ref.artifact_digest")?;
    require_nonempty(detail, "artifact_ref.detail")?;
    Ok(PsionicTrainWeakDeviceAcceptedOutcomeArtifactRef {
        artifact_role: artifact_role.to_string(),
        artifact_path: artifact_path.to_string(),
        artifact_digest: artifact_digest.to_string(),
        detail: detail.to_string(),
    })
}

fn require_manifest_artifact(
    manifest: &PsionicTrainContributionArtifactManifest,
    artifact_kind: &str,
    expected_path: &str,
) -> Result<crate::PsionicTrainContributionArtifact, PsionicTrainWeakDeviceAcceptedOutcomeProofError>
{
    let artifact = manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == artifact_kind)
        .ok_or_else(|| {
            invalid(format!(
            "contribution artifact manifest is missing required artifact kind `{artifact_kind}`",
        ))
        })?;
    require_eq(
        artifact
            .binding
            .materialized_path
            .as_deref()
            .ok_or_else(|| {
                invalid(format!(
                    "artifact manifest kind `{artifact_kind}` is missing one materialized path",
                ))
            })?,
        expected_path,
        format!("artifact manifest path for `{artifact_kind}`").as_str(),
    )?;
    Ok(artifact.clone())
}

fn required_path<'a>(
    path: Option<&'a str>,
    field: &str,
) -> Result<&'a str, PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
    let path = path.ok_or_else(|| {
        invalid(format!(
            "required retained artifact path `{field}` is missing"
        ))
    })?;
    require_nonempty(path, field)?;
    Ok(path)
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| PsionicTrainWeakDeviceAcceptedOutcomeProofError::Read {
                path: path.display().to_string(),
                detail: error.to_string(),
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        PsionicTrainWeakDeviceAcceptedOutcomeProofError::Parse {
            path: path.display().to_string(),
            detail: error.to_string(),
        }
    })
}

fn require_nonempty(
    value: &str,
    field: &str,
) -> Result<(), PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
    if value.trim().is_empty() {
        return Err(invalid(format!("field `{field}` must not be empty")));
    }
    Ok(())
}

fn require_eq(
    expected: &str,
    actual: &str,
    field: &str,
) -> Result<(), PsionicTrainWeakDeviceAcceptedOutcomeProofError> {
    if expected != actual {
        return Err(invalid(format!(
            "field `{field}` mismatch: expected `{expected}`, got `{actual}`",
        )));
    }
    Ok(())
}

fn invalid(detail: String) -> PsionicTrainWeakDeviceAcceptedOutcomeProofError {
    PsionicTrainWeakDeviceAcceptedOutcomeProofError::Invalid { detail }
}

fn stable_window_execution_digest(value: &PsionicTrainWindowExecution) -> String {
    let mut digest_basis = value.clone();
    digest_basis.window_digest.clear();
    stable_digest(b"psionic_train_window_execution|", &digest_basis)
}

fn stable_sealed_window_bundle_digest(value: &PsionicTrainSealedWindowBundle) -> String {
    let mut digest_basis = value.clone();
    digest_basis.sealed_window_digest.clear();
    stable_digest(b"psionic_train_sealed_window_bundle|", &digest_basis)
}

fn stable_validator_score_artifact_digest(value: &PsionicTrainValidatorScoreArtifact) -> String {
    let mut digest_basis = value.clone();
    digest_basis.score_digest.clear();
    stable_digest(b"psionic_train_validator_score_artifact|", &digest_basis)
}

fn stable_validator_score_receipt_digest(value: &PsionicTrainValidatorScoreReceipt) -> String {
    let mut digest_basis = value.clone();
    digest_basis.score_receipt_digest.clear();
    stable_digest(b"psionic_train_validator_score_receipt|", &digest_basis)
}

fn stable_validator_quality_drift_signal_digest(
    value: &PsionicTrainValidatorQualityDriftSignal,
) -> String {
    let mut digest_basis = value.clone();
    digest_basis.drift_signal_digest.clear();
    stable_digest(
        b"psionic_train_validator_quality_drift_signal|",
        &digest_basis,
    )
}

fn stable_validator_rollback_signal_digest(value: &PsionicTrainValidatorRollbackSignal) -> String {
    let mut digest_basis = value.clone();
    digest_basis.rollback_signal_digest.clear();
    stable_digest(b"psionic_train_validator_rollback_signal|", &digest_basis)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let bytes = serde_json::to_vec(value)
        .expect("weak-device accepted-outcome proof payload should serialize");
    let mut digest = Sha256::new();
    digest.update(prefix);
    digest.update(&bytes);
    format!("{:x}", digest.finalize())
}

fn normalized_hook_labels(hooks: &[PsionicTrainValidatorHook]) -> Vec<&'static str> {
    let mut labels = hooks
        .iter()
        .map(|hook| match hook {
            PsionicTrainValidatorHook::AssignmentCorrectness => "assignment_correctness",
            PsionicTrainValidatorHook::CheckpointLineage => "checkpoint_lineage",
            PsionicTrainValidatorHook::WorkExecutionPlausibility => "work_execution_plausibility",
            PsionicTrainValidatorHook::UpdateIntegrity => "update_integrity",
            PsionicTrainValidatorHook::GroupedStageIntegrity => "grouped_stage_integrity",
        })
        .collect::<Vec<_>>();
    labels.sort_unstable();
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_assignment() -> PsionicTrainGroupedReplicaStageAssignment {
        PsionicTrainGroupedReplicaStageAssignment::new(
            "replica-apple-01",
            "stage-01",
            0,
            2,
            crate::PsionicTrainGroupedReplicaStageRole::Ingress,
            None,
            Some(String::from("stage-02")),
        )
        .expect("sample grouped assignment should build")
    }

    fn sample_proof() -> PsionicTrainWeakDeviceAcceptedOutcomeProof {
        let mut proof = PsionicTrainWeakDeviceAcceptedOutcomeProof {
            schema_version: String::from(
                PSIONIC_TRAIN_WEAK_DEVICE_ACCEPTED_OUTCOME_PROOF_SCHEMA_VERSION,
            ),
            proof_id: String::from("weak-device:lane:window:contribution"),
            lane_id: String::from("psion_apple_windowed_training_v1"),
            network_id: Some(String::from("network.psionic.test")),
            carrier: PsionicTrainWeakDeviceCarrier {
                contributor_run_id: String::from("run-apple-0001"),
                contributor_node_pubkey: String::from("npub1-apple"),
                backend_family: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY),
                topology_class: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS),
                work_class: PsionicTrainWorkClass::GroupedReplicaStageExecution,
                grouped_stage_assignment: sample_assignment(),
                weak_device_bearing: true,
                detail: String::from("sample"),
            },
            window: PsionicTrainWeakDeviceWindowProof {
                network_id: Some(String::from("network.psionic.test")),
                window_id: String::from("window-0001"),
                assignment_id: String::from("assignment-0001"),
                window_execution_id: String::from("window-exec-0001"),
                window_digest: String::from("sha256:window"),
                sealed_window_digest: String::from("sha256:sealed"),
                contribution_id: String::from("contribution-0001"),
                contribution_digest: String::from("sha256:contribution"),
                artifact_manifest_digest: String::from("sha256:artifact-manifest"),
                detail: String::from("sample"),
            },
            validator: PsionicTrainWeakDeviceValidatorProof {
                validator_run_id: String::from("validator-run-0001"),
                challenge_id: String::from("challenge-0001"),
                validation_index: 1,
                validator_disposition: TrainingExecutionValidatorDisposition::Accepted,
                validator_score_bps: 10_000,
                verified_hooks: vec![
                    PsionicTrainValidatorHook::AssignmentCorrectness,
                    PsionicTrainValidatorHook::CheckpointLineage,
                    PsionicTrainValidatorHook::WorkExecutionPlausibility,
                    PsionicTrainValidatorHook::UpdateIntegrity,
                    PsionicTrainValidatorHook::GroupedStageIntegrity,
                ],
                quality_drift_state: PsionicTrainValidatorQualityDriftState::Baseline,
                rollback_posture: PsionicTrainValidatorRollbackPosture::Hold,
                detail: String::from("sample"),
            },
            checkpoint: PsionicTrainWeakDeviceCheckpointProof {
                checkpoint_label: String::from("accepted"),
                checkpoint_ref: String::from("checkpoint://accepted"),
                checkpoint_pointer_state: String::from("accepted"),
                optimizer_step: 4096,
                checkpoint_manifest_digest: String::from("sha256:checkpoint-manifest"),
                checkpoint_object_digest: String::from("sha256:checkpoint-object"),
                detail: String::from("sample"),
            },
            cited_artifacts: vec![
                artifact_ref(
                    "window_execution",
                    "/tmp/window_execution.json",
                    "sha256:1",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "contribution_receipt",
                    "/tmp/contribution_receipt.json",
                    "sha256:2",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "contribution_artifact_manifest",
                    "/tmp/artifact_manifest.json",
                    "sha256:3",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "grouped_stage_execution_summary",
                    "/tmp/grouped_stage_execution_summary.json",
                    "sha256:4",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "grouped_stage_replay_evidence",
                    "/tmp/grouped_stage_replay_evidence.json",
                    "sha256:5",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "checkpoint_surface",
                    "/tmp/checkpoint_surface.json",
                    "sha256:6",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "checkpoint_pointer",
                    "/tmp/checkpoint_pointer.json",
                    "sha256:7",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "checkpoint_manifest",
                    "/tmp/checkpoint_manifest.json",
                    "sha256:8",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "sealed_window_bundle",
                    "/tmp/sealed_window_bundle.json",
                    "sha256:9",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_score_artifact",
                    "/tmp/validator_score_artifact.json",
                    "sha256:10",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_score_receipt",
                    "/tmp/validator_score_receipt.json",
                    "sha256:11",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_quality_drift_signal",
                    "/tmp/validator_quality_drift_signal.json",
                    "sha256:12",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_rollback_signal",
                    "/tmp/validator_rollback_signal.json",
                    "sha256:13",
                    "sample",
                )
                .expect("artifact ref should build"),
            ],
            claim_boundary: String::from("sample"),
            detail: String::from("sample"),
            bundle_digest: String::new(),
        };
        proof.bundle_digest = proof.stable_bundle_digest();
        proof
    }

    fn sample_validation_replay_proof() -> PsionicTrainWeakDeviceValidationReplayProof {
        let mut proof = PsionicTrainWeakDeviceValidationReplayProof {
            schema_version: String::from(
                PSIONIC_TRAIN_WEAK_DEVICE_VALIDATION_REPLAY_PROOF_SCHEMA_VERSION,
            ),
            proof_id: String::from("weak-device-validation-replay:lane:window:challenge"),
            lane_id: String::from(PSION_APPLE_WINDOWED_TRAINING_LANE_ID),
            network_id: Some(String::from("network.psionic.test")),
            validator_run_id: String::from("validator-run-0001"),
            validator_node_pubkey: String::from("npub1-validator"),
            backend_family: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY),
            topology_class: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS),
            weak_device_bearing: true,
            challenged_run_id: String::from("worker-run-0001"),
            challenged_node_pubkey: String::from("npub1-worker"),
            challenged_work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            window_id: String::from("window-0001"),
            assignment_id: String::from("assignment-0001"),
            challenge_id: String::from("challenge-0001"),
            contribution_id: String::from("contribution-0001"),
            contribution_digest: String::from("sha256:contribution"),
            artifact_manifest_digest: String::from("sha256:artifact-manifest"),
            public_count_class:
                PsionicTrainWeakDevicePublicCountClass::ValidatorRecognizedParticipation,
            validator_disposition: TrainingExecutionValidatorDisposition::Accepted,
            validator_score_bps: 10_000,
            verified_hooks: vec![
                PsionicTrainValidatorHook::AssignmentCorrectness,
                PsionicTrainValidatorHook::CheckpointLineage,
            ],
            quality_drift_state: PsionicTrainValidatorQualityDriftState::Baseline,
            rollback_posture: PsionicTrainValidatorRollbackPosture::Hold,
            cited_artifacts: vec![
                artifact_ref(
                    "contribution_receipt",
                    "/tmp/contribution_receipt.json",
                    "sha256:1",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "contribution_artifact_manifest",
                    "/tmp/artifact_manifest.json",
                    "sha256:2",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_score_artifact",
                    "/tmp/validator_score_artifact.json",
                    "sha256:3",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_score_receipt",
                    "/tmp/validator_score_receipt.json",
                    "sha256:4",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_quality_drift_signal",
                    "/tmp/validator_quality_drift_signal.json",
                    "sha256:5",
                    "sample",
                )
                .expect("artifact ref should build"),
                artifact_ref(
                    "validator_rollback_signal",
                    "/tmp/validator_rollback_signal.json",
                    "sha256:6",
                    "sample",
                )
                .expect("artifact ref should build"),
            ],
            claim_boundary: String::from("sample"),
            detail: String::from("sample"),
            bundle_digest: String::new(),
        };
        proof.bundle_digest = proof.stable_bundle_digest();
        proof
    }

    #[test]
    fn weak_device_accepted_outcome_proof_validates() {
        sample_proof()
            .validate()
            .expect("sample proof should validate");
    }

    #[test]
    fn weak_device_accepted_outcome_proof_rejects_non_accepted_disposition() {
        let mut proof = sample_proof();
        proof.validator.validator_disposition =
            TrainingExecutionValidatorDisposition::ReplayRequired;
        proof.bundle_digest = proof.stable_bundle_digest();
        let error = proof
            .validate()
            .expect_err("non-accepted proof should fail");
        assert!(error.to_string().contains("validator disposition"));
    }

    #[test]
    fn weak_device_validation_replay_proof_validates() {
        sample_validation_replay_proof()
            .validate()
            .expect("sample validation replay proof should validate");
    }

    #[test]
    fn weak_device_validation_replay_proof_rejects_non_accepted_disposition() {
        let mut proof = sample_validation_replay_proof();
        proof.validator_disposition = TrainingExecutionValidatorDisposition::ReplayRequired;
        proof.bundle_digest = proof.stable_bundle_digest();
        let error = proof
            .validate()
            .expect_err("non-accepted validation replay proof should fail");
        assert!(error.to_string().contains("accepted validator disposition"));
    }
}
