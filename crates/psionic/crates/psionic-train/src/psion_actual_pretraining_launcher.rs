use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    predict_psionic_train_window_artifacts, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingLauncherSurfaces, PsionicTrainAdmissionIdentity,
    PsionicTrainArtifactBinding, PsionicTrainArtifactSurfaceRefs, PsionicTrainCoordinationContext,
    PsionicTrainInvocationManifest, PsionicTrainOperation, PsionicTrainRole,
    PsionicTrainRuntimeContractError, PsionicTrainWorkClass,
    PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF, PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
    PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID, PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID, PSION_ACTUAL_PRETRAINING_START_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
};

/// Stable schema version for the canonical actual-lane launch manifest.
pub const PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_launch_manifest.v1";

/// Stable schema version for the canonical actual-lane resume manifest.
pub const PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_resume_manifest.v1";

/// Stable schema version for the canonical actual-lane checkpoint pointer.
pub const PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_checkpoint_pointer.v1";

/// Stable schema version for the canonical actual-lane closeout bundle.
pub const PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_closeout_bundle.v1";

/// Stable schema version for one strong-node automatic execution request.
pub const PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_automatic_execution_request.v1";

/// Stable schema version for one strong-node automatic execution output plan.
pub const PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_OUTPUTS_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_automatic_execution_outputs.v1";

/// Canonical request that packages assigned actual-lane work into the machine runtime contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAutomaticExecutionRequest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable admitted runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Shared coordination envelope frozen for receipts and status packets.
    #[serde(default)]
    pub coordination: PsionicTrainCoordinationContext,
    /// Admitted build digest expected before launch.
    pub build_digest: String,
    /// Stable run identifier the assignment belongs to.
    pub run_id: String,
    /// Explicit output root for launch-style operations.
    pub output_root: Option<String>,
    /// Explicit run root for retained-state operations.
    pub run_root: Option<String>,
    /// Explicit git ref selection.
    pub selected_git_ref: String,
    /// Optional retained hardware observation path.
    pub hardware_observation_path: Option<String>,
    /// Optional retained run-shape observation path.
    pub run_shape_observation_path: Option<String>,
    /// Optional admitted peer node pubkey for checkpoint serving.
    pub peer_node_pubkey: Option<String>,
    /// Optional resolver-backed peer checkpoint handoff receipt for resume.
    #[serde(default)]
    pub peer_checkpoint_handoff_receipt: Option<PsionicTrainArtifactBinding>,
    /// Optional checkpoint label for checkpoint recording.
    pub checkpoint_label: Option<String>,
    /// Optional optimizer step for checkpoint recording.
    pub optimizer_step: Option<u64>,
    /// Optional checkpoint ref for checkpoint recording.
    pub checkpoint_ref: Option<String>,
    /// Optional checkpoint object digest override.
    pub checkpoint_object_digest: Option<String>,
    /// Optional checkpoint object byte count override.
    pub checkpoint_total_bytes: Option<u64>,
    /// Whether dirty-tree override is admitted.
    #[serde(default)]
    pub allow_dirty_tree: bool,
    /// Whether execution remains in dry-run posture.
    #[serde(default)]
    pub dry_run: bool,
    /// Optional failed-upload drill.
    #[serde(default)]
    pub inject_failed_upload: bool,
    /// Optional eval-worker-unavailable drill.
    #[serde(default)]
    pub inject_eval_worker_unavailable: bool,
}

/// Deterministic retained outputs for one packaged actual-lane machine execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingAutomaticExecutionOutputs {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable admitted lane identifier.
    pub lane_id: String,
    /// Stable runtime role.
    pub role: PsionicTrainRole,
    /// Stable runtime operation.
    pub operation: PsionicTrainOperation,
    /// Stable runtime work class.
    pub work_class: PsionicTrainWorkClass,
    /// Stable run identifier.
    pub run_id: String,
    /// Absolute run root the node will materialize or resume.
    pub run_root: String,
    /// Optional absolute window root when the coordination envelope binds one window.
    pub window_root: Option<String>,
    /// Optional absolute operation-manifest path under the run root.
    pub operation_manifest_path: Option<String>,
    /// Optional absolute current-status path under the run root.
    pub current_status_path: Option<String>,
    /// Optional absolute retained-summary path under the run root.
    pub retained_summary_path: Option<String>,
    /// Optional absolute launcher log path under the run root.
    pub launcher_log_path: Option<String>,
    /// Absolute retained machine run-status packet path.
    pub run_status_packet_path: String,
    /// Absolute retained machine window-status packet path.
    pub window_status_packet_path: String,
    /// Deterministic artifact-surface refs the execution can materialize.
    pub artifacts: PsionicTrainArtifactSurfaceRefs,
}

/// Fixed retained paths used by the actual-lane launcher contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRetainedPathSet {
    /// Relative launch manifest path.
    pub launch_manifest_path: String,
    /// Relative resume manifest path.
    pub resume_manifest_path: String,
    /// Relative current status path.
    pub current_status_path: String,
    /// Relative retained summary path.
    pub retained_summary_path: String,
    /// Relative latest-checkpoint pointer path.
    pub latest_checkpoint_pointer_path: String,
    /// Relative latest accepted-checkpoint backup receipt path.
    pub latest_checkpoint_backup_receipt_path: String,
    /// Relative latest auto-resume receipt path.
    pub auto_resume_receipt_path: String,
    /// Relative latest checkpoint eval decision path.
    pub latest_checkpoint_eval_decision_path: String,
    /// Relative latest checkpoint eval failure path.
    pub latest_checkpoint_eval_failure_path: String,
    /// Relative latest checkpoint comparison path.
    pub latest_checkpoint_comparison_path: String,
    /// Relative latest continue-restart decision path.
    pub latest_continue_restart_decision_path: String,
    /// Relative retained dashboard path.
    pub current_dashboard_path: String,
    /// Relative hardware-qualification receipt path.
    pub hardware_qualification_path: String,
    /// Relative run-shape qualification receipt path.
    pub run_shape_qualification_path: String,
    /// Relative continuation handoff path.
    pub continuation_handoff_path: String,
    /// Relative closeout bundle path.
    pub closeout_bundle_path: String,
    /// Relative launcher log path.
    pub launcher_log_path: String,
    /// Relative latest redacted alert path.
    pub latest_redacted_alert_path: String,
    /// Relative active-alert feed path.
    pub active_alert_feed_path: String,
}

/// Run-local preflight receipt reference consumed by the actual-lane launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingPreflightRef {
    /// Relative path to the retained hardware-qualification receipt.
    pub relative_path: String,
    /// Stable digest of the retained hardware-qualification receipt.
    pub receipt_digest: String,
    /// Hardware admission state consumed by the launcher.
    pub admission_state: String,
}

/// Contract refs consumed by the actual-lane launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingLauncherContractRefs {
    /// Actual-lane spec fixture.
    pub lane_spec: PsionActualPretrainingArtifactRef,
    /// Recipe bundle fixture.
    pub recipe_bundle: PsionActualPretrainingArtifactRef,
    /// Baseline-tools bundle fixture.
    pub baseline_tools_bundle: PsionActualPretrainingArtifactRef,
    /// Scaling bundle fixture.
    pub scaling_bundle: PsionActualPretrainingArtifactRef,
    /// Data bundle fixture.
    pub data_bundle: PsionActualPretrainingArtifactRef,
    /// Systems bundle fixture.
    pub systems_bundle: PsionActualPretrainingArtifactRef,
    /// Topology/storage bundle fixture.
    pub topology_storage_bundle: PsionActualPretrainingArtifactRef,
    /// Evidence contract fixture.
    pub evidence_contract: PsionActualPretrainingArtifactRef,
}

/// Redacted credential-source binding retained by the launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCredentialBinding {
    /// Credential-source kind.
    pub kind: String,
    /// Declared source name.
    pub source_name: String,
    /// Redaction policy retained by artifacts.
    pub retained_redaction: String,
}

/// Actual run roots selected by the launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingRunRoots {
    /// Local run root.
    pub local_run_root: String,
    /// Remote durable run root.
    pub remote_run_root: String,
    /// Remote durable checkpoint root.
    pub remote_checkpoint_root: String,
    /// Remote durable manifest root.
    pub remote_manifest_root: String,
    /// Remote transient log root.
    pub remote_log_root: String,
}

/// Launch manifest for the actual pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingLaunchManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable launcher surface id.
    pub surface_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable recipe identifier.
    pub recipe_id: String,
    /// Stable topology/storage bundle identifier.
    pub topology_storage_bundle_id: String,
    /// Stable evidence-contract identifier.
    pub evidence_contract_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Fixed retained paths under the run root.
    pub retained_paths: PsionActualPretrainingRetainedPathSet,
    /// Reserved launcher surface ids.
    pub launcher_surfaces: PsionActualPretrainingLauncherSurfaces,
    /// Selected local and remote run roots.
    pub run_roots: PsionActualPretrainingRunRoots,
    /// Run-local preflight receipt consumed by launch.
    pub preflight_receipt: PsionActualPretrainingPreflightRef,
    /// Run-local run-shape qualification consumed by launch.
    pub run_shape_receipt: PsionActualPretrainingPreflightRef,
    /// Committed contract refs this launch consumes directly.
    pub contract_refs: PsionActualPretrainingLauncherContractRefs,
    /// Selected git ref.
    pub selected_git_ref: String,
    /// Exact resolved git commit SHA.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Optional digest of the status snapshot when dirty-tree override is used.
    pub workspace_status_sha256: Option<String>,
    /// Redacted credential-source bindings.
    pub credential_sources: Vec<PsionActualPretrainingCredentialBinding>,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
}

/// Resume manifest for the actual pretraining lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingResumeManifest {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable launcher surface id.
    pub surface_id: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable recipe identifier.
    pub recipe_id: String,
    /// Stable topology/storage bundle identifier.
    pub topology_storage_bundle_id: String,
    /// Stable evidence-contract identifier.
    pub evidence_contract_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Fixed retained paths under the run root.
    pub retained_paths: PsionActualPretrainingRetainedPathSet,
    /// Reserved launcher surface ids.
    pub launcher_surfaces: PsionActualPretrainingLauncherSurfaces,
    /// Selected local and remote run roots.
    pub run_roots: PsionActualPretrainingRunRoots,
    /// Run-local preflight receipt consumed by resume.
    pub preflight_receipt: PsionActualPretrainingPreflightRef,
    /// Run-local run-shape qualification consumed by resume.
    pub run_shape_receipt: PsionActualPretrainingPreflightRef,
    /// Committed contract refs this resume consumes directly.
    pub contract_refs: PsionActualPretrainingLauncherContractRefs,
    /// Selected git ref.
    pub selected_git_ref: String,
    /// Exact resolved git commit SHA.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Optional digest of the status snapshot when dirty-tree override is used.
    pub workspace_status_sha256: Option<String>,
    /// Relative path to the canonical checkpoint pointer.
    pub latest_checkpoint_pointer_path: String,
    /// Checkpoint label chosen for resume.
    pub checkpoint_label: String,
    /// Optimizer step chosen for resume.
    pub optimizer_step: u64,
    /// Checkpoint ref chosen for resume.
    pub checkpoint_ref: String,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
}

/// Canonical latest-checkpoint pointer for the actual lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCheckpointPointer {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Pointer state.
    pub pointer_state: String,
    /// Last known checkpoint label.
    pub checkpoint_label: String,
    /// Last known accepted step.
    pub optimizer_step: u64,
    /// Optional checkpoint ref for an accepted pointer.
    pub checkpoint_ref: Option<String>,
    /// Optional checkpoint-manifest path for an accepted pointer.
    pub checkpoint_manifest_relative_path: Option<String>,
    /// Short detail.
    pub detail: String,
}

/// One retained artifact explicitly cited by the actual-lane closeout bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCloseoutArtifact {
    /// Stable artifact kind identifier.
    pub artifact_kind: String,
    /// Retained artifact ref.
    pub artifact: PsionActualPretrainingArtifactRef,
    /// Short detail.
    pub detail: String,
}

/// One explicit gate checked during actual-lane closeout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCloseoutGate {
    /// Stable gate identifier.
    pub gate_id: String,
    /// Whether the gate was satisfied.
    pub satisfied: bool,
    /// Short detail.
    pub detail: String,
}

/// One retained failure drill carried into the actual-lane closeout bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCloseoutFailureDrill {
    /// Stable drill identifier.
    pub drill_id: String,
    /// Stable drill resolution state.
    pub resolution_state: String,
    /// Retained drill artifact.
    pub artifact: PsionActualPretrainingArtifactRef,
    /// Short detail.
    pub detail: String,
}

/// Provisional closeout bundle emitted by the actual-lane launcher.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionActualPretrainingCloseoutBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable actual-lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Current closeout state.
    pub closeout_state: String,
    /// Fixed retained paths under the run root.
    pub retained_paths: PsionActualPretrainingRetainedPathSet,
    /// Selected git ref.
    pub selected_git_ref: String,
    /// Exact resolved git commit SHA.
    pub git_commit_sha: String,
    /// Dirty-tree admission posture.
    pub dirty_tree_admission: String,
    /// Optional digest of the status snapshot when dirty-tree override is used.
    pub workspace_status_sha256: Option<String>,
    /// Explicit retained artifacts cited by the closeout bundle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_artifacts: Vec<PsionActualPretrainingCloseoutArtifact>,
    /// Explicit gates checked during closeout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closeout_gates: Vec<PsionActualPretrainingCloseoutGate>,
    /// Explicit failure drills retained during closeout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_drills: Vec<PsionActualPretrainingCloseoutFailureDrill>,
    /// Things the operator can now honestly claim.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub can_now_claim: Vec<String>,
    /// Things that remain explicitly out of scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub still_out_of_scope: Vec<String>,
    /// Narrow claim boundary.
    pub claim_boundary: String,
    /// Short detail.
    pub detail: String,
}

/// Returns the fixed retained-path contract for the actual pretraining lane.
#[must_use]
pub fn psion_actual_pretraining_retained_paths() -> PsionActualPretrainingRetainedPathSet {
    PsionActualPretrainingRetainedPathSet {
        launch_manifest_path: String::from("manifests/launch_manifest.json"),
        resume_manifest_path: String::from("manifests/resume_manifest.json"),
        current_status_path: String::from("status/current_run_status.json"),
        retained_summary_path: String::from("status/retained_summary.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        latest_checkpoint_backup_receipt_path: String::from(
            "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
        ),
        auto_resume_receipt_path: String::from("checkpoints/auto_resume_receipt.json"),
        latest_checkpoint_eval_decision_path: String::from(
            "evals/latest_checkpoint_eval_decision.json",
        ),
        latest_checkpoint_eval_failure_path: String::from(
            "evals/latest_checkpoint_eval_failure.json",
        ),
        latest_checkpoint_comparison_path: String::from(
            "decisions/latest_checkpoint_comparison.json",
        ),
        latest_continue_restart_decision_path: String::from(
            "decisions/latest_continue_restart_decision.json",
        ),
        current_dashboard_path: String::from("dashboard/current_dashboard.json"),
        hardware_qualification_path: String::from("preflight/hardware_qualification.json"),
        run_shape_qualification_path: String::from("preflight/run_shape_qualification.json"),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        closeout_bundle_path: String::from("closeout/closeout_bundle.json"),
        launcher_log_path: String::from("logs/launcher.log"),
        latest_redacted_alert_path: String::from("alerts/latest_redacted_alert.json"),
        active_alert_feed_path: String::from("alerts/active_alerts.json"),
    }
}

impl PsionActualPretrainingAutomaticExecutionRequest {
    /// Validates the assignment-shaped automatic execution request.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        self.to_invocation_manifest().map(|_| ())
    }

    /// Builds the stable machine runtime invocation manifest for the actual lane.
    pub fn to_invocation_manifest(
        &self,
    ) -> Result<PsionicTrainInvocationManifest, PsionActualPretrainingLauncherError> {
        self.validate_request_fields()?;
        let mut manifest = PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: self.role,
            operation: self.operation,
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            coordination: self.coordination.clone(),
            grouped_stage_assignment: None,
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID),
                build_digest: self.build_digest.clone(),
                environment_ref: String::from(PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF),
            },
            run_id: Some(self.run_id.clone()),
            output_root: self.output_root.clone(),
            run_root: self.run_root.clone(),
            peer_node_pubkey: self.peer_node_pubkey.clone(),
            peer_checkpoint_handoff_receipt: self.peer_checkpoint_handoff_receipt.clone(),
            validator_target_contribution_receipt: None,
            validator_target_contribution_artifact_manifest: None,
            validator_target_work_class: None,
            grouped_stage_input_transport: None,
            selected_git_ref: Some(self.selected_git_ref.clone()),
            hardware_observation_path: self.hardware_observation_path.clone(),
            run_shape_observation_path: self.run_shape_observation_path.clone(),
            allow_dirty_tree: self.allow_dirty_tree,
            dry_run: self.dry_run,
            checkpoint_label: self.checkpoint_label.clone(),
            optimizer_step: self.optimizer_step,
            checkpoint_ref: self.checkpoint_ref.clone(),
            checkpoint_object_digest: self.checkpoint_object_digest.clone(),
            checkpoint_total_bytes: self.checkpoint_total_bytes,
            inject_failed_upload: self.inject_failed_upload,
            inject_eval_worker_unavailable: self.inject_eval_worker_unavailable,
            manifest_digest: None,
        };
        manifest
            .populate_manifest_digest()
            .map_err(map_runtime_contract_error)?;
        manifest
            .validate_machine_contract()
            .map_err(map_runtime_contract_error)?;
        Ok(manifest)
    }

    /// Computes the deterministic retained outputs for one packaged execution.
    pub fn expected_outputs(
        &self,
    ) -> Result<PsionActualPretrainingAutomaticExecutionOutputs, PsionActualPretrainingLauncherError>
    {
        let manifest = self.to_invocation_manifest()?;
        let retained_paths = psion_actual_pretraining_retained_paths();
        retained_paths.validate()?;
        let run_root = self.derived_run_root()?;
        let operation_manifest_path =
            operation_manifest_path(&run_root, &retained_paths, self.operation);
        let writes_actual_lane_surfaces = operation_writes_actual_lane_surfaces(self.operation);
        let window_plan =
            predict_psionic_train_window_artifacts(&manifest, self.run_id.as_str(), &run_root);
        let checkpoint_surface_path = run_root.join("status/checkpoint_surface.json");
        let checkpoint_handoff_receipt_path =
            run_root.join("status/peer_checkpoint_handoff_receipt.json");
        let checkpoint_manifest_path = self.optimizer_step.map(|optimizer_step| {
            run_root
                .join(format!(
                    "checkpoints/step-{optimizer_step}/checkpoint_manifest.json"
                ))
                .display()
                .to_string()
        });
        Ok(PsionActualPretrainingAutomaticExecutionOutputs {
            schema_version: String::from(
                PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_OUTPUTS_SCHEMA_VERSION,
            ),
            lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
            role: self.role,
            operation: self.operation,
            work_class: PsionicTrainWorkClass::FullIslandLocalUpdateTraining,
            run_id: self.run_id.clone(),
            run_root: run_root.display().to_string(),
            window_root: window_plan.as_ref().map(|value| value.window_root.clone()),
            operation_manifest_path: operation_manifest_path.clone(),
            current_status_path: writes_actual_lane_surfaces.then(|| {
                run_root
                    .join(&retained_paths.current_status_path)
                    .display()
                    .to_string()
            }),
            retained_summary_path: writes_actual_lane_surfaces.then(|| {
                run_root
                    .join(&retained_paths.retained_summary_path)
                    .display()
                    .to_string()
            }),
            launcher_log_path: writes_actual_lane_surfaces.then(|| {
                run_root
                    .join(&retained_paths.launcher_log_path)
                    .display()
                    .to_string()
            }),
            run_status_packet_path: run_root
                .join("status/psionic_train_run_status_packet.json")
                .display()
                .to_string(),
            window_status_packet_path: run_root
                .join("status/psionic_train_window_status_packet.json")
                .display()
                .to_string(),
            artifacts: PsionicTrainArtifactSurfaceRefs {
                launch_manifest_path: operation_manifest_path,
                membership_revision_path: Some(
                    run_root
                        .join("status/membership_revision_receipt.json")
                        .display()
                        .to_string(),
                ),
                window_execution_path: window_plan
                    .as_ref()
                    .map(|value| value.window_execution_path.clone()),
                contribution_receipt_path: window_plan
                    .as_ref()
                    .map(|value| value.contribution_receipt_path.clone()),
                contribution_artifact_manifest_path: window_plan
                    .as_ref()
                    .map(|value| value.contribution_artifact_manifest_path.clone()),
                grouped_stage_input_transport_path: None,
                grouped_stage_output_transport_path: None,
                grouped_stage_output_payload_path: None,
                grouped_stage_execution_summary_path: None,
                grouped_stage_replay_evidence_path: None,
                checkpoint_surface_path: Some(checkpoint_surface_path.display().to_string()),
                checkpoint_pointer_path: Some(
                    run_root
                        .join(&retained_paths.latest_checkpoint_pointer_path)
                        .display()
                        .to_string(),
                ),
                checkpoint_manifest_path,
                checkpoint_backup_receipt_path: matches!(
                    self.operation,
                    PsionicTrainOperation::Backup
                )
                .then(|| {
                    run_root
                        .join(&retained_paths.latest_checkpoint_backup_receipt_path)
                        .display()
                        .to_string()
                }),
                checkpoint_handoff_receipt_path: matches!(
                    self.operation,
                    PsionicTrainOperation::ServeCheckpoint
                )
                .then(|| checkpoint_handoff_receipt_path.display().to_string()),
                recovery_receipt_path: matches!(self.operation, PsionicTrainOperation::Resume)
                    .then(|| {
                        run_root
                            .join(&retained_paths.auto_resume_receipt_path)
                            .display()
                            .to_string()
                    }),
                validator_score_receipt_path: None,
                validator_quality_drift_signal_path: None,
                validator_rollback_signal_path: None,
                weak_device_validation_replay_proof_path: None,
                sealed_window_bundle_path: window_plan
                    .as_ref()
                    .map(|value| value.sealed_window_bundle_path.clone()),
                final_closeout_bundle_path: writes_actual_lane_surfaces.then(|| {
                    run_root
                        .join(&retained_paths.closeout_bundle_path)
                        .display()
                        .to_string()
                }),
            },
        })
    }

    fn validate_request_fields(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "automatic_execution_request.schema_version",
            PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
        )?;
        if self.role == PsionicTrainRole::Validator {
            return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                field: String::from("automatic_execution_request.role"),
                detail: String::from(
                    "actual-pretraining automatic execution is reserved for worker and recovery_source roles",
                ),
            });
        }
        if self.operation == PsionicTrainOperation::ValidateContribution {
            return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                field: String::from("automatic_execution_request.operation"),
                detail: String::from(
                    "actual-pretraining automatic execution does not package validator replay operations",
                ),
            });
        }
        ensure_nonempty(
            self.build_digest.as_str(),
            "automatic_execution_request.build_digest",
        )?;
        ensure_nonempty(self.run_id.as_str(), "automatic_execution_request.run_id")?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "automatic_execution_request.selected_git_ref",
        )?;
        self.coordination
            .validate("automatic_execution_request.coordination")
            .map_err(
                |error| PsionActualPretrainingLauncherError::NestedValidation {
                    field: String::from("automatic_execution_request.coordination"),
                    detail: error.to_string(),
                },
            )?;
        if let Some(peer_checkpoint_handoff_receipt) = self.peer_checkpoint_handoff_receipt.as_ref()
        {
            peer_checkpoint_handoff_receipt
                .validate("automatic_execution_request.peer_checkpoint_handoff_receipt")
                .map_err(
                    |detail| PsionActualPretrainingLauncherError::NestedValidation {
                        field: String::from(
                            "automatic_execution_request.peer_checkpoint_handoff_receipt",
                        ),
                        detail,
                    },
                )?;
        }
        Ok(())
    }

    fn derived_run_root(&self) -> Result<PathBuf, PsionActualPretrainingLauncherError> {
        match self.operation {
            PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane => {
                Ok(PathBuf::from(self.output_root.as_deref().ok_or_else(
                    || PsionActualPretrainingLauncherError::MissingField {
                        field: String::from("automatic_execution_request.output_root"),
                    },
                )?))
            }
            _ => Ok(PathBuf::from(self.run_root.as_deref().ok_or_else(
                || PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("automatic_execution_request.run_root"),
                },
            )?)),
        }
    }
}

impl PsionActualPretrainingRetainedPathSet {
    /// Validates the fixed retained paths.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.launch_manifest_path.as_str(),
            "retained_paths.launch_manifest_path",
            "manifests/launch_manifest.json",
        )?;
        ensure_exact(
            self.resume_manifest_path.as_str(),
            "retained_paths.resume_manifest_path",
            "manifests/resume_manifest.json",
        )?;
        ensure_exact(
            self.current_status_path.as_str(),
            "retained_paths.current_status_path",
            "status/current_run_status.json",
        )?;
        ensure_exact(
            self.retained_summary_path.as_str(),
            "retained_paths.retained_summary_path",
            "status/retained_summary.json",
        )?;
        ensure_exact(
            self.latest_checkpoint_pointer_path.as_str(),
            "retained_paths.latest_checkpoint_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_exact(
            self.latest_checkpoint_backup_receipt_path.as_str(),
            "retained_paths.latest_checkpoint_backup_receipt_path",
            "checkpoints/latest_accepted_checkpoint_backup_receipt.json",
        )?;
        ensure_exact(
            self.auto_resume_receipt_path.as_str(),
            "retained_paths.auto_resume_receipt_path",
            "checkpoints/auto_resume_receipt.json",
        )?;
        ensure_exact(
            self.latest_checkpoint_eval_decision_path.as_str(),
            "retained_paths.latest_checkpoint_eval_decision_path",
            "evals/latest_checkpoint_eval_decision.json",
        )?;
        ensure_exact(
            self.latest_checkpoint_eval_failure_path.as_str(),
            "retained_paths.latest_checkpoint_eval_failure_path",
            "evals/latest_checkpoint_eval_failure.json",
        )?;
        ensure_exact(
            self.latest_checkpoint_comparison_path.as_str(),
            "retained_paths.latest_checkpoint_comparison_path",
            "decisions/latest_checkpoint_comparison.json",
        )?;
        ensure_exact(
            self.latest_continue_restart_decision_path.as_str(),
            "retained_paths.latest_continue_restart_decision_path",
            "decisions/latest_continue_restart_decision.json",
        )?;
        ensure_exact(
            self.current_dashboard_path.as_str(),
            "retained_paths.current_dashboard_path",
            "dashboard/current_dashboard.json",
        )?;
        ensure_exact(
            self.hardware_qualification_path.as_str(),
            "retained_paths.hardware_qualification_path",
            "preflight/hardware_qualification.json",
        )?;
        ensure_exact(
            self.run_shape_qualification_path.as_str(),
            "retained_paths.run_shape_qualification_path",
            "preflight/run_shape_qualification.json",
        )?;
        ensure_exact(
            self.continuation_handoff_path.as_str(),
            "retained_paths.continuation_handoff_path",
            PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
        )?;
        ensure_exact(
            self.closeout_bundle_path.as_str(),
            "retained_paths.closeout_bundle_path",
            "closeout/closeout_bundle.json",
        )?;
        ensure_exact(
            self.launcher_log_path.as_str(),
            "retained_paths.launcher_log_path",
            "logs/launcher.log",
        )?;
        ensure_exact(
            self.latest_redacted_alert_path.as_str(),
            "retained_paths.latest_redacted_alert_path",
            "alerts/latest_redacted_alert.json",
        )?;
        ensure_exact(
            self.active_alert_feed_path.as_str(),
            "retained_paths.active_alert_feed_path",
            "alerts/active_alerts.json",
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingPreflightRef {
    /// Validates the retained preflight receipt reference.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        self.validate_for("preflight_receipt", "preflight/hardware_qualification.json")
    }

    /// Validates the retained preflight receipt reference for a specific path.
    pub fn validate_for(
        &self,
        field_prefix: &str,
        expected_relative_path: &str,
    ) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.relative_path.as_str(),
            &format!("{field_prefix}.relative_path"),
            expected_relative_path,
        )?;
        ensure_nonempty(
            self.receipt_digest.as_str(),
            &format!("{field_prefix}.receipt_digest"),
        )?;
        match self.admission_state.as_str() {
            "admitted" | "refused" => Ok(()),
            _ => Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                field: format!("{field_prefix}.admission_state"),
                detail: String::from("preflight receipt must be admitted or refused"),
            }),
        }
    }
}

impl PsionActualPretrainingLauncherContractRefs {
    /// Validates the committed contract refs.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_artifact_ref(&self.lane_spec, "contract_refs.lane_spec")?;
        ensure_artifact_ref(&self.recipe_bundle, "contract_refs.recipe_bundle")?;
        ensure_artifact_ref(
            &self.baseline_tools_bundle,
            "contract_refs.baseline_tools_bundle",
        )?;
        ensure_artifact_ref(&self.scaling_bundle, "contract_refs.scaling_bundle")?;
        ensure_artifact_ref(&self.data_bundle, "contract_refs.data_bundle")?;
        ensure_artifact_ref(&self.systems_bundle, "contract_refs.systems_bundle")?;
        ensure_artifact_ref(
            &self.topology_storage_bundle,
            "contract_refs.topology_storage_bundle",
        )?;
        ensure_artifact_ref(&self.evidence_contract, "contract_refs.evidence_contract")?;
        Ok(())
    }
}

impl PsionActualPretrainingRunRoots {
    /// Validates the selected run roots.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_nonempty(self.local_run_root.as_str(), "run_roots.local_run_root")?;
        ensure_nonempty(self.remote_run_root.as_str(), "run_roots.remote_run_root")?;
        ensure_nonempty(
            self.remote_checkpoint_root.as_str(),
            "run_roots.remote_checkpoint_root",
        )?;
        ensure_nonempty(
            self.remote_manifest_root.as_str(),
            "run_roots.remote_manifest_root",
        )?;
        ensure_nonempty(self.remote_log_root.as_str(), "run_roots.remote_log_root")?;
        Ok(())
    }
}

impl PsionActualPretrainingLaunchManifest {
    /// Validates the actual-lane launch manifest.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "launch_manifest.schema_version",
            PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION,
        )?;
        if self.surface_id != PSION_ACTUAL_PRETRAINING_START_SURFACE_ID
            && self.surface_id != PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID
        {
            return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                field: String::from("launch_manifest.surface_id"),
                detail: String::from(
                    "launch manifest surface_id must be the start or dry-run surface",
                ),
            });
        }
        validate_launcher_common(
            self.lane_id.as_str(),
            self.recipe_id.as_str(),
            self.topology_storage_bundle_id.as_str(),
            self.evidence_contract_id.as_str(),
            self.run_id.as_str(),
            &self.retained_paths,
            &self.launcher_surfaces,
            &self.run_roots,
            &self.preflight_receipt,
            &self.run_shape_receipt,
            &self.contract_refs,
            self.selected_git_ref.as_str(),
            self.git_commit_sha.as_str(),
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            &self.credential_sources,
            self.claim_boundary.as_str(),
            self.detail.as_str(),
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingResumeManifest {
    /// Validates the actual-lane resume manifest.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "resume_manifest.schema_version",
            PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.surface_id.as_str(),
            "resume_manifest.surface_id",
            PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
        )?;
        validate_launcher_common(
            self.lane_id.as_str(),
            self.recipe_id.as_str(),
            self.topology_storage_bundle_id.as_str(),
            self.evidence_contract_id.as_str(),
            self.run_id.as_str(),
            &self.retained_paths,
            &self.launcher_surfaces,
            &self.run_roots,
            &self.preflight_receipt,
            &self.run_shape_receipt,
            &self.contract_refs,
            self.selected_git_ref.as_str(),
            self.git_commit_sha.as_str(),
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            &[],
            self.claim_boundary.as_str(),
            self.detail.as_str(),
        )?;
        ensure_exact(
            self.latest_checkpoint_pointer_path.as_str(),
            "resume_manifest.latest_checkpoint_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_nonempty(
            self.checkpoint_label.as_str(),
            "resume_manifest.checkpoint_label",
        )?;
        if self.optimizer_step == 0 {
            return Err(PsionActualPretrainingLauncherError::MissingField {
                field: String::from("resume_manifest.optimizer_step"),
            });
        }
        ensure_nonempty(
            self.checkpoint_ref.as_str(),
            "resume_manifest.checkpoint_ref",
        )?;
        Ok(())
    }
}

impl PsionActualPretrainingCheckpointPointer {
    /// Validates the actual-lane checkpoint pointer.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "checkpoint_pointer.schema_version",
            PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "checkpoint_pointer.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "checkpoint_pointer.run_id")?;
        ensure_nonempty(
            self.checkpoint_label.as_str(),
            "checkpoint_pointer.checkpoint_label",
        )?;
        ensure_nonempty(self.detail.as_str(), "checkpoint_pointer.detail")?;
        match self.pointer_state.as_str() {
            "pending_first_checkpoint" => {
                if self.optimizer_step != 0 {
                    return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                        field: String::from("checkpoint_pointer.optimizer_step"),
                        detail: String::from(
                            "pending_first_checkpoint pointers must retain optimizer_step 0",
                        ),
                    });
                }
                if self.checkpoint_ref.is_some() {
                    return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                        field: String::from("checkpoint_pointer.checkpoint_ref"),
                        detail: String::from(
                            "pending_first_checkpoint pointers must not retain a checkpoint ref",
                        ),
                    });
                }
                if self.checkpoint_manifest_relative_path.is_some() {
                    return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                        field: String::from("checkpoint_pointer.checkpoint_manifest_relative_path"),
                        detail: String::from(
                            "pending_first_checkpoint pointers must not retain a checkpoint manifest path",
                        ),
                    });
                }
            }
            "accepted" => {
                if self.optimizer_step == 0 {
                    return Err(PsionActualPretrainingLauncherError::MissingField {
                        field: String::from("checkpoint_pointer.optimizer_step"),
                    });
                }
                ensure_nonempty_option(
                    self.checkpoint_ref.as_deref(),
                    "checkpoint_pointer.checkpoint_ref",
                )?;
                ensure_nonempty_option(
                    self.checkpoint_manifest_relative_path.as_deref(),
                    "checkpoint_pointer.checkpoint_manifest_relative_path",
                )?;
            }
            _ => {
                return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
                    field: String::from("checkpoint_pointer.pointer_state"),
                    detail: String::from(
                        "checkpoint pointer state must be pending_first_checkpoint or accepted",
                    ),
                });
            }
        }
        Ok(())
    }
}

impl PsionActualPretrainingCloseoutBundle {
    /// Validates the actual-lane provisional closeout bundle.
    pub fn validate(&self) -> Result<(), PsionActualPretrainingLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "closeout_bundle.schema_version",
            PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
        )?;
        ensure_exact(
            self.lane_id.as_str(),
            "closeout_bundle.lane_id",
            PSION_ACTUAL_PRETRAINING_LANE_ID,
        )?;
        ensure_nonempty(self.run_id.as_str(), "closeout_bundle.run_id")?;
        ensure_nonempty(
            self.closeout_state.as_str(),
            "closeout_bundle.closeout_state",
        )?;
        self.retained_paths.validate()?;
        ensure_nonempty(
            self.selected_git_ref.as_str(),
            "closeout_bundle.selected_git_ref",
        )?;
        ensure_git_sha(
            self.git_commit_sha.as_str(),
            "closeout_bundle.git_commit_sha",
        )?;
        ensure_dirty_tree_admission(
            self.dirty_tree_admission.as_str(),
            self.workspace_status_sha256.as_deref(),
            "closeout_bundle",
        )?;
        for artifact in &self.evidence_artifacts {
            ensure_nonempty(
                artifact.artifact_kind.as_str(),
                "closeout_bundle.evidence_artifacts[].artifact_kind",
            )?;
            ensure_artifact_ref(
                &artifact.artifact,
                "closeout_bundle.evidence_artifacts[].artifact",
            )?;
            ensure_nonempty(
                artifact.detail.as_str(),
                "closeout_bundle.evidence_artifacts[].detail",
            )?;
        }
        for gate in &self.closeout_gates {
            ensure_nonempty(
                gate.gate_id.as_str(),
                "closeout_bundle.closeout_gates[].gate_id",
            )?;
            ensure_nonempty(
                gate.detail.as_str(),
                "closeout_bundle.closeout_gates[].detail",
            )?;
        }
        for drill in &self.failure_drills {
            ensure_nonempty(
                drill.drill_id.as_str(),
                "closeout_bundle.failure_drills[].drill_id",
            )?;
            ensure_nonempty(
                drill.resolution_state.as_str(),
                "closeout_bundle.failure_drills[].resolution_state",
            )?;
            ensure_artifact_ref(&drill.artifact, "closeout_bundle.failure_drills[].artifact")?;
            ensure_nonempty(
                drill.detail.as_str(),
                "closeout_bundle.failure_drills[].detail",
            )?;
        }
        for claim in &self.can_now_claim {
            ensure_nonempty(claim.as_str(), "closeout_bundle.can_now_claim[]")?;
        }
        for item in &self.still_out_of_scope {
            ensure_nonempty(item.as_str(), "closeout_bundle.still_out_of_scope[]")?;
        }
        if self.closeout_state == "base_lane_rehearsal_complete" {
            if self.evidence_artifacts.is_empty() {
                return Err(PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("closeout_bundle.evidence_artifacts"),
                });
            }
            if self.closeout_gates.is_empty() {
                return Err(PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("closeout_bundle.closeout_gates"),
                });
            }
            if self.failure_drills.is_empty() {
                return Err(PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("closeout_bundle.failure_drills"),
                });
            }
            if self.can_now_claim.is_empty() {
                return Err(PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("closeout_bundle.can_now_claim"),
                });
            }
            if self.still_out_of_scope.is_empty() {
                return Err(PsionActualPretrainingLauncherError::MissingField {
                    field: String::from("closeout_bundle.still_out_of_scope"),
                });
            }
            for required_kind in [
                "launch_manifest",
                "hardware_qualification",
                "run_shape_qualification",
                "checkpoint_pointer",
                "checkpoint_manifest",
                "checkpoint_backup_receipt",
                "checkpoint_eval_decision",
                "checkpoint_comparison",
                "continue_restart_decision",
                "auto_resume_receipt",
                "resume_manifest",
                "retained_summary",
                "current_status",
                "dashboard_packet",
                "active_alert_feed",
                "continuation_handoff",
            ] {
                if !self
                    .evidence_artifacts
                    .iter()
                    .any(|artifact| artifact.artifact_kind == required_kind)
                {
                    return Err(PsionActualPretrainingLauncherError::MissingField {
                        field: format!("closeout_bundle.evidence_artifacts[{required_kind}]"),
                    });
                }
            }
            for required_gate in [
                "launch_preflight_admitted",
                "accepted_checkpoint_retained",
                "automatic_checkpoint_eval_retained",
                "checkpoint_backup_success_retained",
                "continue_decision_retained",
                "resume_manifest_retained",
                "failure_drill_retained",
            ] {
                if !self
                    .closeout_gates
                    .iter()
                    .any(|gate| gate.gate_id == required_gate)
                {
                    return Err(PsionActualPretrainingLauncherError::MissingField {
                        field: format!("closeout_bundle.closeout_gates[{required_gate}]"),
                    });
                }
            }
        }
        ensure_nonempty(
            self.claim_boundary.as_str(),
            "closeout_bundle.claim_boundary",
        )?;
        ensure_nonempty(self.detail.as_str(), "closeout_bundle.detail")?;
        Ok(())
    }
}

fn validate_launcher_common(
    lane_id: &str,
    recipe_id: &str,
    topology_storage_bundle_id: &str,
    evidence_contract_id: &str,
    run_id: &str,
    retained_paths: &PsionActualPretrainingRetainedPathSet,
    launcher_surfaces: &PsionActualPretrainingLauncherSurfaces,
    run_roots: &PsionActualPretrainingRunRoots,
    preflight_receipt: &PsionActualPretrainingPreflightRef,
    run_shape_receipt: &PsionActualPretrainingPreflightRef,
    contract_refs: &PsionActualPretrainingLauncherContractRefs,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    credential_sources: &[PsionActualPretrainingCredentialBinding],
    claim_boundary: &str,
    detail: &str,
) -> Result<(), PsionActualPretrainingLauncherError> {
    ensure_exact(lane_id, "lane_id", PSION_ACTUAL_PRETRAINING_LANE_ID)?;
    ensure_exact(recipe_id, "recipe_id", PSION_ACTUAL_PRETRAINING_RECIPE_ID)?;
    ensure_exact(
        topology_storage_bundle_id,
        "topology_storage_bundle_id",
        PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
    )?;
    ensure_exact(
        evidence_contract_id,
        "evidence_contract_id",
        PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID,
    )?;
    ensure_nonempty(run_id, "run_id")?;
    retained_paths.validate()?;
    launcher_surfaces.validate().map_err(|error| {
        PsionActualPretrainingLauncherError::NestedValidation {
            field: String::from("launcher_surfaces"),
            detail: error.to_string(),
        }
    })?;
    run_roots.validate()?;
    preflight_receipt.validate()?;
    run_shape_receipt.validate_for(
        "run_shape_receipt",
        "preflight/run_shape_qualification.json",
    )?;
    contract_refs.validate()?;
    ensure_nonempty(selected_git_ref, "selected_git_ref")?;
    ensure_git_sha(git_commit_sha, "git_commit_sha")?;
    ensure_dirty_tree_admission(
        dirty_tree_admission,
        workspace_status_sha256,
        "dirty_tree_admission",
    )?;
    if !credential_sources.is_empty() {
        for credential in credential_sources {
            ensure_nonempty(credential.kind.as_str(), "credential_source.kind")?;
            ensure_nonempty(
                credential.source_name.as_str(),
                "credential_source.source_name",
            )?;
            ensure_nonempty(
                credential.retained_redaction.as_str(),
                "credential_source.retained_redaction",
            )?;
        }
    }
    ensure_nonempty(claim_boundary, "claim_boundary")?;
    ensure_nonempty(detail, "detail")?;
    Ok(())
}

fn ensure_artifact_ref(
    artifact: &PsionActualPretrainingArtifactRef,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingLauncherError> {
    ensure_nonempty(artifact.path.as_str(), &format!("{field_prefix}.path"))?;
    ensure_nonempty(artifact.sha256.as_str(), &format!("{field_prefix}.sha256"))?;
    Ok(())
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionActualPretrainingLauncherError> {
    if actual != expected {
        return Err(PsionActualPretrainingLauncherError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionActualPretrainingLauncherError> {
    if value.trim().is_empty() {
        return Err(PsionActualPretrainingLauncherError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn ensure_nonempty_option(
    value: Option<&str>,
    field: &str,
) -> Result<(), PsionActualPretrainingLauncherError> {
    match value {
        Some(value) => ensure_nonempty(value, field),
        None => Err(PsionActualPretrainingLauncherError::MissingField {
            field: String::from(field),
        }),
    }
}

fn ensure_git_sha(value: &str, field: &str) -> Result<(), PsionActualPretrainingLauncherError> {
    ensure_nonempty(value, field)?;
    if value.len() != 40 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(PsionActualPretrainingLauncherError::UnsupportedValue {
            field: String::from(field),
            detail: String::from("git commit SHA must be a 40-character hex string"),
        });
    }
    Ok(())
}

fn ensure_dirty_tree_admission(
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<&str>,
    field_prefix: &str,
) -> Result<(), PsionActualPretrainingLauncherError> {
    match dirty_tree_admission {
        "refuse_by_default" => Ok(()),
        "allowed_by_operator_override" => ensure_nonempty_option(
            workspace_status_sha256,
            &format!("{field_prefix}.workspace_status_sha256"),
        ),
        _ => Err(PsionActualPretrainingLauncherError::UnsupportedValue {
            field: String::from(field_prefix),
            detail: String::from(
                "dirty-tree admission must be refuse_by_default or allowed_by_operator_override",
            ),
        }),
    }
}

fn operation_manifest_path(
    run_root: &PathBuf,
    retained_paths: &PsionActualPretrainingRetainedPathSet,
    operation: PsionicTrainOperation,
) -> Option<String> {
    match operation {
        PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane => Some(
            run_root
                .join(&retained_paths.launch_manifest_path)
                .display()
                .to_string(),
        ),
        PsionicTrainOperation::Resume => Some(
            run_root
                .join(&retained_paths.resume_manifest_path)
                .display()
                .to_string(),
        ),
        _ => None,
    }
}

fn operation_writes_actual_lane_surfaces(operation: PsionicTrainOperation) -> bool {
    !matches!(operation, PsionicTrainOperation::ServeCheckpoint)
}

fn map_runtime_contract_error(
    error: PsionicTrainRuntimeContractError,
) -> PsionActualPretrainingLauncherError {
    match error {
        PsionicTrainRuntimeContractError::MissingField { field } => {
            PsionActualPretrainingLauncherError::MissingField { field }
        }
        PsionicTrainRuntimeContractError::FieldMismatch {
            field,
            expected,
            actual,
        } => PsionActualPretrainingLauncherError::FieldMismatch {
            field,
            expected,
            actual,
        },
        PsionicTrainRuntimeContractError::InvalidValue { field, detail } => {
            PsionActualPretrainingLauncherError::UnsupportedValue { field, detail }
        }
    }
}

/// Validation errors for the actual-lane launcher surfaces.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PsionActualPretrainingLauncherError {
    #[error("psion actual-pretraining launcher field `{field}` must not be empty")]
    MissingField { field: String },
    #[error(
        "psion actual-pretraining launcher field `{field}` mismatch: expected `{expected}`, got `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("psion actual-pretraining launcher field `{field}` is unsupported: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("psion actual-pretraining launcher nested validation for `{field}` failed: {detail}")]
    NestedValidation { field: String, detail: String },
}

#[cfg(test)]
mod tests {
    use super::{
        psion_actual_pretraining_retained_paths, PsionActualPretrainingAutomaticExecutionRequest,
        PsionActualPretrainingCheckpointPointer, PsionActualPretrainingCloseoutBundle,
        PsionActualPretrainingLaunchManifest, PsionActualPretrainingResumeManifest,
        PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
    };
    use crate::{
        PsionicTrainArtifactBinding, PsionicTrainArtifactRef, PsionicTrainCoordinationContext,
        PsionicTrainOperation, PsionicTrainRole, PsionicTrainWorkClass,
    };

    fn launch_manifest() -> PsionActualPretrainingLaunchManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_launch_manifest_v1.json"
        ))
        .expect("actual pretraining launch manifest fixture should parse")
    }

    fn resume_manifest() -> PsionActualPretrainingResumeManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_resume_manifest_v1.json"
        ))
        .expect("actual pretraining resume manifest fixture should parse")
    }

    fn checkpoint_pointer() -> PsionActualPretrainingCheckpointPointer {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_checkpoint_pointer_v1.json"
        ))
        .expect("actual pretraining checkpoint pointer fixture should parse")
    }

    fn closeout_bundle() -> PsionActualPretrainingCloseoutBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_actual_pretraining_closeout_bundle_v1.json"
        ))
        .expect("actual pretraining closeout bundle fixture should parse")
    }

    fn automatic_execution_request() -> PsionActualPretrainingAutomaticExecutionRequest {
        PsionActualPretrainingAutomaticExecutionRequest {
            schema_version: String::from(
                PSION_ACTUAL_PRETRAINING_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
            ),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("network.psion.trainnet-a")),
                window_id: Some(String::from("window-0172")),
                assignment_id: Some(String::from("assignment-0044")),
                challenge_id: None,
                node_pubkey: Some(String::from("npub1-strong-node")),
                membership_revision: Some(9),
            },
            build_digest: String::from("sha256:test-build"),
            run_id: String::from("psion-r172"),
            output_root: Some(String::from("/tmp/psion-r172")),
            run_root: None,
            selected_git_ref: String::from("refs/heads/main"),
            hardware_observation_path: Some(String::from("preflight/hardware_observation.json")),
            run_shape_observation_path: Some(String::from("preflight/run_shape_observation.json")),
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            allow_dirty_tree: false,
            dry_run: false,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
        }
    }

    #[test]
    fn actual_pretraining_launch_manifest_fixture_validates() {
        launch_manifest()
            .validate()
            .expect("actual pretraining launch manifest fixture should validate");
    }

    #[test]
    fn actual_pretraining_resume_manifest_fixture_validates() {
        resume_manifest()
            .validate()
            .expect("actual pretraining resume manifest fixture should validate");
    }

    #[test]
    fn actual_pretraining_checkpoint_pointer_fixture_validates() {
        checkpoint_pointer()
            .validate()
            .expect("actual pretraining checkpoint pointer fixture should validate");
    }

    #[test]
    fn actual_pretraining_closeout_bundle_fixture_validates() {
        closeout_bundle()
            .validate()
            .expect("actual pretraining closeout bundle fixture should validate");
    }

    #[test]
    fn actual_pretraining_automatic_execution_request_builds_start_manifest_and_outputs() {
        let request = automatic_execution_request();
        let manifest = request
            .to_invocation_manifest()
            .expect("automatic execution request should build a machine manifest");
        assert_eq!(manifest.lane_id, crate::PSION_ACTUAL_PRETRAINING_LANE_ID);
        assert_eq!(
            manifest.work_class,
            PsionicTrainWorkClass::FullIslandLocalUpdateTraining
        );
        assert_eq!(
            manifest.admission_identity.release_id.as_str(),
            crate::PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID
        );
        assert_eq!(
            manifest.admission_identity.environment_ref.as_str(),
            crate::PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF
        );
        let outputs = request
            .expected_outputs()
            .expect("automatic execution request should plan retained outputs");
        let retained_paths = psion_actual_pretraining_retained_paths();
        assert_eq!(outputs.run_root, "/tmp/psion-r172");
        assert_eq!(
            outputs.operation_manifest_path.as_deref(),
            Some("/tmp/psion-r172/manifests/launch_manifest.json")
        );
        assert_eq!(
            outputs.current_status_path.as_deref(),
            Some("/tmp/psion-r172/status/current_run_status.json")
        );
        assert_eq!(
            outputs.artifacts.checkpoint_pointer_path.as_deref(),
            Some("/tmp/psion-r172/checkpoints/latest_accepted_checkpoint_pointer.json")
        );
        assert_eq!(
            outputs.artifacts.sealed_window_bundle_path.as_deref(),
            Some("/tmp/psion-r172/windows/window-0172/sealed_window_bundle.json")
        );
        assert_eq!(
            outputs.artifacts.final_closeout_bundle_path.as_deref(),
            Some("/tmp/psion-r172/closeout/closeout_bundle.json")
        );
        assert_eq!(
            retained_paths.launch_manifest_path.as_str(),
            "manifests/launch_manifest.json"
        );
    }

    #[test]
    fn actual_pretraining_automatic_execution_request_accepts_resolver_backed_resume_handoff() {
        let mut request = automatic_execution_request();
        request.role = PsionicTrainRole::RecoverySource;
        request.operation = PsionicTrainOperation::Resume;
        request.output_root = None;
        request.run_root = Some(String::from("/tmp/psion-r172"));
        request.peer_checkpoint_handoff_receipt = Some(PsionicTrainArtifactBinding {
            artifact_ref: PsionicTrainArtifactRef {
                artifact_id: String::from("artifact://handoff/agg-r171"),
                artifact_digest: Some(String::from("sha256:handoff")),
                artifact_bytes: Some(4096),
            },
            materialized_path: None,
        });

        let manifest = request
            .to_invocation_manifest()
            .expect("resume request should build a machine manifest");
        assert!(manifest.peer_checkpoint_handoff_receipt.is_some());
        let outputs = request
            .expected_outputs()
            .expect("resume request should plan retained outputs");
        assert_eq!(
            outputs.operation_manifest_path.as_deref(),
            Some("/tmp/psion-r172/manifests/resume_manifest.json")
        );
        assert_eq!(
            outputs.artifacts.recovery_receipt_path.as_deref(),
            Some("/tmp/psion-r172/checkpoints/auto_resume_receipt.json")
        );
        assert_eq!(outputs.role, PsionicTrainRole::RecoverySource);
    }

    #[test]
    fn actual_pretraining_resume_manifest_rejects_pending_pointer_step() {
        let mut manifest = resume_manifest();
        manifest.optimizer_step = 0;
        let error = manifest
            .validate()
            .expect_err("resume manifest must reject zero-step resume");
        assert_eq!(
            error,
            super::PsionActualPretrainingLauncherError::MissingField {
                field: String::from("resume_manifest.optimizer_step"),
            }
        );
    }
}
