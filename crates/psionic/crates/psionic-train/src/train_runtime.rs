use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PsionicTrainGroupedReplicaStageAssignment;
use crate::{
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_CS336_A1_DEMO_LANE_ID,
    QWEN_LEGAL_ADAPTER_SFT_LANE_ID,
};

/// Stable admitted lane id for the first Apple-homogeneous machine training lane.
pub const PSION_APPLE_WINDOWED_TRAINING_LANE_ID: &str = "psion_apple_windowed_training_v1";

/// Stable schema version for the machine-consumable `psionic-train` invocation manifest.
pub const PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION: &str =
    "psionic.train.invocation_manifest.v1";

/// Stable schema version for the machine-consumable `psionic-train` status packet.
pub const PSIONIC_TRAIN_STATUS_PACKET_SCHEMA_VERSION: &str = "psionic.train.status_packet.v1";

/// Stable schema version for one retained runtime attestation packet.
pub const PSIONIC_TRAIN_RUNTIME_ATTESTATION_SCHEMA_VERSION: &str =
    "psionic.train.runtime_attestation.v1";

/// Stable schema version for the machine-readable run-status packet.
pub const PSIONIC_TRAIN_RUN_STATUS_PACKET_SCHEMA_VERSION: &str =
    "psionic.train.run_status_packet.v1";

/// Stable schema version for the machine-readable window-status packet.
pub const PSIONIC_TRAIN_WINDOW_STATUS_PACKET_SCHEMA_VERSION: &str =
    "psionic.train.window_status_packet.v1";

/// Stable runtime surface identifier for the first machine-consumable `psionic-train` CLI.
pub const PSIONIC_TRAIN_RUNTIME_SURFACE_ID: &str = "psionic-train.runtime.v1";

/// Canonical local cache directory where resolver-backed artifacts are materialized for runtime use.
pub const PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR: &str = "artifacts/resolved";

/// Environment variable pointing a packaged machine runtime at its runtime root.
pub const PSIONIC_TRAIN_RUNTIME_ROOT_ENV: &str = "OPENAGENTS_PSIONIC_REPO";

/// Revision marker included in packaged machine runtimes that are not Git checkouts.
pub const PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE: &str = ".openagents-psionic-revision";

/// Synthetic selected ref used when a packaged runtime is identified by its revision marker.
pub const PSIONIC_TRAIN_PACKAGED_RUNTIME_SELECTED_REF: &str = "packaged-runtime";

/// Dirty-tree posture recorded for packaged runtimes built from clean source.
pub const PSIONIC_TRAIN_PACKAGED_RUNTIME_DIRTY_TREE_ADMISSION: &str = "packaged_clean_release";

/// Stable admitted release id for the actual pretraining lane on the first machine runtime.
pub const PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID: &str =
    "psionic-train.psion_actual_pretraining.release.v1";

/// Stable admitted environment ref for the actual pretraining lane on the first machine runtime.
pub const PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_actual_pretraining.cuda_h100.operator@v1";

/// Stable admitted release id for the first Apple machine lane on the runtime surface.
pub const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID: &str =
    "psionic-train.psion_apple_windowed_training.release.v1";

/// Stable admitted release id for the bounded packaged CS336 A1 demo lane.
pub const PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID: &str =
    "psionic-train.psion_cs336_a1_demo.release.v1";

/// Stable admitted release id for the Qwen legal adapter smoke lane.
pub const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID: &str =
    "psionic-train.qwen_legal_adapter_sft.release.v1";

/// Stable admitted environment ref for the first Apple machine lane on the runtime surface.
pub const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_apple_windowed_training.metal_mlx.operator@v1";

/// Stable admitted environment ref for the bounded packaged CS336 A1 demo lane.
pub const PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1";

/// Stable admitted environment ref for the Qwen legal adapter smoke lane.
pub const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF: &str =
    "psionic.environment.qwen_legal_adapter_sft.cuda.operator@v1";

/// Stable backend family projected by the first admitted actual-lane runtime.
pub const PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY: &str = "cuda";

/// Stable topology class projected by the first admitted actual-lane runtime.
pub const PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS: &str =
    "homogeneous_four_node_h100_tensor_parallel";

/// Stable backend family projected by the first admitted Apple machine runtime.
pub const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY: &str = "metal";

/// Stable topology class projected by the first admitted Apple machine runtime.
pub const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS: &str =
    "homogeneous_apple_silicon_data_parallel";

/// Stable backend family projected by the packaged CS336 A1 demo runtime.
pub const PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY: &str = "cpu";

/// Stable topology class projected by the packaged CS336 A1 demo runtime.
pub const PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS: &str = "single_host_cpu_reference";

/// Stable backend family projected by the Qwen legal adapter smoke runtime.
pub const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_BACKEND_FAMILY: &str = "cuda";

/// Stable topology class projected by the Qwen legal adapter smoke runtime.
pub const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_TOPOLOGY_CLASS: &str =
    "single_host_cuda_adapter_smoke";

/// One stable machine role consumed by `psionic-train`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainRole {
    Worker,
    Validator,
    RecoverySource,
}

/// One stable machine operation consumed by `psionic-train`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainOperation {
    Start,
    Resume,
    ServeCheckpoint,
    ValidateContribution,
    RecordCheckpoint,
    Backup,
    DecideContinueRestart,
    RehearseBaseLane,
}

/// One stable work class consumed by `psionic-train`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainWorkClass {
    ValidationReplay,
    Evaluation,
    AdapterTraining,
    SmallModelLocalTraining,
    GroupedReplicaStageExecution,
    FullIslandLocalUpdateTraining,
    Aggregation,
    CheckpointPromotion,
}

impl PsionicTrainWorkClass {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ValidationReplay => "validation_replay",
            Self::Evaluation => "evaluation",
            Self::AdapterTraining => "adapter_training",
            Self::SmallModelLocalTraining => "small_model_local_training",
            Self::GroupedReplicaStageExecution => "grouped_replica_stage_execution",
            Self::FullIslandLocalUpdateTraining => "full_island_local_update_training",
            Self::Aggregation => "aggregation",
            Self::CheckpointPromotion => "checkpoint_promotion",
        }
    }

    #[must_use]
    pub const fn is_validator_target_admitted(self) -> bool {
        matches!(
            self,
            Self::AdapterTraining
                | Self::SmallModelLocalTraining
                | Self::GroupedReplicaStageExecution
                | Self::FullIslandLocalUpdateTraining
        )
    }
}

/// Stable outcome class for the machine-readable status packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainOutcomeKind {
    Succeeded,
    Refused,
}

/// Shared refusal taxonomy for the machine-facing `psionic-train` process boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainRefusalClass {
    BadConfig,
    StaleAssignment,
    LeaseExpired,
    UnsupportedTopology,
    GroupedStageAssignmentInvalid,
    CheckpointMissing,
    CheckpointDigestMismatch,
    ArtifactIncomplete,
    ArtifactDigestMismatch,
    ValidatorTimeout,
    ValidatorDisagreement,
    EnvironmentMismatch,
    BuildRevoked,
    InternalError,
}

/// Authority that owns the next durable transition after the refusal leaves the local process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainAuthorityOwner {
    Pylon,
    Nexus,
}

/// Shared coordination envelope frozen for telemetry, receipts, and machine status packets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCoordinationContext {
    /// Stable network identifier when the caller binds the run to one network.
    pub network_id: Option<String>,
    /// Stable window identifier when the caller binds the run to one window.
    pub window_id: Option<String>,
    /// Stable assignment identifier when the caller binds the run to one assignment.
    pub assignment_id: Option<String>,
    /// Stable challenge identifier when the caller binds the run to one validator challenge.
    pub challenge_id: Option<String>,
    /// Stable node pubkey when the caller binds the run to one admitted node identity.
    pub node_pubkey: Option<String>,
    /// Stable membership revision when one distributed membership revision already exists.
    pub membership_revision: Option<u64>,
}

/// Admitted identity that the machine caller expects the runtime to satisfy before launch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainAdmissionIdentity {
    /// Stable admitted release id.
    pub release_id: String,
    /// Stable admitted build digest.
    pub build_digest: String,
    /// Stable admitted environment ref.
    pub environment_ref: String,
}

/// Runtime attestation the machine process emits after resolving the local executable context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainRuntimeAttestation {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable admitted release id.
    pub release_id: String,
    /// Stable resolved build digest.
    pub build_digest: String,
    /// Stable resolved git commit SHA.
    pub git_commit_sha: String,
    /// Stable dirty-tree posture used to derive the build identity.
    pub dirty_tree_admission: String,
    /// Optional status digest when dirty-tree override is active.
    pub workspace_status_sha256: Option<String>,
    /// Stable admitted environment ref.
    pub environment_ref: String,
}

/// Minimal capability projection frozen for the first machine runtime lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainCapabilityProjection {
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable admitted role.
    pub role: PsionicTrainRole,
    /// Stable backend family.
    pub backend_family: String,
    /// Stable topology class.
    pub topology_class: String,
    /// Stable admitted environment ref.
    pub environment_ref: String,
}

/// Minimum admitted machine class for one canonical machine-runtime lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainMinimumMachineClass {
    ReferenceHostCpuOperator,
    CrossPlatformCpuCompatibleOperator,
    AppleSiliconOperator,
    StrongCudaTrainer,
}

impl PsionicTrainMinimumMachineClass {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReferenceHostCpuOperator => "reference_host_cpu_operator",
            Self::CrossPlatformCpuCompatibleOperator => "cross_platform_cpu_compatible_operator",
            Self::AppleSiliconOperator => "apple_silicon_operator",
            Self::StrongCudaTrainer => "strong_cuda_trainer",
        }
    }
}

/// Canonical machine-runtime contract for one admitted lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainLaneContract {
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable admitted release id.
    pub release_id: String,
    /// Stable admitted environment ref.
    pub environment_ref: String,
    /// Stable backend family.
    pub backend_family: String,
    /// Stable topology class.
    pub topology_class: String,
    /// Minimum machine class admitted for the lane.
    pub minimum_machine_class: PsionicTrainMinimumMachineClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PsionicTrainLaneContractStatic {
    lane_id: &'static str,
    release_id: &'static str,
    environment_ref: &'static str,
    backend_family: &'static str,
    topology_class: &'static str,
    minimum_machine_class: PsionicTrainMinimumMachineClass,
}

/// Logical identity for one retained training artifact independent of any one host path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainArtifactRef {
    /// Stable artifact identifier suitable for resolver-backed rematerialization.
    pub artifact_id: String,
    /// Optional canonical artifact digest when the producer already knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    /// Optional byte count when the producer already knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_bytes: Option<u64>,
}

/// One logical artifact reference plus an optional machine-local materialization path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainArtifactBinding {
    /// Canonical logical reference for the artifact family.
    pub artifact_ref: PsionicTrainArtifactRef,
    /// Optional machine-local materialization path used by the current process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized_path: Option<String>,
}

impl PsionicTrainArtifactRef {
    pub fn validate(&self, field: &str) -> Result<(), String> {
        if self.artifact_id.trim().is_empty() {
            return Err(format!("{field}.artifact_id must not be empty"));
        }
        if let Some(artifact_digest) = self.artifact_digest.as_deref() {
            if artifact_digest.trim().is_empty() {
                return Err(format!("{field}.artifact_digest must not be empty"));
            }
        }
        if matches!(self.artifact_bytes, Some(0)) {
            return Err(format!("{field}.artifact_bytes must be non-zero"));
        }
        Ok(())
    }
}

impl PsionicTrainArtifactBinding {
    pub fn validate(&self, field: &str) -> Result<(), String> {
        self.artifact_ref
            .validate(format!("{field}.artifact_ref").as_str())?;
        if let Some(materialized_path) = self.materialized_path.as_deref() {
            if materialized_path.trim().is_empty() {
                return Err(format!("{field}.materialized_path must not be empty"));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn canonicalize_for_digest(&self) -> Self {
        let mut clone = self.clone();
        clone.materialized_path = None;
        clone
    }

    pub fn require_materialized_path(&self, field: &str) -> Result<&str, String> {
        let materialized_path = self
            .materialized_path
            .as_deref()
            .ok_or_else(|| format!("{field}.materialized_path must not be empty"))?;
        if materialized_path.trim().is_empty() {
            return Err(format!("{field}.materialized_path must not be empty"));
        }
        Ok(materialized_path)
    }
}

/// One shared set of retained artifact refs carried by run-status and window-status packets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainArtifactSurfaceRefs {
    /// Optional retained launch-manifest path.
    pub launch_manifest_path: Option<String>,
    /// Optional retained membership-revision receipt path.
    pub membership_revision_path: Option<String>,
    /// Optional retained runtime-window execution path.
    pub window_execution_path: Option<String>,
    /// Optional retained contribution receipt path.
    pub contribution_receipt_path: Option<String>,
    /// Optional retained contribution artifact-manifest path.
    pub contribution_artifact_manifest_path: Option<String>,
    /// Optional retained grouped-stage input transport path.
    pub grouped_stage_input_transport_path: Option<String>,
    /// Optional retained grouped-stage output transport path.
    pub grouped_stage_output_transport_path: Option<String>,
    /// Optional retained grouped-stage output payload path.
    pub grouped_stage_output_payload_path: Option<String>,
    /// Optional retained grouped-stage execution summary path.
    pub grouped_stage_execution_summary_path: Option<String>,
    /// Optional retained grouped-stage replay evidence path.
    pub grouped_stage_replay_evidence_path: Option<String>,
    /// Optional retained checkpoint-surface path.
    pub checkpoint_surface_path: Option<String>,
    /// Optional retained checkpoint-pointer path.
    pub checkpoint_pointer_path: Option<String>,
    /// Optional retained checkpoint-manifest path.
    pub checkpoint_manifest_path: Option<String>,
    /// Optional retained checkpoint-backup receipt path.
    pub checkpoint_backup_receipt_path: Option<String>,
    /// Optional retained peer checkpoint-handoff receipt path.
    pub checkpoint_handoff_receipt_path: Option<String>,
    /// Optional retained recovery receipt path.
    pub recovery_receipt_path: Option<String>,
    /// Optional retained validator score receipt path.
    pub validator_score_receipt_path: Option<String>,
    /// Optional retained validator quality-drift signal path.
    pub validator_quality_drift_signal_path: Option<String>,
    /// Optional retained validator rollback-signal path.
    pub validator_rollback_signal_path: Option<String>,
    /// Optional retained weak-device validation-replay proof path.
    pub weak_device_validation_replay_proof_path: Option<String>,
    /// Optional retained sealed-window bundle path.
    pub sealed_window_bundle_path: Option<String>,
    /// Optional retained final closeout bundle path.
    pub final_closeout_bundle_path: Option<String>,
}

/// Stable machine-consumable invocation manifest for `psionic-train`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainInvocationManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runtime surface id.
    pub runtime_surface_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable runtime work class.
    pub work_class: PsionicTrainWorkClass,
    /// Shared coordination envelope for the run when the caller already knows those ids.
    #[serde(default)]
    pub coordination: PsionicTrainCoordinationContext,
    /// Optional grouped-replica stage assignment when one worker participates in a staged replica.
    #[serde(default)]
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Admitted release, build, and environment identity expected before launch.
    pub admission_identity: PsionicTrainAdmissionIdentity,
    /// Stable run identifier when the machine caller wants deterministic run roots.
    pub run_id: Option<String>,
    /// Explicit output root for launch-style commands.
    pub output_root: Option<String>,
    /// Explicit run root for commands that operate on retained state.
    pub run_root: Option<String>,
    /// Optional admitted peer node pubkey for recovery-source checkpoint serving.
    pub peer_node_pubkey: Option<String>,
    /// Optional retained peer checkpoint-handoff receipt consumed before resume.
    #[serde(default)]
    pub peer_checkpoint_handoff_receipt: Option<PsionicTrainArtifactBinding>,
    /// Optional challenged contribution receipt consumed by validator replay.
    #[serde(default)]
    pub validator_target_contribution_receipt: Option<PsionicTrainArtifactBinding>,
    /// Optional challenged contribution artifact manifest consumed by validator replay.
    #[serde(default)]
    pub validator_target_contribution_artifact_manifest: Option<PsionicTrainArtifactBinding>,
    /// Optional challenged work class consumed by validator replay.
    pub validator_target_work_class: Option<PsionicTrainWorkClass>,
    /// Optional retained grouped-stage input transport envelope for non-ingress grouped replicas.
    #[serde(default)]
    pub grouped_stage_input_transport: Option<PsionicTrainArtifactBinding>,
    /// Explicit git ref selection. Defaults are not allowed for machine mode.
    pub selected_git_ref: Option<String>,
    /// Optional retained hardware observation path.
    pub hardware_observation_path: Option<String>,
    /// Optional retained run-shape observation path.
    pub run_shape_observation_path: Option<String>,
    /// Whether dirty-tree override is enabled.
    pub allow_dirty_tree: bool,
    /// Whether the execution remains in dry-run posture.
    pub dry_run: bool,
    /// Optional checkpoint label for record-checkpoint.
    pub checkpoint_label: Option<String>,
    /// Optional optimizer step for record-checkpoint.
    pub optimizer_step: Option<u64>,
    /// Optional checkpoint ref for record-checkpoint.
    pub checkpoint_ref: Option<String>,
    /// Optional checkpoint object digest override.
    pub checkpoint_object_digest: Option<String>,
    /// Optional checkpoint object size override.
    pub checkpoint_total_bytes: Option<u64>,
    /// Optional injected failed-upload drill.
    pub inject_failed_upload: bool,
    /// Optional injected eval-worker-unavailable drill.
    pub inject_eval_worker_unavailable: bool,
    /// Canonical digest with this field omitted from the digest basis.
    pub manifest_digest: Option<String>,
}

/// Stable machine-readable status packet emitted by `psionic-train`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainStatusPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runtime surface id.
    pub runtime_surface_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable runtime work class.
    pub work_class: PsionicTrainWorkClass,
    /// Stable outcome kind.
    pub outcome: PsionicTrainOutcomeKind,
    /// Stable numeric exit code.
    pub exit_code: u8,
    /// Retryability bit for supervisors.
    pub retryable: bool,
    /// Authority that owns the next durable transition.
    pub authority_owner: PsionicTrainAuthorityOwner,
    /// Optional refusal class when the run is refused.
    pub refusal_class: Option<PsionicTrainRefusalClass>,
    /// Optional invocation manifest path.
    pub manifest_path: Option<String>,
    /// Optional invocation manifest digest.
    pub manifest_digest: Option<String>,
    /// Shared coordination envelope.
    pub coordination: PsionicTrainCoordinationContext,
    /// Optional grouped-replica stage assignment projected by the admitted manifest.
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Optional challenged work class for validator replay.
    pub validator_target_work_class: Option<PsionicTrainWorkClass>,
    /// Runtime attestation resolved by the local machine before or during execution.
    pub runtime_attestation: Option<PsionicTrainRuntimeAttestation>,
    /// Minimal runtime capability projection.
    pub capability_projection: Option<PsionicTrainCapabilityProjection>,
    /// Optional run identifier.
    pub run_id: Option<String>,
    /// Optional run root.
    pub run_root: Option<String>,
    /// Optional retained run-status packet path.
    pub run_status_packet_path: Option<String>,
    /// Optional retained window-status packet path.
    pub window_status_packet_path: Option<String>,
    /// Optional current-status path for actual-lane runs.
    pub current_status_path: Option<String>,
    /// Optional retained-summary path for actual-lane runs.
    pub retained_summary_path: Option<String>,
    /// Optional latest-checkpoint-pointer path for actual-lane runs.
    pub latest_checkpoint_pointer_path: Option<String>,
    /// Optional launcher log path for actual-lane runs.
    pub launcher_log_path: Option<String>,
    /// Short human-readable detail for logs and operator inspection.
    pub detail: String,
}

/// Stable machine-readable run-status packet for `Pylon`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainRunStatusPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runtime surface id.
    pub runtime_surface_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable runtime work class.
    pub work_class: PsionicTrainWorkClass,
    /// Stable outcome kind.
    pub outcome: PsionicTrainOutcomeKind,
    /// Stable numeric exit code.
    pub exit_code: u8,
    /// Retryability bit for supervisors.
    pub retryable: bool,
    /// Authority that owns the next durable transition.
    pub authority_owner: PsionicTrainAuthorityOwner,
    /// Optional refusal class when the run is refused.
    pub refusal_class: Option<PsionicTrainRefusalClass>,
    /// Shared coordination envelope.
    pub coordination: PsionicTrainCoordinationContext,
    /// Optional grouped-replica stage assignment projected by the admitted manifest.
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Optional challenged work class for validator replay.
    pub validator_target_work_class: Option<PsionicTrainWorkClass>,
    /// Optional invocation manifest path.
    pub manifest_path: Option<String>,
    /// Optional invocation manifest digest.
    pub manifest_digest: Option<String>,
    /// Optional run identifier.
    pub run_id: Option<String>,
    /// Optional run root.
    pub run_root: Option<String>,
    /// Optional retained phase for the live run.
    pub phase: Option<String>,
    /// Resolved runtime attestation.
    pub runtime_attestation: PsionicTrainRuntimeAttestation,
    /// Minimal runtime capability projection.
    pub capability_projection: PsionicTrainCapabilityProjection,
    /// Shared retained artifact refs.
    pub artifacts: PsionicTrainArtifactSurfaceRefs,
    /// Optional current-status path for actual-lane runs.
    pub current_status_path: Option<String>,
    /// Optional retained-summary path for actual-lane runs.
    pub retained_summary_path: Option<String>,
    /// Optional launcher log path for actual-lane runs.
    pub launcher_log_path: Option<String>,
    /// Short human-readable detail for logs and operator inspection.
    pub detail: String,
}

/// Stable machine-readable window-status packet for `Nexus`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainWindowStatusPacket {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable runtime surface id.
    pub runtime_surface_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable runtime work class.
    pub work_class: PsionicTrainWorkClass,
    /// Stable outcome kind.
    pub outcome: PsionicTrainOutcomeKind,
    /// Stable numeric exit code.
    pub exit_code: u8,
    /// Retryability bit for supervisors.
    pub retryable: bool,
    /// Authority that owns the next durable transition.
    pub authority_owner: PsionicTrainAuthorityOwner,
    /// Optional refusal class when the run is refused.
    pub refusal_class: Option<PsionicTrainRefusalClass>,
    /// Shared coordination envelope.
    pub coordination: PsionicTrainCoordinationContext,
    /// Optional grouped-replica stage assignment projected by the admitted manifest.
    pub grouped_stage_assignment: Option<PsionicTrainGroupedReplicaStageAssignment>,
    /// Optional challenged work class for validator replay.
    pub validator_target_work_class: Option<PsionicTrainWorkClass>,
    /// Optional invocation manifest digest.
    pub manifest_digest: Option<String>,
    /// Optional run identifier.
    pub run_id: Option<String>,
    /// Optional run root.
    pub run_root: Option<String>,
    /// Optional current window state.
    pub window_state: Option<String>,
    /// Resolved runtime attestation.
    pub runtime_attestation: PsionicTrainRuntimeAttestation,
    /// Minimal runtime capability projection.
    pub capability_projection: PsionicTrainCapabilityProjection,
    /// Shared retained artifact refs.
    pub artifacts: PsionicTrainArtifactSurfaceRefs,
    /// Short human-readable detail for logs and operator inspection.
    pub detail: String,
}

/// Validation errors for the machine-consumable runtime contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionicTrainRuntimeContractError {
    #[error("psionic-train runtime field `{field}` must not be empty")]
    MissingField { field: String },
    #[error("psionic-train runtime field `{field}` expected `{expected}` but found `{actual}`")]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psionic-train runtime field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
}

impl PsionicTrainInvocationManifest {
    /// Computes the canonical manifest digest with `manifest_digest` omitted from the digest basis.
    pub fn canonical_manifest_digest(&self) -> Result<String, PsionicTrainRuntimeContractError> {
        let mut digest_basis = self.clone();
        digest_basis.manifest_digest = None;
        let serialized = serde_json::to_vec(&digest_basis).map_err(|error| {
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("manifest_digest"),
                detail: error.to_string(),
            }
        })?;
        Ok(sha256_hex(&serialized))
    }

    /// Populates the canonical manifest digest in place.
    pub fn populate_manifest_digest(&mut self) -> Result<(), PsionicTrainRuntimeContractError> {
        self.manifest_digest = Some(self.canonical_manifest_digest()?);
        Ok(())
    }

    /// Validates the machine-consumable runtime contract.
    pub fn validate_machine_contract(&self) -> Result<(), PsionicTrainRuntimeContractError> {
        require_nonempty(
            self.schema_version.as_str(),
            "invocation_manifest.schema_version",
        )?;
        if self.schema_version != PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION {
            return Err(PsionicTrainRuntimeContractError::FieldMismatch {
                field: String::from("invocation_manifest.schema_version"),
                expected: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        require_nonempty(
            self.runtime_surface_id.as_str(),
            "invocation_manifest.runtime_surface_id",
        )?;
        if self.runtime_surface_id != PSIONIC_TRAIN_RUNTIME_SURFACE_ID {
            return Err(PsionicTrainRuntimeContractError::FieldMismatch {
                field: String::from("invocation_manifest.runtime_surface_id"),
                expected: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
                actual: self.runtime_surface_id.clone(),
            });
        }
        require_nonempty(self.lane_id.as_str(), "invocation_manifest.lane_id")?;
        if !is_machine_admitted_lane(self.lane_id.as_str()) {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.lane_id"),
                detail: format!(
                    "lane `{}` is not yet admitted by the machine runtime surface",
                    self.lane_id
                ),
            });
        }
        if let Some(manifest_digest) = self.manifest_digest.as_deref() {
            require_nonempty(manifest_digest, "invocation_manifest.manifest_digest")?;
            let expected = self.canonical_manifest_digest()?;
            if manifest_digest != expected {
                return Err(PsionicTrainRuntimeContractError::FieldMismatch {
                    field: String::from("invocation_manifest.manifest_digest"),
                    expected,
                    actual: String::from(manifest_digest),
                });
            }
        }
        if self.role == PsionicTrainRole::Validator
            && self.work_class != PsionicTrainWorkClass::ValidationReplay
        {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.work_class"),
                detail: String::from(
                    "validator manifests must declare work_class=validation_replay on the machine runtime surface",
                ),
            });
        }
        if self.role != PsionicTrainRole::Validator
            && self.work_class == PsionicTrainWorkClass::ValidationReplay
        {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.work_class"),
                detail: String::from(
                    "work_class=validation_replay is admitted only for validator manifests on the machine runtime surface",
                ),
            });
        }
        self.coordination
            .validate("invocation_manifest.coordination")?;
        if let Some(stage_assignment) = &self.grouped_stage_assignment {
            stage_assignment.validate("invocation_manifest.grouped_stage_assignment")?;
            require_nonempty_option(
                self.coordination.window_id.as_deref(),
                "invocation_manifest.coordination.window_id",
            )?;
            require_nonempty_option(
                self.coordination.assignment_id.as_deref(),
                "invocation_manifest.coordination.assignment_id",
            )?;
            match (self.role, self.operation) {
                (PsionicTrainRole::Worker, _) => {
                    if self.work_class != PsionicTrainWorkClass::GroupedReplicaStageExecution {
                        return Err(PsionicTrainRuntimeContractError::InvalidValue {
                            field: String::from("invocation_manifest.work_class"),
                            detail: String::from(
                                "grouped stage manifests must declare work_class=grouped_replica_stage_execution on the machine runtime surface",
                            ),
                        });
                    }
                    if stage_assignment.upstream_stage_id.is_some() {
                        require_artifact_binding_with_materialized_path(
                            self.grouped_stage_input_transport.as_ref(),
                            "invocation_manifest.grouped_stage_input_transport",
                        )?;
                    } else if self.grouped_stage_input_transport.is_some() {
                        return Err(PsionicTrainRuntimeContractError::InvalidValue {
                            field: String::from(
                                "invocation_manifest.grouped_stage_input_transport",
                            ),
                            detail: String::from(
                                "ingress grouped stage must not declare grouped_stage_input_transport",
                            ),
                        });
                    }
                }
                (PsionicTrainRole::RecoverySource, PsionicTrainOperation::Resume) => {
                    if self.work_class != PsionicTrainWorkClass::GroupedReplicaStageExecution {
                        return Err(PsionicTrainRuntimeContractError::InvalidValue {
                            field: String::from("invocation_manifest.work_class"),
                            detail: String::from(
                                "grouped stage manifests must declare work_class=grouped_replica_stage_execution on the machine runtime surface",
                            ),
                        });
                    }
                    if self.grouped_stage_input_transport.is_some() {
                        return Err(PsionicTrainRuntimeContractError::InvalidValue {
                            field: String::from(
                                "invocation_manifest.grouped_stage_input_transport",
                            ),
                            detail: String::from(
                                "grouped_stage_input_transport is not admitted on grouped stage resume manifests",
                            ),
                        });
                    }
                }
                _ => {
                    return Err(PsionicTrainRuntimeContractError::InvalidValue {
                        field: String::from("invocation_manifest.grouped_stage_assignment"),
                        detail: String::from(
                            "grouped stage assignment is currently admitted only for worker operations and recovery_source resume on the machine runtime surface",
                        ),
                    });
                }
            }
        } else if self.grouped_stage_input_transport.is_some() {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.grouped_stage_input_transport"),
                detail: String::from(
                    "grouped_stage_input_transport is only admitted with grouped_stage_assignment on the machine runtime surface",
                ),
            });
        }
        if self.grouped_stage_assignment.is_none()
            && matches!(
                self.role,
                PsionicTrainRole::Worker | PsionicTrainRole::RecoverySource
            )
            && self.work_class == PsionicTrainWorkClass::GroupedReplicaStageExecution
        {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.grouped_stage_assignment"),
                detail: String::from(
                    "work_class=grouped_replica_stage_execution requires grouped_stage_assignment on the machine runtime surface",
                ),
            });
        }
        self.admission_identity
            .validate("invocation_manifest.admission_identity")?;
        require_nonempty_option(
            self.coordination.node_pubkey.as_deref(),
            "invocation_manifest.coordination.node_pubkey",
        )?;
        validate_optional_field(
            self.peer_node_pubkey.as_deref(),
            "invocation_manifest",
            "peer_node_pubkey",
        )?;
        validate_optional_artifact_binding(
            self.peer_checkpoint_handoff_receipt.as_ref(),
            "invocation_manifest",
            "peer_checkpoint_handoff_receipt",
        )?;
        validate_optional_artifact_binding(
            self.validator_target_contribution_receipt.as_ref(),
            "invocation_manifest",
            "validator_target_contribution_receipt",
        )?;
        validate_optional_artifact_binding(
            self.validator_target_contribution_artifact_manifest
                .as_ref(),
            "invocation_manifest",
            "validator_target_contribution_artifact_manifest",
        )?;
        validate_optional_artifact_binding(
            self.grouped_stage_input_transport.as_ref(),
            "invocation_manifest",
            "grouped_stage_input_transport",
        )?;

        if self.selected_git_ref.is_none() {
            return Err(PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.selected_git_ref"),
            });
        }
        require_nonempty_option(
            self.selected_git_ref.as_deref(),
            "invocation_manifest.selected_git_ref",
        )?;

        match self.role {
            PsionicTrainRole::Worker => match self.operation {
                PsionicTrainOperation::Start
                | PsionicTrainOperation::RecordCheckpoint
                | PsionicTrainOperation::Backup
                | PsionicTrainOperation::RehearseBaseLane => {}
                PsionicTrainOperation::Resume
                | PsionicTrainOperation::ServeCheckpoint
                | PsionicTrainOperation::ValidateContribution
                | PsionicTrainOperation::DecideContinueRestart => {
                    return Err(PsionicTrainRuntimeContractError::InvalidValue {
                        field: String::from("invocation_manifest.role"),
                        detail: String::from(
                            "worker role cannot claim recovery-source operations on the machine runtime surface",
                        ),
                    });
                }
            },
            PsionicTrainRole::RecoverySource => match self.operation {
                PsionicTrainOperation::Resume
                | PsionicTrainOperation::ServeCheckpoint
                | PsionicTrainOperation::DecideContinueRestart => {}
                _ => {
                    return Err(PsionicTrainRuntimeContractError::InvalidValue {
                        field: String::from("invocation_manifest.role"),
                        detail: String::from(
                            "recovery_source role is limited to resume, serve_checkpoint, and decide_continue_restart on the machine runtime surface",
                        ),
                    });
                }
            },
            PsionicTrainRole::Validator => match self.operation {
                PsionicTrainOperation::ValidateContribution => {}
                _ => {
                    return Err(PsionicTrainRuntimeContractError::InvalidValue {
                        field: String::from("invocation_manifest.role"),
                        detail: String::from(
                            "validator role is limited to validate_contribution on the machine runtime surface",
                        ),
                    });
                }
            },
        }

        match self.operation {
            PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane => {
                require_nonempty_option(self.run_id.as_deref(), "invocation_manifest.run_id")?;
                require_nonempty_option(
                    self.output_root.as_deref(),
                    "invocation_manifest.output_root",
                )?;
                if self.run_root.is_some() {
                    return Err(PsionicTrainRuntimeContractError::InvalidValue {
                        field: String::from("invocation_manifest.run_root"),
                        detail: String::from(
                            "launch-style operations must use output_root instead of run_root",
                        ),
                    });
                }
            }
            PsionicTrainOperation::Resume
            | PsionicTrainOperation::ServeCheckpoint
            | PsionicTrainOperation::ValidateContribution
            | PsionicTrainOperation::Backup
            | PsionicTrainOperation::DecideContinueRestart => {
                require_nonempty_option(self.run_root.as_deref(), "invocation_manifest.run_root")?;
            }
            PsionicTrainOperation::RecordCheckpoint => {
                require_nonempty_option(self.run_root.as_deref(), "invocation_manifest.run_root")?;
                require_nonempty_option(
                    self.checkpoint_label.as_deref(),
                    "invocation_manifest.checkpoint_label",
                )?;
                if self.optimizer_step.is_none() {
                    return Err(PsionicTrainRuntimeContractError::MissingField {
                        field: String::from("invocation_manifest.optimizer_step"),
                    });
                }
                require_nonempty_option(
                    self.checkpoint_ref.as_deref(),
                    "invocation_manifest.checkpoint_ref",
                )?;
            }
        }
        if self.operation == PsionicTrainOperation::ServeCheckpoint {
            require_nonempty_option(
                self.peer_node_pubkey.as_deref(),
                "invocation_manifest.peer_node_pubkey",
            )?;
        } else if self.peer_node_pubkey.is_some() {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.peer_node_pubkey"),
                detail: String::from(
                    "peer_node_pubkey is only admitted for serve_checkpoint on the machine runtime surface",
                ),
            });
        }
        if self.operation != PsionicTrainOperation::Resume
            && self.peer_checkpoint_handoff_receipt.is_some()
        {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.peer_checkpoint_handoff_receipt"),
                detail: String::from(
                    "peer_checkpoint_handoff_receipt is only admitted for resume on the machine runtime surface",
                ),
            });
        }
        if self.operation == PsionicTrainOperation::ValidateContribution {
            require_nonempty_option(
                self.coordination.window_id.as_deref(),
                "invocation_manifest.coordination.window_id",
            )?;
            require_nonempty_option(
                self.coordination.assignment_id.as_deref(),
                "invocation_manifest.coordination.assignment_id",
            )?;
            require_nonempty_option(
                self.coordination.challenge_id.as_deref(),
                "invocation_manifest.coordination.challenge_id",
            )?;
            require_artifact_binding(
                self.validator_target_contribution_receipt.as_ref(),
                "invocation_manifest.validator_target_contribution_receipt",
            )?;
            require_artifact_binding(
                self.validator_target_contribution_artifact_manifest
                    .as_ref(),
                "invocation_manifest.validator_target_contribution_artifact_manifest",
            )?;
            let validator_target_work_class =
                self.validator_target_work_class.ok_or_else(|| {
                    PsionicTrainRuntimeContractError::MissingField {
                        field: String::from("invocation_manifest.validator_target_work_class"),
                    }
                })?;
            if !validator_target_work_class.is_validator_target_admitted() {
                return Err(PsionicTrainRuntimeContractError::InvalidValue {
                    field: String::from("invocation_manifest.validator_target_work_class"),
                    detail: format!(
                        "validator replay does not yet admit target work_class={} on the machine runtime surface",
                        validator_target_work_class.label()
                    ),
                });
            }
        } else if self.validator_target_contribution_receipt.is_some()
            || self
                .validator_target_contribution_artifact_manifest
                .is_some()
            || self.validator_target_work_class.is_some()
        {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.validator_target_contribution_receipt"),
                detail: String::from(
                    "validator target contribution inputs and target work class are only admitted for validate_contribution on the machine runtime surface",
                ),
            });
        }

        Ok(())
    }
}

impl PsionicTrainStatusPacket {
    /// Creates a success status packet.
    #[allow(clippy::too_many_arguments)]
    pub fn success(
        manifest: &PsionicTrainInvocationManifest,
        manifest_path: Option<String>,
        runtime_attestation: PsionicTrainRuntimeAttestation,
        capability_projection: PsionicTrainCapabilityProjection,
        run_id: Option<String>,
        run_root: Option<String>,
        run_status_packet_path: Option<String>,
        window_status_packet_path: Option<String>,
        current_status_path: Option<String>,
        retained_summary_path: Option<String>,
        latest_checkpoint_pointer_path: Option<String>,
        launcher_log_path: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: String::from(PSIONIC_TRAIN_STATUS_PACKET_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: manifest.lane_id.clone(),
            role: manifest.role,
            operation: manifest.operation,
            work_class: manifest.work_class,
            outcome: PsionicTrainOutcomeKind::Succeeded,
            exit_code: 0,
            retryable: false,
            authority_owner: PsionicTrainAuthorityOwner::Pylon,
            refusal_class: None,
            manifest_path,
            manifest_digest: manifest.manifest_digest.clone(),
            coordination: manifest.coordination.clone(),
            grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
            validator_target_work_class: manifest.validator_target_work_class,
            runtime_attestation: Some(runtime_attestation),
            capability_projection: Some(capability_projection),
            run_id,
            run_root,
            run_status_packet_path,
            window_status_packet_path,
            current_status_path,
            retained_summary_path,
            latest_checkpoint_pointer_path,
            launcher_log_path,
            detail: detail.into(),
        }
    }

    /// Creates a refusal status packet from the shared refusal taxonomy.
    pub fn refusal(
        manifest: Option<&PsionicTrainInvocationManifest>,
        refusal_class: PsionicTrainRefusalClass,
        manifest_path: Option<String>,
        runtime_attestation: Option<PsionicTrainRuntimeAttestation>,
        capability_projection: Option<PsionicTrainCapabilityProjection>,
        run_id: Option<String>,
        run_root: Option<String>,
        run_status_packet_path: Option<String>,
        window_status_packet_path: Option<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: String::from(PSIONIC_TRAIN_STATUS_PACKET_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: manifest
                .map(|value| value.lane_id.clone())
                .unwrap_or_else(|| String::from(PSION_ACTUAL_PRETRAINING_LANE_ID)),
            role: manifest
                .map(|value| value.role)
                .unwrap_or(PsionicTrainRole::Worker),
            operation: manifest
                .map(|value| value.operation)
                .unwrap_or(PsionicTrainOperation::Start),
            work_class: manifest
                .map(|value| value.work_class)
                .unwrap_or(PsionicTrainWorkClass::FullIslandLocalUpdateTraining),
            outcome: PsionicTrainOutcomeKind::Refused,
            exit_code: refusal_class.exit_code(),
            retryable: refusal_class.retryable(),
            authority_owner: refusal_class.authority_owner(),
            refusal_class: Some(refusal_class),
            manifest_path,
            manifest_digest: manifest.and_then(|value| value.manifest_digest.clone()),
            coordination: manifest
                .map(|value| value.coordination.clone())
                .unwrap_or_default(),
            grouped_stage_assignment: manifest
                .and_then(|value| value.grouped_stage_assignment.clone()),
            validator_target_work_class: manifest
                .and_then(|value| value.validator_target_work_class),
            runtime_attestation,
            capability_projection,
            run_id,
            run_root,
            run_status_packet_path,
            window_status_packet_path,
            current_status_path: None,
            retained_summary_path: None,
            latest_checkpoint_pointer_path: None,
            launcher_log_path: None,
            detail: detail.into(),
        }
    }
}

impl PsionicTrainCoordinationContext {
    /// Validates optional coordination fields when present.
    pub fn validate(&self, field_prefix: &str) -> Result<(), PsionicTrainRuntimeContractError> {
        validate_optional_field(self.network_id.as_deref(), field_prefix, "network_id")?;
        validate_optional_field(self.window_id.as_deref(), field_prefix, "window_id")?;
        validate_optional_field(self.assignment_id.as_deref(), field_prefix, "assignment_id")?;
        validate_optional_field(self.challenge_id.as_deref(), field_prefix, "challenge_id")?;
        validate_optional_field(self.node_pubkey.as_deref(), field_prefix, "node_pubkey")?;
        Ok(())
    }
}

impl PsionicTrainAdmissionIdentity {
    /// Validates the admitted runtime identity fields.
    pub fn validate(&self, field_prefix: &str) -> Result<(), PsionicTrainRuntimeContractError> {
        require_nonempty(
            self.release_id.as_str(),
            Box::leak(format!("{field_prefix}.release_id").into_boxed_str()),
        )?;
        require_nonempty(
            self.build_digest.as_str(),
            Box::leak(format!("{field_prefix}.build_digest").into_boxed_str()),
        )?;
        require_nonempty(
            self.environment_ref.as_str(),
            Box::leak(format!("{field_prefix}.environment_ref").into_boxed_str()),
        )?;
        Ok(())
    }
}

impl PsionicTrainRuntimeAttestation {
    /// Creates one resolved runtime attestation packet.
    pub fn new(
        release_id: impl Into<String>,
        build_digest: impl Into<String>,
        git_commit_sha: impl Into<String>,
        dirty_tree_admission: impl Into<String>,
        workspace_status_sha256: Option<String>,
        environment_ref: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: String::from(PSIONIC_TRAIN_RUNTIME_ATTESTATION_SCHEMA_VERSION),
            release_id: release_id.into(),
            build_digest: build_digest.into(),
            git_commit_sha: git_commit_sha.into(),
            dirty_tree_admission: dirty_tree_admission.into(),
            workspace_status_sha256,
            environment_ref: environment_ref.into(),
        }
    }
}

impl PsionicTrainLaneContract {
    /// Builds the canonical machine-runtime contract for one admitted lane.
    pub fn for_lane(lane_id: &str) -> Result<Self, PsionicTrainRuntimeContractError> {
        let contract = canonical_lane_contract_for_lane(lane_id)?;
        Ok(Self {
            lane_id: String::from(contract.lane_id),
            release_id: String::from(contract.release_id),
            environment_ref: String::from(contract.environment_ref),
            backend_family: String::from(contract.backend_family),
            topology_class: String::from(contract.topology_class),
            minimum_machine_class: contract.minimum_machine_class,
        })
    }
}

impl PsionicTrainCapabilityProjection {
    /// Builds the frozen capability projection for one admitted lane and role.
    pub fn for_lane(
        lane_id: &str,
        role: PsionicTrainRole,
        environment_ref: impl Into<String>,
    ) -> Result<Self, PsionicTrainRuntimeContractError> {
        let contract = canonical_lane_contract_for_lane(lane_id)?;
        let environment_ref = environment_ref.into();
        if environment_ref != contract.environment_ref {
            return Err(PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.admission_identity.environment_ref"),
                detail: format!(
                    "lane `{lane_id}` expects admitted environment ref `{}` on the machine runtime surface",
                    contract.environment_ref
                ),
            });
        }
        Ok(Self {
            lane_id: String::from(contract.lane_id),
            role,
            backend_family: String::from(contract.backend_family),
            topology_class: String::from(contract.topology_class),
            environment_ref,
        })
    }
}

/// Returns the admitted release id for one machine-supported lane.
pub fn admitted_release_id_for_lane(
    lane_id: &str,
) -> Result<&'static str, PsionicTrainRuntimeContractError> {
    canonical_lane_contract_for_lane(lane_id).map(|contract| contract.release_id)
}

/// Returns the admitted environment ref for one machine-supported lane.
pub fn admitted_environment_ref_for_lane(
    lane_id: &str,
) -> Result<&'static str, PsionicTrainRuntimeContractError> {
    canonical_lane_contract_for_lane(lane_id).map(|contract| contract.environment_ref)
}

/// Whether the lane is admitted by the machine runtime surface.
pub fn is_machine_admitted_lane(lane_id: &str) -> bool {
    canonical_lane_contract_for_lane(lane_id).is_ok()
}

fn canonical_lane_contract_for_lane(
    lane_id: &str,
) -> Result<PsionicTrainLaneContractStatic, PsionicTrainRuntimeContractError> {
    match lane_id {
        PSION_ACTUAL_PRETRAINING_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_ACTUAL_PRETRAINING_LANE_ID,
            release_id: PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::StrongCudaTrainer,
        }),
        PSION_CS336_A1_DEMO_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_CS336_A1_DEMO_LANE_ID,
            release_id: PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS,
            minimum_machine_class:
                PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator,
        }),
        QWEN_LEGAL_ADAPTER_SFT_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: QWEN_LEGAL_ADAPTER_SFT_LANE_ID,
            release_id: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::StrongCudaTrainer,
        }),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
            release_id: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::AppleSiliconOperator,
        }),
        other => Err(PsionicTrainRuntimeContractError::InvalidValue {
            field: String::from("invocation_manifest.lane_id"),
            detail: format!("lane `{other}` has no canonical machine-runtime lane contract"),
        }),
    }
}

/// Computes the stable runtime build digest from the resolved executable posture.
pub fn runtime_build_digest(
    release_id: &str,
    runtime_surface_id: &str,
    lane_id: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    environment_ref: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"psionic-train-runtime-build|release_id|");
    digest.update(release_id.as_bytes());
    digest.update(b"|runtime_surface_id|");
    digest.update(runtime_surface_id.as_bytes());
    digest.update(b"|lane_id|");
    digest.update(lane_id.as_bytes());
    digest.update(b"|git_commit_sha|");
    digest.update(git_commit_sha.as_bytes());
    digest.update(b"|dirty_tree_admission|");
    digest.update(dirty_tree_admission.as_bytes());
    digest.update(b"|workspace_status_sha256|");
    digest.update(workspace_status_sha256.unwrap_or(""));
    digest.update(b"|environment_ref|");
    digest.update(environment_ref.as_bytes());
    format!("{:x}", digest.finalize())
}

impl PsionicTrainOperation {
    /// Stable CLI subcommand used by the actual-lane operator surface.
    pub fn cli_subcommand(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Resume => "resume",
            Self::ServeCheckpoint => "serve-checkpoint",
            Self::ValidateContribution => "validate-contribution",
            Self::RecordCheckpoint => "record-checkpoint",
            Self::Backup => "backup",
            Self::DecideContinueRestart => "decide-continue-restart",
            Self::RehearseBaseLane => "rehearse-base-lane",
        }
    }
}

impl PsionicTrainRefusalClass {
    /// Stable exit code for the refusal class.
    pub fn exit_code(self) -> u8 {
        match self {
            Self::BadConfig => 10,
            Self::StaleAssignment => 11,
            Self::LeaseExpired => 12,
            Self::UnsupportedTopology => 13,
            Self::GroupedStageAssignmentInvalid => 14,
            Self::CheckpointMissing => 15,
            Self::CheckpointDigestMismatch => 16,
            Self::ArtifactIncomplete => 17,
            Self::ArtifactDigestMismatch => 18,
            Self::ValidatorTimeout => 19,
            Self::ValidatorDisagreement => 20,
            Self::EnvironmentMismatch => 21,
            Self::BuildRevoked => 22,
            Self::InternalError => 70,
        }
    }

    /// Whether the caller should treat the refusal as retryable.
    pub fn retryable(self) -> bool {
        match self {
            Self::BadConfig => false,
            Self::StaleAssignment => true,
            Self::LeaseExpired => true,
            Self::UnsupportedTopology => false,
            Self::GroupedStageAssignmentInvalid => false,
            Self::CheckpointMissing => true,
            Self::CheckpointDigestMismatch => false,
            Self::ArtifactIncomplete => true,
            Self::ArtifactDigestMismatch => false,
            Self::ValidatorTimeout => true,
            Self::ValidatorDisagreement => true,
            Self::EnvironmentMismatch => false,
            Self::BuildRevoked => false,
            Self::InternalError => false,
        }
    }

    /// Which authority owns the next durable transition once the refusal leaves the local process.
    pub fn authority_owner(self) -> PsionicTrainAuthorityOwner {
        match self {
            Self::StaleAssignment
            | Self::LeaseExpired
            | Self::ValidatorTimeout
            | Self::ValidatorDisagreement => PsionicTrainAuthorityOwner::Nexus,
            Self::BadConfig
            | Self::UnsupportedTopology
            | Self::GroupedStageAssignmentInvalid
            | Self::CheckpointMissing
            | Self::CheckpointDigestMismatch
            | Self::ArtifactIncomplete
            | Self::ArtifactDigestMismatch
            | Self::EnvironmentMismatch
            | Self::BuildRevoked
            | Self::InternalError => PsionicTrainAuthorityOwner::Pylon,
        }
    }
}

fn require_nonempty(
    value: &str,
    field: &'static str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    if value.trim().is_empty() {
        return Err(PsionicTrainRuntimeContractError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn require_nonempty_option(
    value: Option<&str>,
    field: &'static str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    let value = value.ok_or_else(|| PsionicTrainRuntimeContractError::MissingField {
        field: String::from(field),
    })?;
    require_nonempty(value, field)
}

fn validate_optional_field(
    value: Option<&str>,
    field_prefix: &str,
    field_name: &str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    if let Some(value) = value {
        require_nonempty(
            value,
            Box::leak(format!("{field_prefix}.{field_name}").into_boxed_str()),
        )?;
    }
    Ok(())
}

fn validate_optional_artifact_binding(
    value: Option<&PsionicTrainArtifactBinding>,
    field_prefix: &str,
    field_name: &str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    if let Some(value) = value {
        value
            .validate(format!("{field_prefix}.{field_name}").as_str())
            .map_err(|detail| PsionicTrainRuntimeContractError::InvalidValue {
                field: format!("{field_prefix}.{field_name}"),
                detail,
            })?;
    }
    Ok(())
}

fn require_artifact_binding_with_materialized_path(
    value: Option<&PsionicTrainArtifactBinding>,
    field: &'static str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    let value = value.ok_or_else(|| PsionicTrainRuntimeContractError::MissingField {
        field: String::from(field),
    })?;
    value
        .validate(field)
        .map_err(|detail| PsionicTrainRuntimeContractError::InvalidValue {
            field: String::from(field),
            detail,
        })?;
    value.require_materialized_path(field).map_err(|detail| {
        PsionicTrainRuntimeContractError::InvalidValue {
            field: String::from(field),
            detail,
        }
    })?;
    Ok(())
}

fn require_artifact_binding(
    value: Option<&PsionicTrainArtifactBinding>,
    field: &'static str,
) -> Result<(), PsionicTrainRuntimeContractError> {
    let value = value.ok_or_else(|| PsionicTrainRuntimeContractError::MissingField {
        field: String::from(field),
    })?;
    value
        .validate(field)
        .map_err(|detail| PsionicTrainRuntimeContractError::InvalidValue {
            field: String::from(field),
            detail,
        })?;
    Ok(())
}

#[must_use]
pub fn psionic_train_local_artifact_id(artifact_role: &str, artifact_digest: &str) -> String {
    let sanitized_role = artifact_role
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("psionic.train.artifact.{sanitized_role}.{artifact_digest}")
}

#[must_use]
pub fn psionic_train_resolved_artifact_cache_key(artifact_id: &str) -> String {
    let mut sanitized = artifact_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() {
                value.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        String::from("artifact")
    } else {
        sanitized.to_string()
    }
}

#[must_use]
pub fn psionic_train_resolved_artifact_cache_candidates(
    run_root: &Path,
    artifact_id: &str,
) -> Vec<PathBuf> {
    let cache_root = run_root.join(PSIONIC_TRAIN_RESOLVED_ARTIFACT_CACHE_RELATIVE_DIR);
    let cache_key = psionic_train_resolved_artifact_cache_key(artifact_id);
    vec![
        cache_root.join(format!("{cache_key}.json")),
        cache_root.join(cache_key),
    ]
}

pub fn build_psionic_train_artifact_binding_from_path(
    artifact_role: &str,
    path: &Path,
) -> Result<PsionicTrainArtifactBinding, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    let artifact_bytes = u64::try_from(bytes.len())
        .map_err(|error| format!("failed to size `{}`: {error}", path.display()))?;
    let artifact_digest = sha256_hex(bytes.as_slice());
    Ok(PsionicTrainArtifactBinding {
        artifact_ref: PsionicTrainArtifactRef {
            artifact_id: psionic_train_local_artifact_id(artifact_role, artifact_digest.as_str()),
            artifact_digest: Some(artifact_digest),
            artifact_bytes: Some(artifact_bytes),
        },
        materialized_path: Some(path.display().to_string()),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PsionicTrainGroupedReplicaStageRole;

    fn artifact_binding(path: &str) -> PsionicTrainArtifactBinding {
        PsionicTrainArtifactBinding {
            artifact_ref: PsionicTrainArtifactRef {
                artifact_id: format!("artifact://{}", path.replace('/', "_")),
                artifact_digest: Some(format!("sha256:{}", sha256_hex(path.as_bytes()))),
                artifact_bytes: Some(path.len() as u64),
            },
            materialized_path: Some(String::from(path)),
        }
    }

    #[test]
    fn resolved_artifact_cache_candidates_are_stable() {
        let candidates = psionic_train_resolved_artifact_cache_candidates(
            Path::new("/tmp/psionic-train-run"),
            "artifact://checkpoint/pointer@v1",
        );
        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0],
            Path::new(
                "/tmp/psionic-train-run/artifacts/resolved/artifact_checkpoint_pointer_v1.json"
            )
        );
        assert_eq!(
            candidates[1],
            Path::new("/tmp/psionic-train-run/artifacts/resolved/artifact_checkpoint_pointer_v1")
        );
    }

    fn base_manifest() -> PsionicTrainInvocationManifest {
        PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psionic.contract-test")),
                window_id: None,
                assignment_id: None,
                challenge_id: None,
                node_pubkey: Some(String::from("npub1-psionic-contract-test")),
                membership_revision: None,
            },
            grouped_stage_assignment: None,
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: String::from("sha256:test-build"),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(String::from("psion-train-contract-test")),
            output_root: Some(String::from("/tmp/psion-train-contract-test")),
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

    #[test]
    fn invocation_manifest_digest_round_trips() {
        let mut manifest = base_manifest();
        manifest
            .populate_manifest_digest()
            .expect("digest should populate");
        manifest
            .validate_machine_contract()
            .expect("manifest should validate after digest population");
    }

    #[test]
    fn launch_manifest_requires_explicit_output_root() {
        let mut manifest = base_manifest();
        manifest.output_root = None;
        let error = manifest
            .validate_machine_contract()
            .expect_err("machine launch manifest should require output_root");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.output_root"),
            }
        );
    }

    #[test]
    fn validator_role_requires_validate_contribution_operation() {
        let mut manifest = base_manifest();
        manifest.role = PsionicTrainRole::Validator;
        manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
        let error = manifest
            .validate_machine_contract()
            .expect_err("validator role should reject worker operations");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.role"),
                detail: String::from(
                    "validator role is limited to validate_contribution on the machine runtime surface",
                ),
            }
        );
    }

    #[test]
    fn validator_manifest_requires_replay_target_bindings() {
        let mut manifest = base_manifest();
        manifest.role = PsionicTrainRole::Validator;
        manifest.operation = PsionicTrainOperation::ValidateContribution;
        manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
        manifest.output_root = None;
        manifest.run_root = Some(String::from("/tmp/validator-run"));
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.coordination.challenge_id = Some(String::from("challenge-0001"));
        let error = manifest
            .validate_machine_contract()
            .expect_err("validator replay should require target contribution paths");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.validator_target_contribution_receipt"),
            }
        );
    }

    #[test]
    fn refusal_codes_and_authority_owners_are_stable() {
        assert_eq!(PsionicTrainRefusalClass::BadConfig.exit_code(), 10);
        assert_eq!(PsionicTrainRefusalClass::LeaseExpired.exit_code(), 12);
        assert_eq!(
            PsionicTrainRefusalClass::GroupedStageAssignmentInvalid.exit_code(),
            14
        );
        assert_eq!(
            PsionicTrainRefusalClass::ValidatorTimeout.authority_owner(),
            PsionicTrainAuthorityOwner::Nexus
        );
        assert!(!PsionicTrainRefusalClass::BuildRevoked.retryable());
    }

    #[test]
    fn worker_manifest_requires_node_pubkey() {
        let mut manifest = base_manifest();
        manifest.coordination.node_pubkey = None;
        let error = manifest
            .validate_machine_contract()
            .expect_err("worker manifest should require node_pubkey");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.coordination.node_pubkey"),
            }
        );
    }

    #[test]
    fn runtime_build_digest_is_stable() {
        let digest = runtime_build_digest(
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
            PSION_ACTUAL_PRETRAINING_LANE_ID,
            "1111222233334444555566667777888899990000",
            "refuse_by_default",
            None,
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        );
        assert_eq!(
            digest,
            "2ed54b617f887b00bcb7f65caecf75a40058e5cea8c7b12631948f199c8281b7"
        );
    }

    #[test]
    fn apple_lane_is_admitted_by_machine_contract() {
        let mut manifest = base_manifest();
        manifest.lane_id = String::from(PSION_APPLE_WINDOWED_TRAINING_LANE_ID);
        manifest.admission_identity = PsionicTrainAdmissionIdentity {
            release_id: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID),
            build_digest: String::from("sha256:apple-build"),
            environment_ref: String::from(PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF),
        };
        manifest
            .validate_machine_contract()
            .expect("apple machine manifests should validate");
        let projection = PsionicTrainCapabilityProjection::for_lane(
            PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
            PsionicTrainRole::Worker,
            PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
        )
        .expect("apple capability projection should exist");
        assert_eq!(
            projection.backend_family,
            PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY
        );
        assert_eq!(
            projection.topology_class,
            PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS
        );
    }

    #[test]
    fn cs336_a1_demo_lane_is_admitted_by_machine_contract() {
        let mut manifest = base_manifest();
        manifest.lane_id = String::from(PSION_CS336_A1_DEMO_LANE_ID);
        manifest.work_class = PsionicTrainWorkClass::SmallModelLocalTraining;
        manifest.admission_identity = PsionicTrainAdmissionIdentity {
            release_id: String::from(PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID),
            build_digest: String::from("sha256:cs336-a1-demo"),
            environment_ref: String::from(PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF),
        };
        manifest
            .validate_machine_contract()
            .expect("bounded A1 demo machine manifests should validate");
        let projection = PsionicTrainCapabilityProjection::for_lane(
            PSION_CS336_A1_DEMO_LANE_ID,
            PsionicTrainRole::Worker,
            PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF,
        )
        .expect("bounded A1 demo capability projection should exist");
        assert_eq!(
            projection.backend_family,
            PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY
        );
        assert_eq!(
            projection.topology_class,
            PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS
        );
    }

    #[test]
    fn cs336_a1_demo_lane_contract_is_frozen_as_cross_platform_cpu_compatible() {
        let contract = PsionicTrainLaneContract::for_lane(PSION_CS336_A1_DEMO_LANE_ID)
            .expect("bounded A1 demo lane contract should exist");
        assert_eq!(contract.lane_id, PSION_CS336_A1_DEMO_LANE_ID);
        assert_eq!(contract.release_id, PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID);
        assert_eq!(
            contract.environment_ref,
            PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF
        );
        assert_eq!(
            contract.backend_family,
            PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY
        );
        assert_eq!(
            contract.topology_class,
            PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS
        );
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator
        );
        assert_eq!(
            contract.minimum_machine_class.label(),
            "cross_platform_cpu_compatible_operator"
        );
    }

    #[test]
    fn qwen_legal_adapter_lane_is_admitted_by_machine_contract() {
        let mut manifest = base_manifest();
        manifest.lane_id = String::from(QWEN_LEGAL_ADAPTER_SFT_LANE_ID);
        manifest.work_class = PsionicTrainWorkClass::AdapterTraining;
        manifest.admission_identity = PsionicTrainAdmissionIdentity {
            release_id: String::from(PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID),
            build_digest: String::from("sha256:qwen-legal-adapter-smoke"),
            environment_ref: String::from(PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF),
        };
        manifest
            .validate_machine_contract()
            .expect("Qwen legal adapter smoke manifests should validate");
        let contract = PsionicTrainLaneContract::for_lane(QWEN_LEGAL_ADAPTER_SFT_LANE_ID)
            .expect("Qwen legal adapter lane contract should exist");
        assert_eq!(
            contract.release_id,
            PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID
        );
        assert_eq!(
            contract.environment_ref,
            PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF
        );
        assert_eq!(
            contract.backend_family,
            PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_BACKEND_FAMILY
        );
        assert_eq!(
            contract.topology_class,
            PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_TOPOLOGY_CLASS
        );
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::StrongCudaTrainer
        );
    }

    #[test]
    fn capability_projection_refuses_wrong_environment_ref_for_lane() {
        let error = PsionicTrainCapabilityProjection::for_lane(
            PSION_CS336_A1_DEMO_LANE_ID,
            PsionicTrainRole::Worker,
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        )
        .expect_err("capability projection should refuse conflicting environment truth");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.admission_identity.environment_ref"),
                detail: format!(
                    "lane `{}` expects admitted environment ref `{}` on the machine runtime surface",
                    PSION_CS336_A1_DEMO_LANE_ID, PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF
                ),
            }
        );
    }

    #[test]
    fn grouped_stage_assignment_requires_window_and_assignment_context() {
        let mut manifest = base_manifest();
        manifest.coordination.window_id = None;
        manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
        manifest.grouped_stage_assignment = Some(
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
        );
        let error = manifest
            .validate_machine_contract()
            .expect_err("grouped stage assignment should require window context");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.coordination.window_id"),
            }
        );
    }

    #[test]
    fn validator_manifest_rejects_grouped_stage_assignment() {
        let mut manifest = base_manifest();
        manifest.role = PsionicTrainRole::Validator;
        manifest.operation = PsionicTrainOperation::ValidateContribution;
        manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
        manifest.output_root = None;
        manifest.run_root = Some(String::from("/tmp/validator-run"));
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.coordination.challenge_id = Some(String::from("challenge-0001"));
        manifest.validator_target_contribution_receipt =
            Some(artifact_binding("/tmp/contribution_receipt.json"));
        manifest.validator_target_contribution_artifact_manifest =
            Some(artifact_binding("/tmp/artifact_manifest.json"));
        manifest.validator_target_work_class = Some(PsionicTrainWorkClass::AdapterTraining);
        manifest.grouped_stage_assignment = Some(
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
        );
        let error = manifest
            .validate_machine_contract()
            .expect_err("validator should reject grouped-stage worker assignment");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.grouped_stage_assignment"),
                detail: String::from(
                    "grouped stage assignment is currently admitted only for worker operations and recovery_source resume on the machine runtime surface",
                ),
            }
        );
    }

    #[test]
    fn downstream_grouped_stage_requires_input_transport_binding() {
        let mut manifest = base_manifest();
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
        manifest.grouped_stage_assignment = Some(
            PsionicTrainGroupedReplicaStageAssignment::new(
                "replica-01",
                "stage-02",
                1,
                2,
                PsionicTrainGroupedReplicaStageRole::Egress,
                Some(String::from("stage-01")),
                None,
            )
            .expect("grouped stage assignment should build"),
        );
        let error = manifest
            .validate_machine_contract()
            .expect_err("downstream grouped stage should require input transport path");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.grouped_stage_input_transport"),
            }
        );
    }

    #[test]
    fn ingress_grouped_stage_rejects_input_transport_binding() {
        let mut manifest = base_manifest();
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
        manifest.grouped_stage_assignment = Some(
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
        );
        manifest.grouped_stage_input_transport =
            Some(artifact_binding("/tmp/grouped-stage-input-transport.json"));
        let error = manifest
            .validate_machine_contract()
            .expect_err("ingress grouped stage should reject input transport path");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.grouped_stage_input_transport"),
                detail: String::from(
                    "ingress grouped stage must not declare grouped_stage_input_transport",
                ),
            }
        );
    }

    #[test]
    fn validator_manifest_requires_target_work_class() {
        let mut manifest = base_manifest();
        manifest.role = PsionicTrainRole::Validator;
        manifest.operation = PsionicTrainOperation::ValidateContribution;
        manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
        manifest.output_root = None;
        manifest.run_root = Some(String::from("/tmp/validator-run"));
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.coordination.challenge_id = Some(String::from("challenge-0001"));
        manifest.validator_target_contribution_receipt =
            Some(artifact_binding("/tmp/contribution_receipt.json"));
        manifest.validator_target_contribution_artifact_manifest =
            Some(artifact_binding("/tmp/artifact_manifest.json"));
        let error = manifest
            .validate_machine_contract()
            .expect_err("validator replay should require validator_target_work_class");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::MissingField {
                field: String::from("invocation_manifest.validator_target_work_class"),
            }
        );
    }

    #[test]
    fn grouped_stage_manifest_requires_grouped_work_class() {
        let mut manifest = base_manifest();
        manifest.coordination.window_id = Some(String::from("window-0001"));
        manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
        manifest.grouped_stage_assignment = Some(
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
        );
        let error = manifest
            .validate_machine_contract()
            .expect_err("grouped stage manifests should require grouped work class");
        assert_eq!(
            error,
            PsionicTrainRuntimeContractError::InvalidValue {
                field: String::from("invocation_manifest.work_class"),
                detail: String::from(
                    "grouped stage manifests must declare work_class=grouped_replica_stage_execution on the machine runtime surface",
                ),
            }
        );
    }
}
