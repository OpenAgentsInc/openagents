use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH, Cs336A1ReferenceTrainingConfig,
    PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF, PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID,
    PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
    PsionicTrainAdmissionIdentity, PsionicTrainArtifactSurfaceRefs,
    PsionicTrainCoordinationContext, PsionicTrainInvocationManifest, PsionicTrainOperation,
    PsionicTrainRole, PsionicTrainRuntimeContractError, PsionicTrainWorkClass,
    predict_psionic_train_window_artifacts,
};

pub const PSION_CS336_A1_DEMO_LANE_ID: &str = "psion_cs336_a1_demo_v1";
pub const PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_automatic_execution_request.v1";
pub const PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_OUTPUTS_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_automatic_execution_outputs.v1";
pub const PSION_CS336_A1_DEMO_LAUNCH_MANIFEST_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_launch_manifest.v1";
pub const PSION_CS336_A1_DEMO_CURRENT_RUN_STATUS_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_current_run_status.v1";
pub const PSION_CS336_A1_DEMO_RETAINED_SUMMARY_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_retained_summary.v1";
pub const PSION_CS336_A1_DEMO_CLOSEOUT_BUNDLE_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_closeout_bundle.v1";
pub const PSION_CS336_A1_DEMO_START_SURFACE_ID: &str = "psion.cs336_a1_demo.start.v1";
pub const PSION_CS336_A1_DEMO_REHEARSAL_SURFACE_ID: &str =
    "psion.cs336_a1_demo.rehearse_base_lane.v1";
pub const PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT: u64 = 4;
pub const PSION_CS336_A1_DEMO_CHECKPOINT_LABEL: &str = "bounded_step_000004";
pub const PSION_CS336_A1_DEMO_CLAIM_BOUNDARY: &str = "This lane packages the tiny bounded CS336 A1 port into the shared Pylon machine-runtime contract: byte-level BPE, the owned A1 reference stack, four deterministic tiny training steps, one retained accepted checkpoint, and one closeout bundle. It does not claim broader-pretraining scale, production autograd closure, or that the bounded demo lane is the actual Psion pretraining lane.";

#[derive(Debug, Error)]
pub enum PsionCs336A1DemoLauncherError {
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("unsupported value for `{field}`: {detail}")]
    UnsupportedValue { field: String, detail: String },
    #[error("invalid `{field}`: {detail}")]
    NestedValidation { field: String, detail: String },
    #[error(transparent)]
    RuntimeContract(#[from] PsionicTrainRuntimeContractError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoAutomaticExecutionRequest {
    pub schema_version: String,
    pub role: PsionicTrainRole,
    pub operation: PsionicTrainOperation,
    #[serde(default)]
    pub coordination: PsionicTrainCoordinationContext,
    pub build_digest: String,
    pub run_id: String,
    pub output_root: Option<String>,
    pub run_root: Option<String>,
    pub selected_git_ref: String,
    #[serde(default)]
    pub allow_dirty_tree: bool,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoAutomaticExecutionOutputs {
    pub schema_version: String,
    pub lane_id: String,
    pub role: PsionicTrainRole,
    pub operation: PsionicTrainOperation,
    pub work_class: PsionicTrainWorkClass,
    pub run_id: String,
    pub run_root: String,
    pub window_root: Option<String>,
    pub operation_manifest_path: Option<String>,
    pub current_status_path: Option<String>,
    pub retained_summary_path: Option<String>,
    pub launcher_log_path: Option<String>,
    pub run_status_packet_path: String,
    pub window_status_packet_path: String,
    pub artifacts: PsionicTrainArtifactSurfaceRefs,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoRetainedPathSet {
    pub launch_manifest_path: String,
    pub current_status_path: String,
    pub retained_summary_path: String,
    pub checkpoint_pointer_path: String,
    pub checkpoint_manifest_path: String,
    pub checkpoint_payload_path: String,
    pub closeout_bundle_path: String,
    pub launcher_log_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoLaunchManifest {
    pub schema_version: String,
    pub surface_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub retained_paths: PsionCs336A1DemoRetainedPathSet,
    pub corpus_fixture_path: String,
    pub training_config: Cs336A1ReferenceTrainingConfig,
    pub training_step_count: u64,
    pub selected_git_ref: String,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoCurrentRunStatus {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub phase: String,
    pub completed_steps: u64,
    pub total_steps: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_ref: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoRetainedSummary {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub claim_boundary: String,
    pub corpus_fixture_path: String,
    pub training_config: Cs336A1ReferenceTrainingConfig,
    pub total_steps: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_loss: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_state_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizer_state_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_digest: Option<String>,
    pub detail: String,
}

impl PsionCs336A1DemoAutomaticExecutionRequest {
    pub fn validate(&self) -> Result<(), PsionCs336A1DemoLauncherError> {
        self.to_invocation_manifest().map(|_| ())
    }

    pub fn to_invocation_manifest(
        &self,
    ) -> Result<PsionicTrainInvocationManifest, PsionCs336A1DemoLauncherError> {
        self.validate_request_fields()?;
        let mut manifest = PsionicTrainInvocationManifest {
            schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
            runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
            lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
            role: self.role,
            operation: self.operation,
            work_class: PsionicTrainWorkClass::SmallModelLocalTraining,
            coordination: self.coordination.clone(),
            grouped_stage_assignment: None,
            admission_identity: PsionicTrainAdmissionIdentity {
                release_id: String::from(PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID),
                build_digest: self.build_digest.clone(),
                environment_ref: String::from(PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF),
            },
            run_id: Some(self.run_id.clone()),
            output_root: self.output_root.clone(),
            run_root: self.run_root.clone(),
            peer_node_pubkey: None,
            peer_checkpoint_handoff_receipt: None,
            validator_target_contribution_receipt: None,
            validator_target_contribution_artifact_manifest: None,
            validator_target_work_class: None,
            grouped_stage_input_transport: None,
            selected_git_ref: Some(self.selected_git_ref.clone()),
            hardware_observation_path: None,
            run_shape_observation_path: None,
            allow_dirty_tree: self.allow_dirty_tree,
            dry_run: self.dry_run,
            checkpoint_label: None,
            optimizer_step: None,
            checkpoint_ref: None,
            checkpoint_object_digest: None,
            checkpoint_total_bytes: None,
            inject_failed_upload: false,
            inject_eval_worker_unavailable: false,
            manifest_digest: None,
        };
        manifest.populate_manifest_digest()?;
        manifest.validate_machine_contract()?;
        Ok(manifest)
    }

    pub fn expected_outputs(
        &self,
    ) -> Result<PsionCs336A1DemoAutomaticExecutionOutputs, PsionCs336A1DemoLauncherError> {
        let manifest = self.to_invocation_manifest()?;
        let retained_paths = psion_cs336_a1_demo_retained_paths();
        retained_paths.validate()?;
        let run_root = self.derived_run_root()?;
        let window_plan =
            predict_psionic_train_window_artifacts(&manifest, self.run_id.as_str(), &run_root);
        let writes_checkpoint = !self.dry_run;
        Ok(PsionCs336A1DemoAutomaticExecutionOutputs {
            schema_version: String::from(
                PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_OUTPUTS_SCHEMA_VERSION,
            ),
            lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
            role: self.role,
            operation: self.operation,
            work_class: PsionicTrainWorkClass::SmallModelLocalTraining,
            run_id: self.run_id.clone(),
            run_root: run_root.display().to_string(),
            window_root: window_plan.as_ref().map(|value| value.window_root.clone()),
            operation_manifest_path: Some(
                run_root
                    .join(&retained_paths.launch_manifest_path)
                    .display()
                    .to_string(),
            ),
            current_status_path: Some(
                run_root
                    .join(&retained_paths.current_status_path)
                    .display()
                    .to_string(),
            ),
            retained_summary_path: Some(
                run_root
                    .join(&retained_paths.retained_summary_path)
                    .display()
                    .to_string(),
            ),
            launcher_log_path: Some(
                run_root
                    .join(&retained_paths.launcher_log_path)
                    .display()
                    .to_string(),
            ),
            run_status_packet_path: run_root
                .join("status/psionic_train_run_status_packet.json")
                .display()
                .to_string(),
            window_status_packet_path: run_root
                .join("status/psionic_train_window_status_packet.json")
                .display()
                .to_string(),
            artifacts: PsionicTrainArtifactSurfaceRefs {
                launch_manifest_path: Some(
                    run_root
                        .join(&retained_paths.launch_manifest_path)
                        .display()
                        .to_string(),
                ),
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
                checkpoint_surface_path: writes_checkpoint.then(|| {
                    run_root
                        .join("status/checkpoint_surface.json")
                        .display()
                        .to_string()
                }),
                checkpoint_pointer_path: writes_checkpoint.then(|| {
                    run_root
                        .join(&retained_paths.checkpoint_pointer_path)
                        .display()
                        .to_string()
                }),
                checkpoint_manifest_path: writes_checkpoint.then(|| {
                    run_root
                        .join(&retained_paths.checkpoint_manifest_path)
                        .display()
                        .to_string()
                }),
                checkpoint_backup_receipt_path: None,
                checkpoint_handoff_receipt_path: None,
                recovery_receipt_path: None,
                validator_score_receipt_path: None,
                validator_quality_drift_signal_path: None,
                validator_rollback_signal_path: None,
                weak_device_validation_replay_proof_path: None,
                sealed_window_bundle_path: window_plan
                    .as_ref()
                    .map(|value| value.sealed_window_bundle_path.clone()),
                final_closeout_bundle_path: writes_checkpoint.then(|| {
                    run_root
                        .join(&retained_paths.closeout_bundle_path)
                        .display()
                        .to_string()
                }),
            },
        })
    }

    fn validate_request_fields(&self) -> Result<(), PsionCs336A1DemoLauncherError> {
        ensure_exact(
            self.schema_version.as_str(),
            "automatic_execution_request.schema_version",
            PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
        )?;
        if self.role != PsionicTrainRole::Worker {
            return Err(PsionCs336A1DemoLauncherError::UnsupportedValue {
                field: String::from("automatic_execution_request.role"),
                detail: String::from("the bounded A1 demo lane is packaged only for worker runs"),
            });
        }
        if !matches!(
            self.operation,
            PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane
        ) {
            return Err(PsionCs336A1DemoLauncherError::UnsupportedValue {
                field: String::from("automatic_execution_request.operation"),
                detail: String::from(
                    "the bounded A1 demo lane currently packages only start and rehearse_base_lane",
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
            .map_err(|error| PsionCs336A1DemoLauncherError::NestedValidation {
                field: String::from("automatic_execution_request.coordination"),
                detail: error.to_string(),
            })?;
        ensure_nonempty_option(
            self.output_root.as_deref(),
            "automatic_execution_request.output_root",
        )?;
        if self.run_root.is_some() {
            return Err(PsionCs336A1DemoLauncherError::UnsupportedValue {
                field: String::from("automatic_execution_request.run_root"),
                detail: String::from(
                    "launch-style A1 demo operations use output_root instead of run_root",
                ),
            });
        }
        Ok(())
    }

    fn derived_run_root(&self) -> Result<PathBuf, PsionCs336A1DemoLauncherError> {
        Ok(PathBuf::from(self.output_root.as_deref().ok_or_else(
            || PsionCs336A1DemoLauncherError::MissingField {
                field: String::from("automatic_execution_request.output_root"),
            },
        )?))
    }
}

impl PsionCs336A1DemoRetainedPathSet {
    pub fn validate(&self) -> Result<(), PsionCs336A1DemoLauncherError> {
        ensure_exact(
            self.launch_manifest_path.as_str(),
            "retained_paths.launch_manifest_path",
            "manifests/launch_manifest.json",
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
            self.checkpoint_pointer_path.as_str(),
            "retained_paths.checkpoint_pointer_path",
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        )?;
        ensure_exact(
            self.checkpoint_manifest_path.as_str(),
            "retained_paths.checkpoint_manifest_path",
            "checkpoints/manifests/checkpoint_manifest_step-000004.json",
        )?;
        ensure_exact(
            self.checkpoint_payload_path.as_str(),
            "retained_paths.checkpoint_payload_path",
            "checkpoints/step-000004/cs336_a1_reference_checkpoint.json",
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
        Ok(())
    }
}

#[must_use]
pub fn psion_cs336_a1_demo_retained_paths() -> PsionCs336A1DemoRetainedPathSet {
    PsionCs336A1DemoRetainedPathSet {
        launch_manifest_path: String::from("manifests/launch_manifest.json"),
        current_status_path: String::from("status/current_run_status.json"),
        retained_summary_path: String::from("status/retained_summary.json"),
        checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        checkpoint_manifest_path: String::from(
            "checkpoints/manifests/checkpoint_manifest_step-000004.json",
        ),
        checkpoint_payload_path: String::from(
            "checkpoints/step-000004/cs336_a1_reference_checkpoint.json",
        ),
        closeout_bundle_path: String::from("closeout/closeout_bundle.json"),
        launcher_log_path: String::from("logs/launcher.log"),
    }
}

#[must_use]
pub fn build_psion_cs336_a1_demo_launch_manifest(
    run_id: impl Into<String>,
    selected_git_ref: impl Into<String>,
    rehearsal: bool,
) -> PsionCs336A1DemoLaunchManifest {
    PsionCs336A1DemoLaunchManifest {
        schema_version: String::from(PSION_CS336_A1_DEMO_LAUNCH_MANIFEST_SCHEMA_VERSION),
        surface_id: String::from(if rehearsal {
            PSION_CS336_A1_DEMO_REHEARSAL_SURFACE_ID
        } else {
            PSION_CS336_A1_DEMO_START_SURFACE_ID
        }),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: run_id.into(),
        retained_paths: psion_cs336_a1_demo_retained_paths(),
        corpus_fixture_path: String::from(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH),
        training_config: Cs336A1ReferenceTrainingConfig::tiny(),
        training_step_count: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        selected_git_ref: selected_git_ref.into(),
        claim_boundary: String::from(PSION_CS336_A1_DEMO_CLAIM_BOUNDARY),
    }
}

fn ensure_exact(
    actual: &str,
    field: &str,
    expected: &str,
) -> Result<(), PsionCs336A1DemoLauncherError> {
    if actual == expected {
        Ok(())
    } else {
        Err(PsionCs336A1DemoLauncherError::UnsupportedValue {
            field: String::from(field),
            detail: format!("expected `{expected}` but found `{actual}`"),
        })
    }
}

fn ensure_nonempty(actual: &str, field: &str) -> Result<(), PsionCs336A1DemoLauncherError> {
    if actual.trim().is_empty() {
        Err(PsionCs336A1DemoLauncherError::MissingField {
            field: String::from(field),
        })
    } else {
        Ok(())
    }
}

fn ensure_nonempty_option(
    actual: Option<&str>,
    field: &str,
) -> Result<(), PsionCs336A1DemoLauncherError> {
    ensure_nonempty(
        actual.ok_or_else(|| PsionCs336A1DemoLauncherError::MissingField {
            field: String::from(field),
        })?,
        field,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
        PSION_CS336_A1_DEMO_LANE_ID, PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        PsionCs336A1DemoAutomaticExecutionRequest, psion_cs336_a1_demo_retained_paths,
    };
    use crate::{
        PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF, PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID,
        PsionicTrainCoordinationContext, PsionicTrainOperation, PsionicTrainRole,
        PsionicTrainWorkClass,
    };

    fn automatic_execution_request() -> PsionCs336A1DemoAutomaticExecutionRequest {
        PsionCs336A1DemoAutomaticExecutionRequest {
            schema_version: String::from(
                PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
            ),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            coordination: PsionicTrainCoordinationContext {
                network_id: Some(String::from("net-demo")),
                window_id: Some(String::from("window-demo")),
                assignment_id: Some(String::from("assignment-demo")),
                challenge_id: None,
                node_pubkey: Some(String::from("npub1-demo-node")),
                membership_revision: Some(22),
            },
            build_digest: String::from("sha256:demo-build"),
            run_id: String::from("psion-cs336-a1-demo-r001"),
            output_root: Some(String::from("/tmp/psion-cs336-a1-demo-r001")),
            run_root: None,
            selected_git_ref: String::from("HEAD"),
            allow_dirty_tree: false,
            dry_run: false,
        }
    }

    #[test]
    fn request_builds_start_manifest_and_outputs() {
        let request = automatic_execution_request();
        let manifest = request
            .to_invocation_manifest()
            .expect("request should build an admitted manifest");
        assert_eq!(manifest.lane_id, PSION_CS336_A1_DEMO_LANE_ID);
        assert_eq!(
            manifest.work_class,
            PsionicTrainWorkClass::SmallModelLocalTraining
        );
        assert_eq!(
            manifest.admission_identity.release_id,
            PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID
        );
        assert_eq!(
            manifest.admission_identity.environment_ref,
            PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF
        );
        let outputs = request
            .expected_outputs()
            .expect("request should compute deterministic outputs");
        assert_eq!(outputs.run_root, "/tmp/psion-cs336-a1-demo-r001");
        assert_eq!(
            outputs.operation_manifest_path.as_deref(),
            Some("/tmp/psion-cs336-a1-demo-r001/manifests/launch_manifest.json")
        );
        assert_eq!(
            outputs.artifacts.checkpoint_manifest_path.as_deref(),
            Some(
                "/tmp/psion-cs336-a1-demo-r001/checkpoints/manifests/checkpoint_manifest_step-000004.json"
            )
        );
    }

    #[test]
    fn request_refuses_resume_packaging() {
        let mut request = automatic_execution_request();
        request.operation = PsionicTrainOperation::Resume;
        let error = request
            .to_invocation_manifest()
            .expect_err("resume should not yet be packaged for the bounded A1 lane");
        assert!(error.to_string().contains("start and rehearse_base_lane"));
    }

    #[test]
    fn retained_paths_stay_frozen() {
        let retained_paths = psion_cs336_a1_demo_retained_paths();
        retained_paths
            .validate()
            .expect("retained A1 demo paths should stay frozen");
        assert_eq!(
            PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT, 4,
            "the retained checkpoint path currently assumes one four-step bound"
        );
    }
}
