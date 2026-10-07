use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

use psionic_eval::{
    build_psion_actual_pretraining_checkpoint_eval_benchmark_package,
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH,
};
use psionic_train::{
    build_psion_actual_pretraining_dashboard_packet, checkpoint_comparison_relative_path,
    checkpoint_eval_decision_relative_path, checkpoint_eval_failure_relative_path,
    continue_restart_decision_relative_path,
    derive_psion_actual_pretraining_hardware_qualification,
    derive_psion_actual_pretraining_run_shape_qualification,
    psion_actual_pretraining_retained_paths, record_psion_actual_pretraining_auto_resume_receipt,
    record_psion_actual_pretraining_checkpoint_backup_receipt,
    record_psion_actual_pretraining_checkpoint_comparison,
    record_psion_actual_pretraining_checkpoint_eval_decision,
    record_psion_actual_pretraining_checkpoint_eval_failure,
    record_psion_actual_pretraining_checkpoint_failure_drill,
    record_psion_actual_pretraining_checkpoint_manifest,
    record_psion_actual_pretraining_continuation_handoff,
    record_psion_actual_pretraining_continue_restart_decision,
    record_psion_actual_pretraining_redacted_alert, PsionActualPretrainingAlertFeed,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingAutoResumeReceipt,
    PsionActualPretrainingBaselineToolsBundle, PsionActualPretrainingCheckpointBackupReceipt,
    PsionActualPretrainingCheckpointComparison, PsionActualPretrainingCheckpointEvalDecision,
    PsionActualPretrainingCheckpointEvalFailure, PsionActualPretrainingCheckpointFailureDrill,
    PsionActualPretrainingCheckpointManifest, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingCloseoutArtifact, PsionActualPretrainingCloseoutBundle,
    PsionActualPretrainingCloseoutFailureDrill, PsionActualPretrainingCloseoutGate,
    PsionActualPretrainingContinuationHandoff, PsionActualPretrainingContinueRestartDecision,
    PsionActualPretrainingCredentialBinding, PsionActualPretrainingCurrentRunStatus,
    PsionActualPretrainingDashboardPacket, PsionActualPretrainingDataBundle,
    PsionActualPretrainingDataloaderProbe, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingEvidenceContractError, PsionActualPretrainingHardwareObservation,
    PsionActualPretrainingHardwareQualification, PsionActualPretrainingLaneSpec,
    PsionActualPretrainingLaunchManifest, PsionActualPretrainingLauncherContractRefs,
    PsionActualPretrainingLauncherSurfaces, PsionActualPretrainingObservedCredentialSource,
    PsionActualPretrainingObservedWorker, PsionActualPretrainingPreflightRef,
    PsionActualPretrainingRecipeBundle, PsionActualPretrainingRedactedAlert,
    PsionActualPretrainingResumeManifest, PsionActualPretrainingRetainedPathSet,
    PsionActualPretrainingRetainedSummary, PsionActualPretrainingRunRoots,
    PsionActualPretrainingRunShapeObservation, PsionActualPretrainingRunShapeQualification,
    PsionActualPretrainingScalingBundle, PsionActualPretrainingStorageProbe,
    PsionActualPretrainingSystemsBundle, PsionActualPretrainingThroughputProbe,
    PsionActualPretrainingTopologyStorageBundle, PsionPluginConditionedSftStageManifest,
    PsionPretrainStageRunReceipt, PsionReferencePilotCheckpointManifest,
    PsionReferencePilotContributorContinuityReceipt, PsionReferencePilotDualHostStepReceipt,
    PsionReferencePilotDualHostTopologyReceipt, PsionReferencePilotJointContributionSummaryReceipt,
    PsionReferencePilotProgressCheckpointReceipt, PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID, PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_RECIPE_ID, PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_START_SURFACE_ID, PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;

static OPERATOR_HUMAN_OUTPUT_ENABLED: AtomicBool = AtomicBool::new(true);
const PSION_ACTUAL_PRETRAINING_DISTRIBUTED_BRINGUP_REHEARSAL_SCHEMA_VERSION: &str =
    "psion.actual_pretraining_distributed_bringup_rehearsal.v1";

macro_rules! println {
    ($($arg:tt)*) => {{
        if OPERATOR_HUMAN_OUTPUT_ENABLED.load(Ordering::Relaxed) {
            ::std::println!($($arg)*);
        }
    }};
}

enum Cli {
    Start {
        run_id: String,
        run_root: PathBuf,
        selected_git_ref: String,
        hardware_observation_path: Option<PathBuf>,
        run_shape_observation_path: Option<PathBuf>,
        allow_dirty_tree: bool,
        dry_run: bool,
    },
    Resume {
        run_root: PathBuf,
        selected_git_ref: String,
        hardware_observation_path: Option<PathBuf>,
        run_shape_observation_path: Option<PathBuf>,
        allow_dirty_tree: bool,
        dry_run: bool,
    },
    RecordCheckpoint {
        run_root: PathBuf,
        selected_git_ref: String,
        checkpoint_label: String,
        optimizer_step: u64,
        checkpoint_ref: String,
        checkpoint_object_digest: Option<String>,
        checkpoint_total_bytes: Option<u64>,
        inject_eval_worker_unavailable: bool,
        allow_dirty_tree: bool,
    },
    Backup {
        run_root: PathBuf,
        selected_git_ref: String,
        allow_dirty_tree: bool,
        inject_failed_upload: bool,
    },
    DecideContinueRestart {
        run_root: PathBuf,
        selected_git_ref: String,
        allow_dirty_tree: bool,
    },
    RehearseBaseLane {
        run_id: String,
        run_root: PathBuf,
        selected_git_ref: String,
        hardware_observation_path: Option<PathBuf>,
        run_shape_observation_path: Option<PathBuf>,
        allow_dirty_tree: bool,
        remote_host: Option<String>,
        secondary_remote_host: Option<String>,
        planned_interruption_step: Option<u64>,
        cleanup_remote: bool,
    },
}

struct FrozenContracts {
    lane_spec_ref: PsionActualPretrainingArtifactRef,
    recipe_bundle_ref: PsionActualPretrainingArtifactRef,
    baseline_tools_bundle_ref: PsionActualPretrainingArtifactRef,
    scaling_bundle_ref: PsionActualPretrainingArtifactRef,
    data_bundle_ref: PsionActualPretrainingArtifactRef,
    systems_bundle_ref: PsionActualPretrainingArtifactRef,
    topology_storage_bundle_ref: PsionActualPretrainingArtifactRef,
    evidence_contract_ref: PsionActualPretrainingArtifactRef,
    checkpoint_eval_benchmark_fixture_ref: PsionActualPretrainingArtifactRef,
    baseline_tools_bundle: PsionActualPretrainingBaselineToolsBundle,
    data_bundle: PsionActualPretrainingDataBundle,
    recipe_bundle: PsionActualPretrainingRecipeBundle,
    plugin_conditioned_stage_manifest: PsionPluginConditionedSftStageManifest,
    topology: PsionActualPretrainingTopologyStorageBundle,
    systems_bundle: PsionActualPretrainingSystemsBundle,
    evidence_contract: PsionActualPretrainingEvidenceContract,
}

struct ResolvedResumeTarget {
    checkpoint_pointer: Option<PsionActualPretrainingCheckpointPointer>,
    auto_resume_receipt: PsionActualPretrainingAutoResumeReceipt,
    failure_drill: Option<PsionActualPretrainingCheckpointFailureDrill>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PsionActualPretrainingDistributedBringupRehearsal {
    schema_version: String,
    run_id: String,
    bringup_run_id: String,
    rehearsal_summary_path: String,
    relative_output_root: String,
    operator_manifest_path: String,
    operator_summary_path: String,
    train_log_path: String,
    cluster_topology_receipt_path: String,
    cluster_contribution_receipts_path: String,
    contributor_continuity_receipt_path: String,
    progress_checkpoint_receipts_path: String,
    progress_checkpoint_directory: String,
    checkpoint_manifest_path: String,
    checkpoint_weights_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_drill_receipt_path: Option<String>,
    execution_topology_classification: String,
    contributor_count: usize,
    worker_hosts: Vec<String>,
    runtime_backends: Vec<String>,
    checkpoint_ref: String,
    checkpoint_object_digest: String,
    checkpoint_total_bytes: u64,
    optimizer_steps: u64,
    contribution_receipt_count: usize,
    progress_checkpoint_count: usize,
    progress_window_count: usize,
    progress_cadence_count: usize,
    final_cumulative_train_tokens_processed: u64,
    final_cumulative_mean_tokens_per_second: u64,
    claim_boundary: String,
    detail: String,
}

impl PsionActualPretrainingDistributedBringupRehearsal {
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.schema_version
            != PSION_ACTUAL_PRETRAINING_DISTRIBUTED_BRINGUP_REHEARSAL_SCHEMA_VERSION
        {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal schema_version drifted",
            )
            .into());
        }
        for value in [
            self.run_id.as_str(),
            self.bringup_run_id.as_str(),
            self.rehearsal_summary_path.as_str(),
            self.relative_output_root.as_str(),
            self.operator_manifest_path.as_str(),
            self.operator_summary_path.as_str(),
            self.train_log_path.as_str(),
            self.cluster_topology_receipt_path.as_str(),
            self.cluster_contribution_receipts_path.as_str(),
            self.contributor_continuity_receipt_path.as_str(),
            self.progress_checkpoint_receipts_path.as_str(),
            self.progress_checkpoint_directory.as_str(),
            self.checkpoint_manifest_path.as_str(),
            self.checkpoint_weights_path.as_str(),
            self.execution_topology_classification.as_str(),
            self.checkpoint_ref.as_str(),
            self.checkpoint_object_digest.as_str(),
            self.claim_boundary.as_str(),
            self.detail.as_str(),
        ] {
            if value.trim().is_empty() {
                return Err(std::io::Error::other(
                    "distributed bringup rehearsal retained one empty field",
                )
                .into());
            }
        }
        if self.contributor_count == 0 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal must retain at least one contributor",
            )
            .into());
        }
        if self.worker_hosts.len() != self.contributor_count {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal worker host count drifted from contributor_count",
            )
            .into());
        }
        if self.runtime_backends.len() != self.contributor_count {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal runtime backend count drifted from contributor_count",
            )
            .into());
        }
        if self.checkpoint_total_bytes == 0 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal checkpoint_total_bytes must be nonzero",
            )
            .into());
        }
        if self.optimizer_steps == 0 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal optimizer_steps must be nonzero",
            )
            .into());
        }
        if self.contribution_receipt_count < self.contributor_count {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal contribution receipts are too small for the contributor count",
            )
            .into());
        }
        if self.progress_checkpoint_count == 0 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal must retain at least one progress checkpoint",
            )
            .into());
        }
        if self.progress_window_count == 0 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal must retain at least one progress window",
            )
            .into());
        }
        if self
            .recovery_drill_receipt_path
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal recovery_drill_receipt_path must not be empty",
            )
            .into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PsionActualPretrainingDistributedRecoveryDrillReceipt {
    schema_version: String,
    run_id: String,
    recovery_state: String,
    interruption_kind: String,
    interruption_step: u64,
    total_target_optimizer_steps: u64,
    interrupted_segment_summary_path: String,
    interrupted_segment_artifact_dir: String,
    interrupted_checkpoint_ref: String,
    interrupted_checkpoint_manifest_path: String,
    interrupted_checkpoint_object_digest: String,
    recovered_segment_summary_path: Option<String>,
    recovered_segment_artifact_dir: Option<String>,
    recovered_checkpoint_ref: Option<String>,
    recovered_checkpoint_manifest_path: Option<String>,
    recovered_checkpoint_object_digest: Option<String>,
    lineage_preserved: bool,
    refusal_reason: Option<String>,
    detail: String,
}

impl PsionActualPretrainingDistributedRecoveryDrillReceipt {
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        for value in [
            self.schema_version.as_str(),
            self.run_id.as_str(),
            self.recovery_state.as_str(),
            self.interruption_kind.as_str(),
            self.interrupted_segment_summary_path.as_str(),
            self.interrupted_segment_artifact_dir.as_str(),
            self.interrupted_checkpoint_ref.as_str(),
            self.interrupted_checkpoint_manifest_path.as_str(),
            self.interrupted_checkpoint_object_digest.as_str(),
            self.detail.as_str(),
        ] {
            if value.trim().is_empty() {
                return Err(std::io::Error::other(
                    "distributed recovery drill retained one empty field",
                )
                .into());
            }
        }
        if self.interruption_step == 0 || self.total_target_optimizer_steps == 0 {
            return Err(std::io::Error::other(
                "distributed recovery drill steps must be nonzero",
            )
            .into());
        }
        if self.recovery_state == "recovered" {
            for value in [
                self.recovered_segment_summary_path.as_deref().unwrap_or_default(),
                self.recovered_segment_artifact_dir.as_deref().unwrap_or_default(),
                self.recovered_checkpoint_ref.as_deref().unwrap_or_default(),
                self.recovered_checkpoint_manifest_path
                    .as_deref()
                    .unwrap_or_default(),
                self.recovered_checkpoint_object_digest
                    .as_deref()
                    .unwrap_or_default(),
            ] {
                if value.trim().is_empty() {
                    return Err(std::io::Error::other(
                        "recovered drill must retain recovered segment surfaces",
                    )
                    .into());
                }
            }
            if !self.lineage_preserved {
                return Err(std::io::Error::other(
                    "recovered drill must preserve checkpoint lineage",
                )
                .into());
            }
        }
        if self.recovery_state == "refused"
            && self
                .refusal_reason
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(std::io::Error::other(
                "refused drill must retain refusal_reason",
            )
            .into());
        }
        Ok(())
    }
}

struct DistributedBringupSegmentArtifacts {
    output_root: PathBuf,
    operator_manifest_path: PathBuf,
    operator_summary_path: PathBuf,
    train_log_path: PathBuf,
    artifact_dir: PathBuf,
    cluster_topology_receipt_path: PathBuf,
    cluster_step_receipts_path: PathBuf,
    cluster_contribution_receipts_path: PathBuf,
    contributor_continuity_receipt_path: PathBuf,
    progress_checkpoint_receipts_path: PathBuf,
    progress_checkpoint_directory: PathBuf,
    checkpoint_manifest_path: PathBuf,
    checkpoint_weights_path: PathBuf,
    checkpoint_optimizer_state_path: PathBuf,
    checkpoint_stage_receipt_path: PathBuf,
    topology_receipt: PsionReferencePilotDualHostTopologyReceipt,
    joint_step_receipts: Vec<PsionReferencePilotDualHostStepReceipt>,
    contribution_receipts: Vec<PsionReferencePilotJointContributionSummaryReceipt>,
    contributor_continuity_receipt: PsionReferencePilotContributorContinuityReceipt,
    progress_checkpoint_receipts: Vec<PsionReferencePilotProgressCheckpointReceipt>,
    checkpoint_manifest: PsionReferencePilotCheckpointManifest,
    checkpoint_stage_receipt: PsionPretrainStageRunReceipt,
    checkpoint_object_digest: String,
    checkpoint_total_bytes: u64,
}

pub fn set_human_output_enabled(enabled: bool) {
    OPERATOR_HUMAN_OUTPUT_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn run_with_args<I, S>(args: I) -> Result<(), Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let cli = parse_cli_from_args(args)?;
    let root = workspace_root()?;
    let repo_root = root.as_path();
    let contracts = load_frozen_contracts(repo_root)?;
    let launcher_surfaces = launcher_surfaces();
    launcher_surfaces.validate()?;
    let retained_paths_set = retained_paths();
    retained_paths_set.validate()?;

    match cli {
        Cli::Start {
            run_id,
            run_root,
            selected_git_ref,
            hardware_observation_path,
            run_shape_observation_path,
            allow_dirty_tree,
            dry_run,
        } => {
            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let run_roots = run_roots(&run_root, &run_id, &contracts.topology);
            let hardware_qualification = build_hardware_qualification(
                repo_root,
                &run_id,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                hardware_observation_path.as_deref(),
                &contracts,
            )?;
            let run_shape_qualification = build_run_shape_qualification(
                repo_root,
                &run_id,
                &run_root,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                run_shape_observation_path.as_deref(),
                &contracts,
            )?;
            let preflight_receipt =
                preflight_ref_from_qualification(&hardware_qualification, &retained_paths_set);
            let run_shape_receipt =
                run_shape_ref_from_qualification(&run_shape_qualification, &retained_paths_set);
            let contract_refs = PsionActualPretrainingLauncherContractRefs {
                lane_spec: contracts.lane_spec_ref.clone(),
                recipe_bundle: contracts.recipe_bundle_ref.clone(),
                baseline_tools_bundle: contracts.baseline_tools_bundle_ref.clone(),
                scaling_bundle: contracts.scaling_bundle_ref.clone(),
                data_bundle: contracts.data_bundle_ref.clone(),
                systems_bundle: contracts.systems_bundle_ref.clone(),
                topology_storage_bundle: contracts.topology_storage_bundle_ref.clone(),
                evidence_contract: contracts.evidence_contract_ref.clone(),
            };
            if !dry_run
                && (preflight_receipt.admission_state != "admitted"
                    || run_shape_receipt.admission_state != "admitted")
            {
                write_preflight_receipts(
                    &run_root,
                    &hardware_qualification,
                    &run_shape_qualification,
                    &format!(
                        "{} phase=launch_refused_preflight surface_id={} git_commit_sha={} hardware_admission_state={} run_shape_admission_state={}\n",
                        now_utc(repo_root)?,
                        PSION_ACTUAL_PRETRAINING_START_SURFACE_ID,
                        git_commit_sha,
                        preflight_receipt.admission_state,
                        run_shape_receipt.admission_state,
                    ),
                )?;
                return Err(std::io::Error::other(format!(
                    "actual pretraining launch refused preflight admission; see {} and {}",
                    run_root
                        .join(&retained_paths_set.hardware_qualification_path)
                        .display(),
                    run_root
                        .join(&retained_paths_set.run_shape_qualification_path)
                        .display()
                ))
                .into());
            }
            let credential_sources = credential_bindings(&contracts.topology);
            let launch_manifest = PsionActualPretrainingLaunchManifest {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION,
                ),
                surface_id: String::from(if dry_run {
                    PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID
                } else {
                    PSION_ACTUAL_PRETRAINING_START_SURFACE_ID
                }),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
                topology_storage_bundle_id: String::from(
                    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
                ),
                evidence_contract_id: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
                run_id: run_id.clone(),
                retained_paths: retained_paths_set.clone(),
                launcher_surfaces: launcher_surfaces.clone(),
                run_roots: run_roots.clone(),
                preflight_receipt: preflight_receipt.clone(),
                run_shape_receipt: run_shape_receipt.clone(),
                contract_refs,
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                workspace_status_sha256: workspace_status_sha256.clone(),
                credential_sources,
                claim_boundary: String::from(
                    "The actual-lane launcher materializes the frozen launch manifest, retained status surfaces, checkpoint pointer, and provisional closeout bundle. It does not by itself execute the distributed broader-pretraining run.",
                ),
                detail: String::from(
                    "Launch manifest binds the actual pretraining operator command to the frozen lane, recipe, baseline-tools, scaling, data, systems, topology/storage, evidence, and git-provenance surfaces.",
                ),
            };
            launch_manifest.validate()?;

            let checkpoint_pointer = PsionActualPretrainingCheckpointPointer {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: run_id.clone(),
                pointer_state: String::from("pending_first_checkpoint"),
                checkpoint_label: String::from("pending_first_checkpoint"),
                optimizer_step: 0,
                checkpoint_ref: None,
                checkpoint_manifest_relative_path: None,
                detail: String::from(
                    "Launch-time checkpoint pointer records that the actual lane has not yet admitted the first durable checkpoint.",
                ),
            };
            checkpoint_pointer.validate()?;

            let phase = if dry_run {
                "dry_run_planned"
            } else {
                "launch_staged"
            };
            let current_status = PsionActualPretrainingCurrentRunStatus {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: run_id.clone(),
                phase: String::from(phase),
                current_status_path: retained_paths_set.current_status_path.clone(),
                retained_summary_path: retained_paths_set.retained_summary_path.clone(),
                latest_checkpoint_pointer_path: retained_paths_set
                    .latest_checkpoint_pointer_path
                    .clone(),
                continuation_handoff_path: retained_paths_set.continuation_handoff_path.clone(),
                latest_checkpoint_label: String::from("pending_first_checkpoint"),
                last_completed_step: 0,
                launcher_surfaces: launcher_surfaces.clone(),
                updated_at_utc: now_utc(repo_root)?,
                detail: String::from(
                    "Current status records the canonical actual-lane launch state before the first accepted checkpoint exists.",
                ),
            };
            current_status.validate()?;

            let retained_summary = PsionActualPretrainingRetainedSummary {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: run_id.clone(),
                last_known_phase: String::from(phase),
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                current_status_path: retained_paths_set.current_status_path.clone(),
                latest_checkpoint_pointer_path: retained_paths_set
                    .latest_checkpoint_pointer_path
                    .clone(),
                continuation_handoff_path: retained_paths_set.continuation_handoff_path.clone(),
                launcher_surfaces: launcher_surfaces.clone(),
                claim_boundary: String::from(
                    "The retained summary records actual-lane start, dry-run, resume, and status surfaces plus the last known operator state. It does not claim that cluster execution, automatic eval, or durable backup are finished.",
                ),
                detail: String::from(
                    "Retained summary keeps the last known actual-lane operator state legible outside the launch manifest.",
                ),
            };
            retained_summary.validate()?;

            let closeout_bundle = PsionActualPretrainingCloseoutBundle {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: run_id.clone(),
                closeout_state: String::from(phase),
                retained_paths: retained_paths_set.clone(),
                selected_git_ref: selected_git_ref,
                git_commit_sha,
                dirty_tree_admission,
                workspace_status_sha256,
                evidence_artifacts: Vec::new(),
                closeout_gates: Vec::new(),
                failure_drills: Vec::new(),
                can_now_claim: Vec::new(),
                still_out_of_scope: Vec::new(),
                claim_boundary: String::from(
                    "This provisional closeout bundle repeats launcher provenance early so later closeout work can extend the same evidence family without losing source-state identity. It does not claim completed training.",
                ),
                detail: String::from(
                    "Launch-time closeout bundle repeats the selected ref, git SHA, and dirty-tree posture inside the retained evidence family.",
                ),
            };
            closeout_bundle.validate()?;

            write_launcher_bundle(
                &run_root,
                &hardware_qualification,
                &run_shape_qualification,
                &contracts.systems_bundle,
                Some(&launch_manifest),
                None,
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                None,
                &closeout_bundle,
                &format!(
                    "{} phase={} surface_id={} git_commit_sha={} hardware_admission_state={} run_shape_admission_state={}\n",
                    now_utc(repo_root)?,
                    phase,
                    launch_manifest.surface_id,
                    launch_manifest.git_commit_sha,
                    preflight_receipt.admission_state,
                    run_shape_receipt.admission_state,
                ),
            )?;

            println!("status={phase}");
            println!("surface_id={}", launch_manifest.surface_id);
            println!("run_id={run_id}");
            println!("run_root={}", run_root.display());
            println!(
                "launch_manifest={}",
                run_root
                    .join(&retained_paths_set.launch_manifest_path)
                    .display()
            );
            println!(
                "current_status={}",
                run_root
                    .join(&retained_paths_set.current_status_path)
                    .display()
            );
            println!(
                "retained_summary={}",
                run_root
                    .join(&retained_paths_set.retained_summary_path)
                    .display()
            );
            println!(
                "checkpoint_pointer={}",
                run_root
                    .join(&retained_paths_set.latest_checkpoint_pointer_path)
                    .display()
            );
            println!(
                "hardware_qualification={}",
                run_root
                    .join(&retained_paths_set.hardware_qualification_path)
                    .display()
            );
            println!(
                "hardware_admission_state={}",
                preflight_receipt.admission_state
            );
            println!(
                "run_shape_qualification={}",
                run_root
                    .join(&retained_paths_set.run_shape_qualification_path)
                    .display()
            );
            println!(
                "run_shape_admission_state={}",
                run_shape_receipt.admission_state
            );
            println!(
                "closeout_bundle={}",
                run_root
                    .join(&retained_paths_set.closeout_bundle_path)
                    .display()
            );
            println!(
                "dashboard={}",
                run_root
                    .join(&retained_paths_set.current_dashboard_path)
                    .display()
            );
            println!(
                "active_alert_feed={}",
                run_root
                    .join(&retained_paths_set.active_alert_feed_path)
                    .display()
            );
            println!(
                "launcher_log={}",
                run_root
                    .join(&retained_paths_set.launcher_log_path)
                    .display()
            );
        }
        Cli::Resume {
            run_root,
            selected_git_ref,
            hardware_observation_path,
            run_shape_observation_path,
            allow_dirty_tree,
            dry_run,
        } => {
            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let retained_paths = retained_paths();
            let resolved_resume = resolve_resume_target(
                &run_root,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.auto_resume_receipt_path),
                &resolved_resume.auto_resume_receipt,
            )?;
            if let Some(failure_drill) = &resolved_resume.failure_drill {
                let drill_path =
                    checkpoint_failure_drill_path(&run_root, &failure_drill.drill_kind);
                write_json_pretty(&drill_path, failure_drill)?;
            }
            if resolved_resume.auto_resume_receipt.resolution_state == "refused" {
                append_launcher_log(
                    &run_root,
                    &format!(
                        "{} phase=resume_refused_auto_resume surface_id={} git_commit_sha={} refusal_reason={}\n",
                        now_utc(repo_root)?,
                        PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
                        git_commit_sha,
                        resolved_resume
                            .auto_resume_receipt
                            .refusal_reason
                            .as_deref()
                            .unwrap_or("none")
                    ),
                )?;
                return Err(std::io::Error::other(
                    "resume could not resolve an admitted checkpoint pointer from the primary or backup path",
                )
                .into());
            }
            let checkpoint_pointer = resolved_resume.checkpoint_pointer.ok_or_else(|| {
                std::io::Error::other("resolved auto-resume receipt is missing checkpoint pointer")
            })?;
            let run_roots = run_roots(&run_root, &checkpoint_pointer.run_id, &contracts.topology);
            let hardware_qualification = build_hardware_qualification(
                repo_root,
                &checkpoint_pointer.run_id,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                hardware_observation_path.as_deref(),
                &contracts,
            )?;
            let run_shape_qualification = build_run_shape_qualification(
                repo_root,
                &checkpoint_pointer.run_id,
                &run_root,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                run_shape_observation_path.as_deref(),
                &contracts,
            )?;
            let preflight_receipt =
                preflight_ref_from_qualification(&hardware_qualification, &retained_paths);
            let run_shape_receipt =
                run_shape_ref_from_qualification(&run_shape_qualification, &retained_paths);
            if !dry_run
                && (preflight_receipt.admission_state != "admitted"
                    || run_shape_receipt.admission_state != "admitted")
            {
                write_preflight_receipts(
                    &run_root,
                    &hardware_qualification,
                    &run_shape_qualification,
                    &format!(
                        "{} phase=resume_refused_preflight surface_id={} git_commit_sha={} hardware_admission_state={} run_shape_admission_state={}\n",
                        now_utc(repo_root)?,
                        PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
                        git_commit_sha,
                        preflight_receipt.admission_state,
                        run_shape_receipt.admission_state,
                    ),
                )?;
                return Err(std::io::Error::other(format!(
                    "actual pretraining resume refused preflight admission; see {} and {}",
                    run_root
                        .join(&retained_paths.hardware_qualification_path)
                        .display(),
                    run_root
                        .join(&retained_paths.run_shape_qualification_path)
                        .display()
                ))
                .into());
            }
            let resume_manifest = PsionActualPretrainingResumeManifest {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION,
                ),
                surface_id: String::from(PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
                topology_storage_bundle_id: String::from(
                    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
                ),
                evidence_contract_id: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                retained_paths: retained_paths.clone(),
                launcher_surfaces: launcher_surfaces.clone(),
                run_roots: run_roots.clone(),
                preflight_receipt: preflight_receipt.clone(),
                run_shape_receipt: run_shape_receipt.clone(),
                contract_refs: PsionActualPretrainingLauncherContractRefs {
                    lane_spec: contracts.lane_spec_ref.clone(),
                    recipe_bundle: contracts.recipe_bundle_ref.clone(),
                    baseline_tools_bundle: contracts.baseline_tools_bundle_ref.clone(),
                    scaling_bundle: contracts.scaling_bundle_ref.clone(),
                    data_bundle: contracts.data_bundle_ref.clone(),
                    systems_bundle: contracts.systems_bundle_ref.clone(),
                    topology_storage_bundle: contracts.topology_storage_bundle_ref.clone(),
                    evidence_contract: contracts.evidence_contract_ref.clone(),
                },
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                workspace_status_sha256: workspace_status_sha256.clone(),
                latest_checkpoint_pointer_path: retained_paths
                    .latest_checkpoint_pointer_path
                    .clone(),
                checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
                optimizer_step: checkpoint_pointer.optimizer_step,
                checkpoint_ref: checkpoint_pointer
                    .checkpoint_ref
                    .clone()
                    .expect("accepted checkpoint pointer must retain checkpoint_ref"),
                claim_boundary: String::from(
                    "The actual-lane resume manifest binds the canonical resume command to the accepted checkpoint pointer inside the frozen evidence family. It does not claim post-resume training success by itself.",
                ),
                detail: String::from(
                    "Resume manifest records the exact accepted checkpoint selection and repeats launcher provenance plus the frozen baseline-tools, scaling, data, and systems bundles for restart decisions.",
                ),
            };
            resume_manifest.validate()?;

            let phase = if dry_run {
                "resume_dry_run_planned"
            } else {
                "resume_staged"
            };
            let current_status = PsionActualPretrainingCurrentRunStatus {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                phase: String::from(phase),
                current_status_path: retained_paths.current_status_path.clone(),
                retained_summary_path: retained_paths.retained_summary_path.clone(),
                latest_checkpoint_pointer_path: retained_paths
                    .latest_checkpoint_pointer_path
                    .clone(),
                continuation_handoff_path: retained_paths.continuation_handoff_path.clone(),
                latest_checkpoint_label: checkpoint_pointer.checkpoint_label.clone(),
                last_completed_step: checkpoint_pointer.optimizer_step,
                launcher_surfaces: launcher_surfaces.clone(),
                updated_at_utc: now_utc(repo_root)?,
                detail: String::from(
                    "Current status records the accepted checkpoint selected by the canonical actual-lane resume command.",
                ),
            };
            current_status.validate()?;

            let retained_summary = PsionActualPretrainingRetainedSummary {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                last_known_phase: String::from(phase),
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                current_status_path: retained_paths.current_status_path.clone(),
                latest_checkpoint_pointer_path: retained_paths
                    .latest_checkpoint_pointer_path
                    .clone(),
                continuation_handoff_path: retained_paths.continuation_handoff_path.clone(),
                launcher_surfaces: launcher_surfaces.clone(),
                claim_boundary: String::from(
                    "The retained summary records actual-lane start, dry-run, resume, and status surfaces plus the last known operator state. It does not claim that cluster execution, automatic eval, or durable backup are finished.",
                ),
                detail: String::from(
                    "Retained summary keeps the accepted resume checkpoint legible for operator review.",
                ),
            };
            retained_summary.validate()?;

            let closeout_bundle = PsionActualPretrainingCloseoutBundle {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                closeout_state: String::from(phase),
                retained_paths: retained_paths.clone(),
                selected_git_ref: selected_git_ref,
                git_commit_sha,
                dirty_tree_admission,
                workspace_status_sha256,
                evidence_artifacts: Vec::new(),
                closeout_gates: Vec::new(),
                failure_drills: Vec::new(),
                can_now_claim: Vec::new(),
                still_out_of_scope: Vec::new(),
                claim_boundary: String::from(
                    "This provisional closeout bundle repeats launcher provenance early so later closeout work can extend the same evidence family without losing source-state identity. It does not claim completed training.",
                ),
                detail: String::from(
                    "Resume-time closeout bundle repeats the selected ref, git SHA, and dirty-tree posture inside the retained evidence family.",
                ),
            };
            closeout_bundle.validate()?;
            let continuation_handoff = record_psion_actual_pretraining_continuation_handoff(
                &checkpoint_pointer,
                &contracts.recipe_bundle,
                &contracts.plugin_conditioned_stage_manifest,
            )?;

            write_launcher_bundle(
                &run_root,
                &hardware_qualification,
                &run_shape_qualification,
                &contracts.systems_bundle,
                None,
                Some(&resume_manifest),
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                Some(&continuation_handoff),
                &closeout_bundle,
                &format!(
                    "{} phase={} surface_id={} git_commit_sha={} hardware_admission_state={} run_shape_admission_state={}\n",
                    now_utc(repo_root)?,
                    phase,
                    resume_manifest.surface_id,
                    resume_manifest.git_commit_sha,
                    preflight_receipt.admission_state,
                    run_shape_receipt.admission_state,
                ),
            )?;

            println!("status={phase}");
            println!("surface_id={}", resume_manifest.surface_id);
            println!("run_id={}", checkpoint_pointer.run_id);
            println!("run_root={}", run_root.display());
            println!(
                "resume_manifest={}",
                run_root
                    .join(&retained_paths.resume_manifest_path)
                    .display()
            );
            println!(
                "current_status={}",
                run_root.join(&retained_paths.current_status_path).display()
            );
            println!(
                "retained_summary={}",
                run_root
                    .join(&retained_paths.retained_summary_path)
                    .display()
            );
            println!(
                "checkpoint_pointer={}",
                run_root
                    .join(&retained_paths.latest_checkpoint_pointer_path)
                    .display()
            );
            println!(
                "auto_resume_receipt={}",
                run_root
                    .join(&retained_paths.auto_resume_receipt_path)
                    .display()
            );
            println!(
                "hardware_qualification={}",
                run_root
                    .join(&retained_paths.hardware_qualification_path)
                    .display()
            );
            println!(
                "hardware_admission_state={}",
                preflight_receipt.admission_state
            );
            println!(
                "run_shape_qualification={}",
                run_root
                    .join(&retained_paths.run_shape_qualification_path)
                    .display()
            );
            println!(
                "run_shape_admission_state={}",
                run_shape_receipt.admission_state
            );
            println!(
                "continuation_handoff={}",
                run_root
                    .join(&retained_paths.continuation_handoff_path)
                    .display()
            );
            println!(
                "closeout_bundle={}",
                run_root
                    .join(&retained_paths.closeout_bundle_path)
                    .display()
            );
            println!(
                "dashboard={}",
                run_root
                    .join(&retained_paths.current_dashboard_path)
                    .display()
            );
            println!(
                "active_alert_feed={}",
                run_root
                    .join(&retained_paths.active_alert_feed_path)
                    .display()
            );
            println!(
                "launcher_log={}",
                run_root.join(&retained_paths.launcher_log_path).display()
            );
        }
        Cli::RecordCheckpoint {
            run_root,
            selected_git_ref,
            checkpoint_label,
            optimizer_step,
            checkpoint_ref,
            checkpoint_object_digest,
            checkpoint_total_bytes,
            inject_eval_worker_unavailable,
            allow_dirty_tree,
        } => {
            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let retained_paths = retained_paths();
            let pointer_path = run_root.join(&retained_paths.latest_checkpoint_pointer_path);
            let mut checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
                load_json(&pointer_path)?;
            checkpoint_pointer.validate()?;
            let mut current_status: PsionActualPretrainingCurrentRunStatus =
                load_json(&run_root.join(&retained_paths.current_status_path))?;
            current_status.validate()?;
            let mut retained_summary: PsionActualPretrainingRetainedSummary =
                load_json(&run_root.join(&retained_paths.retained_summary_path))?;
            retained_summary.validate()?;
            let resolved_checkpoint_object_digest = checkpoint_object_digest.unwrap_or_else(|| {
                sha256_hex(
                    format!(
                        "{}|{}|{}|{}",
                        checkpoint_pointer.run_id, checkpoint_label, optimizer_step, checkpoint_ref
                    )
                    .as_bytes(),
                )
            });
            let checkpoint_manifest = record_psion_actual_pretraining_checkpoint_manifest(
                &checkpoint_pointer.run_id,
                &checkpoint_label,
                optimizer_step,
                &checkpoint_ref,
                &resolved_checkpoint_object_digest,
                checkpoint_total_bytes.unwrap_or(
                    contracts
                        .systems_bundle
                        .memory_qualification
                        .checkpoint_total_bytes,
                ),
                &contracts.data_bundle.replay_authority.dataset_identity,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
                "Checkpoint manifest records one accepted actual-lane checkpoint inside the frozen evidence family without claiming that the broader distributed training job ran inside this launcher process.",
            )?;
            write_json_pretty(
                &run_root.join(&checkpoint_manifest.relative_manifest_path),
                &checkpoint_manifest,
            )?;
            let checkpoint_manifest_ref = run_artifact_ref(
                &run_root,
                &run_root.join(&checkpoint_manifest.relative_manifest_path),
            )?;
            checkpoint_pointer.pointer_state = String::from("accepted");
            checkpoint_pointer.checkpoint_label = checkpoint_label.clone();
            checkpoint_pointer.optimizer_step = optimizer_step;
            checkpoint_pointer.checkpoint_ref = Some(checkpoint_ref.clone());
            checkpoint_pointer.checkpoint_manifest_relative_path =
                Some(checkpoint_manifest.relative_manifest_path.clone());
            checkpoint_pointer.detail = String::from(
                "Accepted checkpoint pointer binds actual-lane resume to the latest admitted checkpoint manifest.",
            );
            checkpoint_pointer.validate()?;
            write_json_pretty(&pointer_path, &checkpoint_pointer)?;
            let (backup_receipt, failure_drill) = materialize_checkpoint_backup(
                &run_root,
                &checkpoint_pointer,
                &checkpoint_manifest,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
                &contracts,
                false,
            )?;
            if let Some(failure_drill) = &failure_drill {
                write_json_pretty(
                    &checkpoint_failure_drill_path(&run_root, &failure_drill.drill_kind),
                    failure_drill,
                )?;
            }
            let checkpoint_eval_decision_relative_path =
                checkpoint_eval_decision_relative_path(optimizer_step);
            let checkpoint_eval_failure_receipt_relative_path =
                checkpoint_eval_failure_relative_path(optimizer_step);
            let checkpoint_eval_decision = if inject_eval_worker_unavailable {
                None
            } else {
                let decision = record_psion_actual_pretraining_checkpoint_eval_decision(
                    &checkpoint_pointer.run_id,
                    &selected_git_ref,
                    &git_commit_sha,
                    &dirty_tree_admission,
                    workspace_status_sha256.clone(),
                    &checkpoint_label,
                    optimizer_step,
                    &checkpoint_ref,
                    checkpoint_manifest_ref.clone(),
                    contracts.checkpoint_eval_benchmark_fixture_ref.clone(),
                    &contracts.data_bundle,
                    "This retained checkpoint-eval decision binds one accepted actual-lane checkpoint to the frozen checkpoint review pack and the frozen benchmark families already attached to the actual-lane data bundle. It is the automatic checkpoint review surface for later continue-vs-restart logic. It does not claim distributed broader-pretraining closure, dashboard fan-out, or final promotion review.",
                    "Checkpoint eval decision records one automatic retained review over the accepted checkpoint using the canonical actual-lane benchmark pack.",
                )?;
                write_json_pretty(
                    &run_root.join(&checkpoint_eval_decision_relative_path),
                    &decision,
                )?;
                write_json_pretty(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
                    &decision,
                )?;
                Some(decision)
            };
            let checkpoint_eval_failure = if inject_eval_worker_unavailable {
                let failure = record_psion_actual_pretraining_checkpoint_eval_failure(
                    &checkpoint_pointer.run_id,
                    &selected_git_ref,
                    &git_commit_sha,
                    &dirty_tree_admission,
                    workspace_status_sha256.clone(),
                    &checkpoint_label,
                    optimizer_step,
                    &checkpoint_ref,
                    checkpoint_manifest_ref.clone(),
                    contracts.checkpoint_eval_benchmark_fixture_ref.clone(),
                    "eval_worker_unavailable",
                    "This retained checkpoint-eval failure proves the actual-lane operator path does not silently skip automatic checkpoint review when the eval worker is unavailable. It retains an explicit retry requirement and a redacted alert instead.",
                    "Checkpoint eval failure records that the automatic checkpoint-review worker was unavailable after the accepted checkpoint entered the retained backup family.",
                )?;
                write_json_pretty(
                    &run_root.join(&checkpoint_eval_failure_receipt_relative_path),
                    &failure,
                )?;
                write_json_pretty(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_failure_path),
                    &failure,
                )?;
                Some(failure)
            } else {
                None
            };
            let redacted_alert = if let Some(_failure) = &checkpoint_eval_failure {
                let alert = record_psion_actual_pretraining_redacted_alert(
                    &checkpoint_pointer.run_id,
                    optimizer_step,
                    &checkpoint_eval_failure_receipt_relative_path,
                    "Checkpoint eval retry alerts keep the actual lane honest by retaining the failed trigger path under one redacted alert surface instead of silently dropping the missing eval.",
                )?;
                write_json_pretty(
                    &run_root.join(&retained_paths.latest_redacted_alert_path),
                    &alert,
                )?;
                Some(alert)
            } else {
                None
            };
            if checkpoint_eval_decision.is_some() {
                remove_if_exists(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_failure_path),
                )?;
                remove_if_exists(&run_root.join(&retained_paths.latest_redacted_alert_path))?;
            } else {
                remove_if_exists(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
                )?;
            }
            current_status.phase = String::from(if checkpoint_eval_decision.is_some() {
                "checkpoint_evaluated"
            } else {
                "checkpoint_eval_retry_required"
            });
            current_status.latest_checkpoint_label = checkpoint_label.clone();
            current_status.last_completed_step = optimizer_step;
            current_status.updated_at_utc = now_utc(repo_root)?;
            current_status.detail = String::from(if checkpoint_eval_decision.is_some() {
                "Current status records that one accepted checkpoint was materialized, backed up, and automatically evaluated under the actual-lane checkpoint review contract."
            } else {
                "Current status records that one accepted checkpoint was materialized and backed up, but automatic checkpoint eval now requires retry under the retained actual-lane alert surface."
            });
            current_status.validate()?;
            retained_summary.last_known_phase = current_status.phase.clone();
            retained_summary.selected_git_ref = selected_git_ref.clone();
            retained_summary.git_commit_sha = git_commit_sha.clone();
            retained_summary.dirty_tree_admission = dirty_tree_admission.clone();
            retained_summary.detail = String::from(if checkpoint_eval_decision.is_some() {
                "Retained summary records the latest accepted checkpoint, backup posture, and automatic checkpoint-eval decision for the actual lane."
            } else {
                "Retained summary records the latest accepted checkpoint and backup posture while keeping the checkpoint-eval retry requirement explicit for the actual lane."
            });
            retained_summary.validate()?;
            let closeout_bundle = PsionActualPretrainingCloseoutBundle {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                closeout_state: current_status.phase.clone(),
                retained_paths: retained_paths.clone(),
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                workspace_status_sha256: workspace_status_sha256.clone(),
                evidence_artifacts: Vec::new(),
                closeout_gates: Vec::new(),
                failure_drills: Vec::new(),
                can_now_claim: Vec::new(),
                still_out_of_scope: Vec::new(),
                claim_boundary: String::from(if checkpoint_eval_decision.is_some() {
                    "This provisional closeout bundle now records accepted-checkpoint, backup, and automatic checkpoint-eval progress under the actual-lane evidence family. It does not claim dashboard alerting fan-out or final closeout completion."
                } else {
                    "This provisional closeout bundle now records accepted-checkpoint and backup progress plus an explicit checkpoint-eval retry requirement under the actual-lane evidence family. It does not claim dashboard alerting fan-out or final closeout completion."
                }),
                detail: String::from(if checkpoint_eval_decision.is_some() {
                    "Checkpoint-evaluated closeout bundle repeats the selected ref, git SHA, and dirty-tree posture after the accepted checkpoint entered the automatic review family."
                } else {
                    "Checkpoint-eval-retry closeout bundle repeats the selected ref, git SHA, and dirty-tree posture after the accepted checkpoint entered the retained failure and alert family."
                }),
            };
            closeout_bundle.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.current_status_path),
                &current_status,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.retained_summary_path),
                &retained_summary,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.closeout_bundle_path),
                &closeout_bundle,
            )?;
            write_dashboard_bundle(
                &run_root,
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                &load_json(&run_root.join(&retained_paths.hardware_qualification_path))?,
                &load_json(&run_root.join(&retained_paths.run_shape_qualification_path))?,
                &contracts.systems_bundle,
                Some(&backup_receipt),
                checkpoint_eval_decision.as_ref(),
                checkpoint_eval_failure.as_ref(),
                redacted_alert.as_ref(),
            )?;
            append_launcher_log(
                &run_root,
                &format!(
                    "{} phase={} surface_id={} git_commit_sha={} checkpoint_label={} checkpoint_step={} backup_state={} checkpoint_eval_state={}\n",
                    now_utc(repo_root)?,
                    current_status.phase,
                    "psion_actual_pretraining.record_checkpoint",
                    git_commit_sha,
                    checkpoint_label,
                    optimizer_step,
                    backup_receipt.backup_state,
                    if let Some(decision) = &checkpoint_eval_decision {
                        decision.decision_state.as_str()
                    } else {
                        "retry_required"
                    }
                ),
            )?;
            println!("status={}", current_status.phase);
            println!("run_id={}", checkpoint_pointer.run_id);
            println!("run_root={}", run_root.display());
            println!(
                "checkpoint_manifest={}",
                run_root
                    .join(&checkpoint_manifest.relative_manifest_path)
                    .display()
            );
            println!("checkpoint_pointer={}", pointer_path.display());
            println!(
                "checkpoint_backup_receipt={}",
                run_root
                    .join(&retained_paths.latest_checkpoint_backup_receipt_path)
                    .display()
            );
            if checkpoint_eval_decision.is_some() {
                println!(
                    "checkpoint_eval_decision={}",
                    run_root
                        .join(&retained_paths.latest_checkpoint_eval_decision_path)
                        .display()
                );
            }
            if checkpoint_eval_failure.is_some() {
                println!(
                    "checkpoint_eval_failure={}",
                    run_root
                        .join(&retained_paths.latest_checkpoint_eval_failure_path)
                        .display()
                );
            }
            if redacted_alert.is_some() {
                println!(
                    "latest_redacted_alert={}",
                    run_root
                        .join(&retained_paths.latest_redacted_alert_path)
                        .display()
                );
            }
            println!(
                "closeout_bundle={}",
                run_root
                    .join(&retained_paths.closeout_bundle_path)
                    .display()
            );
            println!(
                "dashboard={}",
                run_root
                    .join(&retained_paths.current_dashboard_path)
                    .display()
            );
            println!(
                "active_alert_feed={}",
                run_root
                    .join(&retained_paths.active_alert_feed_path)
                    .display()
            );
        }
        Cli::Backup {
            run_root,
            selected_git_ref,
            allow_dirty_tree,
            inject_failed_upload,
        } => {
            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let retained_paths = retained_paths();
            let pointer_path = run_root.join(&retained_paths.latest_checkpoint_pointer_path);
            let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
                load_json(&pointer_path)?;
            let checkpoint_manifest = validate_resume_candidate(&run_root, &checkpoint_pointer)
                .map_err(std::io::Error::other)?;
            let (backup_receipt, failure_drill) = materialize_checkpoint_backup(
                &run_root,
                &checkpoint_pointer,
                &checkpoint_manifest,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
                &contracts,
                inject_failed_upload,
            )?;
            if let Some(failure_drill) = &failure_drill {
                write_json_pretty(
                    &checkpoint_failure_drill_path(&run_root, &failure_drill.drill_kind),
                    failure_drill,
                )?;
            }
            let mut current_status: PsionActualPretrainingCurrentRunStatus =
                load_json(&run_root.join(&retained_paths.current_status_path))?;
            current_status.validate()?;
            let mut retained_summary: PsionActualPretrainingRetainedSummary =
                load_json(&run_root.join(&retained_paths.retained_summary_path))?;
            retained_summary.validate()?;
            let hardware_qualification: PsionActualPretrainingHardwareQualification =
                load_json(&run_root.join(&retained_paths.hardware_qualification_path))?;
            hardware_qualification.validate()?;
            let run_shape_qualification: PsionActualPretrainingRunShapeQualification =
                load_json(&run_root.join(&retained_paths.run_shape_qualification_path))?;
            run_shape_qualification.validate()?;
            let checkpoint_eval_decision: Option<PsionActualPretrainingCheckpointEvalDecision> =
                load_optional_json(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
                )?;
            if let Some(decision) = &checkpoint_eval_decision {
                decision.validate()?;
            }
            let checkpoint_eval_failure: Option<PsionActualPretrainingCheckpointEvalFailure> =
                load_optional_json(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_failure_path),
                )?;
            if let Some(failure) = &checkpoint_eval_failure {
                failure.validate()?;
            }
            let latest_redacted_alert: Option<PsionActualPretrainingRedactedAlert> =
                load_optional_json(&run_root.join(&retained_paths.latest_redacted_alert_path))?;
            if let Some(alert) = &latest_redacted_alert {
                alert.validate()?;
            }
            let phase = if backup_receipt.backup_state == "backed_up" {
                "checkpoint_backed_up"
            } else {
                "checkpoint_backup_refused"
            };
            current_status.phase = String::from(phase);
            current_status.latest_checkpoint_label = checkpoint_pointer.checkpoint_label.clone();
            current_status.last_completed_step = checkpoint_pointer.optimizer_step;
            current_status.updated_at_utc = now_utc(repo_root)?;
            current_status.detail = String::from(if backup_receipt.backup_state == "backed_up" {
                "Current status records that the latest accepted checkpoint retains a durable backup receipt under the actual-lane evidence family."
            } else {
                "Current status records that the latest accepted checkpoint backup now requires operator recovery before the actual lane should rely on remote durability."
            });
            current_status.validate()?;
            retained_summary.last_known_phase = current_status.phase.clone();
            retained_summary.selected_git_ref = selected_git_ref.clone();
            retained_summary.git_commit_sha = git_commit_sha.clone();
            retained_summary.dirty_tree_admission = dirty_tree_admission.clone();
            retained_summary.detail = String::from(if backup_receipt.backup_state == "backed_up" {
                "Retained summary records that the latest accepted checkpoint now has a durable backup receipt plus the retained actual-lane dashboard and alert surface."
            } else {
                "Retained summary records that the latest accepted checkpoint backup was refused and the retained dashboard now carries that active operator alert."
            });
            retained_summary.validate()?;
            let closeout_bundle = PsionActualPretrainingCloseoutBundle {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                closeout_state: current_status.phase.clone(),
                retained_paths: retained_paths.clone(),
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                workspace_status_sha256: workspace_status_sha256.clone(),
                evidence_artifacts: Vec::new(),
                closeout_gates: Vec::new(),
                failure_drills: Vec::new(),
                can_now_claim: Vec::new(),
                still_out_of_scope: Vec::new(),
                claim_boundary: String::from(if backup_receipt.backup_state == "backed_up" {
                    "This provisional closeout bundle records that the latest accepted checkpoint now has retained backup evidence plus a retained dashboard and alert surface. It does not yet claim external alert delivery, streaming dashboards, or final closeout completion."
                } else {
                    "This provisional closeout bundle records that the latest accepted checkpoint backup was refused and the retained dashboard now carries that operator-visible alert. It does not yet claim external alert delivery, streaming dashboards, or final closeout completion."
                }),
                detail: String::from(if backup_receipt.backup_state == "backed_up" {
                    "Checkpoint-backed-up closeout bundle repeats launcher provenance after durable backup replay for the accepted checkpoint."
                } else {
                    "Checkpoint-backup-refused closeout bundle repeats launcher provenance after the retained backup refusal receipt and failure drill were written."
                }),
            };
            closeout_bundle.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.current_status_path),
                &current_status,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.retained_summary_path),
                &retained_summary,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.closeout_bundle_path),
                &closeout_bundle,
            )?;
            write_dashboard_bundle(
                &run_root,
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                &hardware_qualification,
                &run_shape_qualification,
                &contracts.systems_bundle,
                Some(&backup_receipt),
                checkpoint_eval_decision.as_ref(),
                checkpoint_eval_failure.as_ref(),
                latest_redacted_alert.as_ref(),
            )?;
            append_launcher_log(
                &run_root,
                &format!(
                    "{} phase={} surface_id={} git_commit_sha={} checkpoint_label={} checkpoint_step={}\n",
                    now_utc(repo_root)?,
                    phase,
                    "psion_actual_pretraining.backup",
                    git_commit_sha,
                    checkpoint_pointer.checkpoint_label,
                    checkpoint_pointer.optimizer_step
                ),
            )?;
            println!(
                "status={}",
                if backup_receipt.backup_state == "backed_up" {
                    "checkpoint_backed_up"
                } else {
                    "checkpoint_backup_refused"
                }
            );
            println!("run_id={}", checkpoint_pointer.run_id);
            println!("run_root={}", run_root.display());
            println!(
                "checkpoint_backup_receipt={}",
                run_root
                    .join(&retained_paths.latest_checkpoint_backup_receipt_path)
                    .display()
            );
            println!(
                "dashboard={}",
                run_root
                    .join(&retained_paths.current_dashboard_path)
                    .display()
            );
            println!(
                "active_alert_feed={}",
                run_root
                    .join(&retained_paths.active_alert_feed_path)
                    .display()
            );
            if let Some(failure_drill) = failure_drill {
                println!(
                    "checkpoint_failure_drill={}",
                    checkpoint_failure_drill_path(&run_root, &failure_drill.drill_kind).display()
                );
            }
        }
        Cli::DecideContinueRestart {
            run_root,
            selected_git_ref,
            allow_dirty_tree,
        } => {
            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let retained_paths = retained_paths();
            let pointer_path = run_root.join(&retained_paths.latest_checkpoint_pointer_path);
            let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
                load_json(&pointer_path)?;
            checkpoint_pointer.validate()?;
            let checkpoint_manifest = validate_resume_candidate(&run_root, &checkpoint_pointer)
                .map_err(std::io::Error::other)?;
            let checkpoint_manifest_path =
                run_root.join(&checkpoint_manifest.relative_manifest_path);
            let checkpoint_manifest_artifact =
                run_artifact_ref(&run_root, &checkpoint_manifest_path)?;
            let checkpoint_pointer_artifact = run_artifact_ref(&run_root, &pointer_path)?;
            let mut current_status: PsionActualPretrainingCurrentRunStatus =
                load_json(&run_root.join(&retained_paths.current_status_path))?;
            current_status.validate()?;
            let mut retained_summary: PsionActualPretrainingRetainedSummary =
                load_json(&run_root.join(&retained_paths.retained_summary_path))?;
            retained_summary.validate()?;
            let hardware_qualification: PsionActualPretrainingHardwareQualification =
                load_json(&run_root.join(&retained_paths.hardware_qualification_path))?;
            hardware_qualification.validate()?;
            let run_shape_qualification: PsionActualPretrainingRunShapeQualification =
                load_json(&run_root.join(&retained_paths.run_shape_qualification_path))?;
            run_shape_qualification.validate()?;
            let checkpoint_backup_receipt: Option<PsionActualPretrainingCheckpointBackupReceipt> =
                load_optional_json(
                    &run_root.join(&retained_paths.latest_checkpoint_backup_receipt_path),
                )?;
            if let Some(receipt) = &checkpoint_backup_receipt {
                receipt.validate()?;
            }
            let checkpoint_eval_decision: Option<PsionActualPretrainingCheckpointEvalDecision> =
                load_optional_json(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
                )?;
            if let Some(decision) = &checkpoint_eval_decision {
                decision.validate()?;
            }
            let checkpoint_eval_failure: Option<PsionActualPretrainingCheckpointEvalFailure> =
                load_optional_json(
                    &run_root.join(&retained_paths.latest_checkpoint_eval_failure_path),
                )?;
            if let Some(failure) = &checkpoint_eval_failure {
                failure.validate()?;
            }
            let latest_redacted_alert: Option<PsionActualPretrainingRedactedAlert> =
                load_optional_json(&run_root.join(&retained_paths.latest_redacted_alert_path))?;
            if let Some(alert) = &latest_redacted_alert {
                alert.validate()?;
            }
            let checkpoint_backup_receipt_artifact = if run_root
                .join(&retained_paths.latest_checkpoint_backup_receipt_path)
                .is_file()
            {
                Some(run_artifact_ref(
                    &run_root,
                    &run_root.join(&retained_paths.latest_checkpoint_backup_receipt_path),
                )?)
            } else {
                None
            };
            let checkpoint_eval_decision_artifact = if run_root
                .join(&retained_paths.latest_checkpoint_eval_decision_path)
                .is_file()
            {
                Some(run_artifact_ref(
                    &run_root,
                    &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
                )?)
            } else {
                None
            };
            let checkpoint_eval_failure_artifact = if run_root
                .join(&retained_paths.latest_checkpoint_eval_failure_path)
                .is_file()
            {
                Some(run_artifact_ref(
                    &run_root,
                    &run_root.join(&retained_paths.latest_checkpoint_eval_failure_path),
                )?)
            } else {
                None
            };
            let hardware_qualification_artifact = run_artifact_ref(
                &run_root,
                &run_root.join(&retained_paths.hardware_qualification_path),
            )?;
            let run_shape_qualification_artifact = run_artifact_ref(
                &run_root,
                &run_root.join(&retained_paths.run_shape_qualification_path),
            )?;
            let checkpoint_comparison = record_psion_actual_pretraining_checkpoint_comparison(
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
                checkpoint_pointer_artifact,
                &checkpoint_pointer,
                checkpoint_manifest_artifact,
                checkpoint_backup_receipt_artifact.clone(),
                checkpoint_backup_receipt.as_ref(),
                checkpoint_eval_decision_artifact.clone(),
                checkpoint_eval_decision.as_ref(),
                checkpoint_eval_failure_artifact.clone(),
                checkpoint_eval_failure.as_ref(),
                hardware_qualification_artifact.clone(),
                &hardware_qualification,
                run_shape_qualification_artifact.clone(),
                &run_shape_qualification,
                contracts.systems_bundle_ref.clone(),
                &contracts.systems_bundle,
                "This retained checkpoint comparison binds the latest accepted checkpoint to the frozen checkpoint-eval, backup, hardware, run-shape, and systems receipts before the actual lane decides whether to continue, hold, or restart. It does not claim that the operator already performed the chosen action.",
                "Checkpoint comparison records the explicit continue threshold against the trusted-cluster throughput anchor and the retained actual-lane checkpoint lineage.",
            )?;
            let checkpoint_comparison_relative_path =
                checkpoint_comparison_relative_path(checkpoint_pointer.optimizer_step);
            write_json_pretty(
                &run_root.join(&checkpoint_comparison_relative_path),
                &checkpoint_comparison,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.latest_checkpoint_comparison_path),
                &checkpoint_comparison,
            )?;
            let checkpoint_comparison_artifact = run_artifact_ref(
                &run_root,
                &run_root.join(&checkpoint_comparison_relative_path),
            )?;
            let continue_restart_decision =
                record_psion_actual_pretraining_continue_restart_decision(
                    &selected_git_ref,
                    &git_commit_sha,
                    &dirty_tree_admission,
                    workspace_status_sha256.clone(),
                    &checkpoint_pointer,
                    checkpoint_comparison_artifact,
                    &checkpoint_comparison,
                    checkpoint_backup_receipt_artifact,
                    checkpoint_eval_decision_artifact,
                    checkpoint_eval_decision.as_ref(),
                    checkpoint_eval_failure_artifact,
                    checkpoint_eval_failure.as_ref(),
                    hardware_qualification_artifact,
                    run_shape_qualification_artifact,
                    contracts.systems_bundle_ref.clone(),
                    "This retained continue-restart decision keeps long-run operator posture machine-readable under the actual-lane evidence family. It does not claim that the operator already restarted or continued the cluster run; it only records the bounded next action.",
                    "Continue-restart decision consumes retained eval, backup, hardware, run-shape, and systems evidence before the operator chooses the next long-run action.",
                )?;
            let continue_restart_decision_relative_path =
                continue_restart_decision_relative_path(checkpoint_pointer.optimizer_step);
            write_json_pretty(
                &run_root.join(&continue_restart_decision_relative_path),
                &continue_restart_decision,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.latest_continue_restart_decision_path),
                &continue_restart_decision,
            )?;
            let phase = match continue_restart_decision.decision_state.as_str() {
                "continue" => "continue_decision_recorded",
                "restart_from_last_accepted_checkpoint" => "restart_decision_recorded",
                _ => "hold_decision_recorded",
            };
            current_status.phase = String::from(phase);
            current_status.latest_checkpoint_label = checkpoint_pointer.checkpoint_label.clone();
            current_status.last_completed_step = checkpoint_pointer.optimizer_step;
            current_status.updated_at_utc = now_utc(repo_root)?;
            current_status.detail = String::from(
                match continue_restart_decision.decision_state.as_str() {
                    "continue" => {
                        "Current status records that the latest accepted checkpoint remains admitted for uninterrupted continuation after the actual lane consumed backup, eval, hardware, and run-shape receipts."
                    }
                    "restart_from_last_accepted_checkpoint" => {
                        "Current status records that the latest accepted checkpoint should be used for restart after the actual lane consumed explicit eval, backup, hardware, and run-shape evidence."
                    }
                    _ => {
                        "Current status records that the actual lane is paused for operator investigation because the retained continue-restart comparison found blocking evidence."
                    }
                },
            );
            current_status.validate()?;
            retained_summary.last_known_phase = current_status.phase.clone();
            retained_summary.selected_git_ref = selected_git_ref.clone();
            retained_summary.git_commit_sha = git_commit_sha.clone();
            retained_summary.dirty_tree_admission = dirty_tree_admission.clone();
            retained_summary.detail = String::from(
                match continue_restart_decision.decision_state.as_str() {
                    "continue" => {
                        "Retained summary records that the latest accepted checkpoint is still admitted for continuation after explicit comparison against eval, backup, hardware, and throughput receipts."
                    }
                    "restart_from_last_accepted_checkpoint" => {
                        "Retained summary records that the actual lane should restart from the latest accepted checkpoint after the retained comparison consumed backup and systems evidence."
                    }
                    _ => {
                        "Retained summary records that the actual lane must pause and review the retained comparison before continuing or restarting."
                    }
                },
            );
            retained_summary.validate()?;
            let closeout_bundle = PsionActualPretrainingCloseoutBundle {
                schema_version: String::from(
                    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
                ),
                lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
                run_id: checkpoint_pointer.run_id.clone(),
                closeout_state: current_status.phase.clone(),
                retained_paths: retained_paths.clone(),
                selected_git_ref: selected_git_ref.clone(),
                git_commit_sha: git_commit_sha.clone(),
                dirty_tree_admission: dirty_tree_admission.clone(),
                workspace_status_sha256: workspace_status_sha256.clone(),
                evidence_artifacts: Vec::new(),
                closeout_gates: Vec::new(),
                failure_drills: Vec::new(),
                can_now_claim: Vec::new(),
                still_out_of_scope: Vec::new(),
                claim_boundary: String::from(
                    match continue_restart_decision.decision_state.as_str() {
                        "continue" => {
                            "This provisional closeout bundle now records one retained continue decision over the latest accepted checkpoint. It does not claim that the operator already advanced the distributed run beyond this decision."
                        }
                        "restart_from_last_accepted_checkpoint" => {
                            "This provisional closeout bundle now records one retained restart decision over the latest accepted checkpoint. It does not claim that the operator already performed the restart."
                        }
                        _ => {
                            "This provisional closeout bundle now records one retained hold-and-investigate decision over the latest accepted checkpoint. It does not claim that the operator already resolved the blocking evidence."
                        }
                    },
                ),
                detail: String::from(match continue_restart_decision.decision_state.as_str() {
                    "continue" => {
                        "Continue-decision closeout bundle repeats launcher provenance after the actual lane compared the latest checkpoint against eval, backup, and system receipts."
                    }
                    "restart_from_last_accepted_checkpoint" => {
                        "Restart-decision closeout bundle repeats launcher provenance after the actual lane compared the latest checkpoint against eval, backup, and system receipts."
                    }
                    _ => {
                        "Hold-decision closeout bundle repeats launcher provenance after the actual lane retained blocking comparison rows for operator review."
                    }
                }),
            };
            closeout_bundle.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.current_status_path),
                &current_status,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.retained_summary_path),
                &retained_summary,
            )?;
            write_json_pretty(
                &run_root.join(&retained_paths.closeout_bundle_path),
                &closeout_bundle,
            )?;
            write_dashboard_bundle(
                &run_root,
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                &hardware_qualification,
                &run_shape_qualification,
                &contracts.systems_bundle,
                checkpoint_backup_receipt.as_ref(),
                checkpoint_eval_decision.as_ref(),
                checkpoint_eval_failure.as_ref(),
                latest_redacted_alert.as_ref(),
            )?;
            append_launcher_log(
                &run_root,
                &format!(
                    "{} phase={} surface_id={} git_commit_sha={} checkpoint_label={} checkpoint_step={} decision_state={} operator_action={}\n",
                    now_utc(repo_root)?,
                    phase,
                    "psion_actual_pretraining.decide_continue_restart",
                    git_commit_sha,
                    checkpoint_pointer.checkpoint_label,
                    checkpoint_pointer.optimizer_step,
                    continue_restart_decision.decision_state,
                    continue_restart_decision.operator_action
                ),
            )?;
            println!("status={phase}");
            println!("run_id={}", checkpoint_pointer.run_id);
            println!("run_root={}", run_root.display());
            println!(
                "checkpoint_comparison={}",
                run_root
                    .join(&retained_paths.latest_checkpoint_comparison_path)
                    .display()
            );
            println!(
                "continue_restart_decision={}",
                run_root
                    .join(&retained_paths.latest_continue_restart_decision_path)
                    .display()
            );
            println!(
                "decision_state={}",
                continue_restart_decision.decision_state
            );
            println!(
                "operator_action={}",
                continue_restart_decision.operator_action
            );
        }
        Cli::RehearseBaseLane {
            run_id,
            run_root,
            selected_git_ref,
            hardware_observation_path,
            run_shape_observation_path,
            allow_dirty_tree,
            remote_host,
            secondary_remote_host,
            planned_interruption_step,
            cleanup_remote,
        } => {
            let current_exe = env::current_exe()?;
            run_rehearsal_subcommand(
                &current_exe,
                "start",
                &run_id,
                &run_root,
                &selected_git_ref,
                hardware_observation_path.as_deref(),
                run_shape_observation_path.as_deref(),
                allow_dirty_tree,
                &[],
            )?;
            let distributed_rehearsal = match remote_host.as_deref() {
                Some(remote_host) => Some(run_actual_pretraining_distributed_bringup_rehearsal(
                    repo_root,
                    &run_root,
                    &run_id,
                    &selected_git_ref,
                    remote_host,
                    secondary_remote_host.as_deref(),
                    planned_interruption_step,
                    cleanup_remote,
                )?),
                None => None,
            };
            let mut record_checkpoint_args = Vec::new();
            if let Some(rehearsal) = distributed_rehearsal.as_ref() {
                record_checkpoint_args.extend([
                    String::from("--checkpoint-label"),
                    format!(
                        "bounded-actual-pretraining-bringup-step-{}",
                        rehearsal.optimizer_steps
                    ),
                    String::from("--optimizer-step"),
                    rehearsal.optimizer_steps.to_string(),
                    String::from("--checkpoint-ref"),
                    format!(
                        "checkpoint://psion/actual-pretraining/bounded-bringup/{}",
                        rehearsal.checkpoint_ref
                    ),
                    String::from("--checkpoint-object-digest"),
                    rehearsal.checkpoint_object_digest.clone(),
                    String::from("--checkpoint-total-bytes"),
                    rehearsal.checkpoint_total_bytes.to_string(),
                ]);
            } else {
                record_checkpoint_args.extend([
                    String::from("--checkpoint-label"),
                    String::from("broader-pretrain-final"),
                    String::from("--optimizer-step"),
                    String::from("16384"),
                    String::from("--checkpoint-ref"),
                    String::from("checkpoint://psion/actual-pretraining/broader-pretrain-final"),
                ]);
            }
            run_rehearsal_subcommand(
                &current_exe,
                "record-checkpoint",
                &run_id,
                &run_root,
                &selected_git_ref,
                None,
                None,
                allow_dirty_tree,
                &record_checkpoint_args,
            )?;
            run_rehearsal_subcommand(
                &current_exe,
                "backup",
                &run_id,
                &run_root,
                &selected_git_ref,
                None,
                None,
                allow_dirty_tree,
                &[String::from("--inject-failed-upload")],
            )?;
            run_rehearsal_subcommand(
                &current_exe,
                "backup",
                &run_id,
                &run_root,
                &selected_git_ref,
                None,
                None,
                allow_dirty_tree,
                &[],
            )?;
            run_rehearsal_subcommand(
                &current_exe,
                "decide-continue-restart",
                &run_id,
                &run_root,
                &selected_git_ref,
                None,
                None,
                allow_dirty_tree,
                &[],
            )?;
            run_rehearsal_subcommand(
                &current_exe,
                "resume",
                &run_id,
                &run_root,
                &selected_git_ref,
                hardware_observation_path.as_deref(),
                run_shape_observation_path.as_deref(),
                allow_dirty_tree,
                &[],
            )?;

            let git_commit_sha = git_output(repo_root, &["rev-parse", selected_git_ref.as_str()])?;
            let (dirty_tree_admission, workspace_status_sha256) =
                dirty_tree_posture(repo_root, allow_dirty_tree)?;
            let retained_paths = retained_paths();
            let mut current_status: PsionActualPretrainingCurrentRunStatus =
                load_json(&run_root.join(&retained_paths.current_status_path))?;
            current_status.phase = String::from("base_lane_rehearsal_complete");
            current_status.detail = match distributed_rehearsal.as_ref() {
                Some(rehearsal) if rehearsal.recovery_drill_receipt_path.is_some() => format!(
                    "Base-lane rehearsal completed one retained launch, one bounded {} distributed training proof, one planned interruption and recovery drill, accepted checkpoint retention, backup recovery drill, continue decision, and resume cycle for the actual pretraining lane.",
                    rehearsal.execution_topology_classification
                ),
                Some(rehearsal) => format!(
                    "Base-lane rehearsal completed one retained launch, one bounded {} distributed training proof, accepted checkpoint retention, backup recovery drill, continue decision, and resume cycle for the actual pretraining lane.",
                    rehearsal.execution_topology_classification
                ),
                None => String::from(
                    "Base-lane rehearsal completed one retained launch, accepted checkpoint, backup recovery drill, continue decision, and resume cycle for the actual pretraining lane.",
                ),
            };
            current_status.updated_at_utc = now_utc(repo_root)?;
            current_status.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.current_status_path),
                &current_status,
            )?;

            let mut retained_summary: PsionActualPretrainingRetainedSummary =
                load_json(&run_root.join(&retained_paths.retained_summary_path))?;
            retained_summary.last_known_phase = current_status.phase.clone();
            retained_summary.claim_boundary = match distributed_rehearsal.as_ref() {
                Some(rehearsal) if rehearsal.recovery_drill_receipt_path.is_some() => String::from(
                    "The retained summary now proves one full base-lane rehearsal for the actual pretraining operator path, including launch, one bounded multi-host distributed training proof, one planned interruption and recovery drill on the same checkpoint lineage, accepted checkpoint retention, backup recovery drill, continue-vs-restart decision, and resume. It does not claim external alert delivery, streaming dashboard publication, broader actual-lane distributed cluster execution beyond that bounded rehearsal segment, or plugin-conditioned continuation execution.",
                ),
                Some(_) => String::from(
                    "The retained summary now proves one full base-lane rehearsal for the actual pretraining operator path, including launch, one bounded multi-host distributed training proof, accepted checkpoint retention, backup recovery drill, continue-vs-restart decision, and resume. It does not claim external alert delivery, streaming dashboard publication, broader actual-lane distributed cluster execution beyond that bounded rehearsal segment, or plugin-conditioned continuation execution.",
                ),
                None => String::from(
                    "The retained summary now proves one full base-lane rehearsal for the actual pretraining operator path, including launch, accepted checkpoint retention, backup recovery drill, continue-vs-restart decision, and resume. It does not claim external alert delivery, streaming dashboard publication, distributed cluster execution, or plugin-conditioned continuation execution.",
                ),
            };
            retained_summary.detail = match distributed_rehearsal.as_ref() {
                Some(rehearsal) if rehearsal.recovery_drill_receipt_path.is_some() => format!(
                    "Retained summary now points at the exact base-lane rehearsal closeout state plus the bounded {} training proof and retained recovery drill for operator review.",
                    rehearsal.execution_topology_classification
                ),
                Some(rehearsal) => format!(
                    "Retained summary now points at the exact base-lane rehearsal closeout state plus the bounded {} training proof for operator review.",
                    rehearsal.execution_topology_classification
                ),
                None => String::from(
                    "Retained summary now points at the exact base-lane rehearsal closeout state for operator review.",
                ),
            };
            retained_summary.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.retained_summary_path),
                &retained_summary,
            )?;

            let hardware_qualification: PsionActualPretrainingHardwareQualification =
                load_json(&run_root.join(&retained_paths.hardware_qualification_path))?;
            let run_shape_qualification: PsionActualPretrainingRunShapeQualification =
                load_json(&run_root.join(&retained_paths.run_shape_qualification_path))?;
            let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
                load_json(&run_root.join(&retained_paths.latest_checkpoint_pointer_path))?;
            let checkpoint_manifest_path = run_root.join(
                checkpoint_pointer
                    .checkpoint_manifest_relative_path
                    .as_deref()
                    .ok_or_else(|| {
                        std::io::Error::other(
                            "base-lane rehearsal closeout requires checkpoint manifest path",
                        )
                    })?,
            );
            let checkpoint_backup_receipt: PsionActualPretrainingCheckpointBackupReceipt =
                load_json(&run_root.join(&retained_paths.latest_checkpoint_backup_receipt_path))?;
            let checkpoint_eval_decision: PsionActualPretrainingCheckpointEvalDecision =
                load_json(&run_root.join(&retained_paths.latest_checkpoint_eval_decision_path))?;
            let checkpoint_comparison: PsionActualPretrainingCheckpointComparison =
                load_json(&run_root.join(&retained_paths.latest_checkpoint_comparison_path))?;
            let continue_restart_decision: PsionActualPretrainingContinueRestartDecision =
                load_json(&run_root.join(&retained_paths.latest_continue_restart_decision_path))?;
            let auto_resume_receipt: PsionActualPretrainingAutoResumeReceipt =
                load_json(&run_root.join(&retained_paths.auto_resume_receipt_path))?;
            let continuation_handoff: PsionActualPretrainingContinuationHandoff =
                load_json(&run_root.join(&retained_paths.continuation_handoff_path))?;
            checkpoint_backup_receipt.validate()?;
            checkpoint_eval_decision.validate()?;
            checkpoint_comparison.validate()?;
            continue_restart_decision.validate()?;
            auto_resume_receipt.validate()?;
            continuation_handoff.validate()?;
            if checkpoint_backup_receipt.backup_state != "backed_up" {
                return Err(std::io::Error::other(
                    "base-lane rehearsal closeout requires a successful final backup receipt",
                )
                .into());
            }
            if checkpoint_eval_decision.decision_state != "continue" {
                return Err(std::io::Error::other(
                    "base-lane rehearsal closeout requires a `continue` checkpoint eval decision",
                )
                .into());
            }
            if continue_restart_decision.decision_state != "continue" {
                return Err(std::io::Error::other(
                    "base-lane rehearsal closeout requires a `continue` long-run decision",
                )
                .into());
            }
            if auto_resume_receipt.resolution_state == "refused" {
                return Err(std::io::Error::other(
                    "base-lane rehearsal closeout requires a successful auto-resume receipt",
                )
                .into());
            }
            write_dashboard_bundle(
                &run_root,
                &current_status,
                &retained_summary,
                &checkpoint_pointer,
                &hardware_qualification,
                &run_shape_qualification,
                &contracts.systems_bundle,
                Some(&checkpoint_backup_receipt),
                Some(&checkpoint_eval_decision),
                None,
                None,
            )?;

            let failed_upload_drill_path =
                checkpoint_failure_drill_path(&run_root, "failed_upload");
            let closeout_bundle = build_base_lane_rehearsal_closeout_bundle(
                &run_root,
                &retained_paths,
                &checkpoint_pointer.run_id,
                &selected_git_ref,
                &git_commit_sha,
                &dirty_tree_admission,
                workspace_status_sha256.clone(),
                &checkpoint_manifest_path,
                failed_upload_drill_path.as_path(),
                distributed_rehearsal.as_ref(),
            )?;
            closeout_bundle.validate()?;
            write_json_pretty(
                &run_root.join(&retained_paths.closeout_bundle_path),
                &closeout_bundle,
            )?;
            append_launcher_log(
                &run_root,
                &format!(
                    "{} phase=base_lane_rehearsal_complete surface_id=psion_actual_pretraining.rehearse_base_lane git_commit_sha={} run_id={}\n",
                    now_utc(repo_root)?,
                    git_commit_sha,
                    checkpoint_pointer.run_id
                ),
            )?;
            println!("status=base_lane_rehearsal_complete");
            println!("run_id={}", checkpoint_pointer.run_id);
            println!("run_root={}", run_root.display());
            println!(
                "closeout_bundle={}",
                run_root
                    .join(&retained_paths.closeout_bundle_path)
                    .display()
            );
            println!(
                "checkpoint_eval_decision={}",
                checkpoint_eval_decision.decision_state
            );
            println!(
                "continue_restart_decision={}",
                continue_restart_decision.decision_state
            );
            println!("failure_drill={}", failed_upload_drill_path.display());
            if let Some(rehearsal) = distributed_rehearsal.as_ref() {
                println!(
                    "distributed_bringup={}",
                    run_root.join(&rehearsal.rehearsal_summary_path).display()
                );
                if let Some(recovery_drill_receipt_path) =
                    rehearsal.recovery_drill_receipt_path.as_deref()
                {
                    println!(
                        "distributed_recovery_drill={}",
                        run_root.join(recovery_drill_receipt_path).display()
                    );
                }
                println!(
                    "distributed_topology={}",
                    rehearsal.execution_topology_classification
                );
            }
        }
    }

    Ok(())
}

pub fn run_from_env() -> Result<(), Box<dyn Error>> {
    run_with_args(env::args().skip(1))
}

fn main() -> Result<(), Box<dyn Error>> {
    run_from_env()
}

fn parse_cli_from_args<I, S>(args: I) -> Result<Cli, Box<dyn Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let Some(command) = args.next() else {
        usage();
        return Err(std::io::Error::other("missing subcommand").into());
    };

    let mut run_id = String::new();
    let mut output_root = String::new();
    let mut run_root = String::new();
    let mut git_ref = String::new();
    let mut checkpoint_label = String::new();
    let mut checkpoint_ref = String::new();
    let mut checkpoint_object_digest = String::new();
    let mut optimizer_step: Option<u64> = None;
    let mut checkpoint_total_bytes: Option<u64> = None;
    let mut hardware_observation_path: Option<PathBuf> = None;
    let mut run_shape_observation_path: Option<PathBuf> = None;
    let mut remote_host = String::new();
    let mut secondary_remote_host = String::new();
    let mut planned_interruption_step: Option<u64> = None;
    let mut allow_dirty_tree = false;
    let mut dry_run = false;
    let mut cleanup_remote = false;
    let mut inject_failed_upload = false;
    let mut inject_eval_worker_unavailable = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run-id" => {
                run_id = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--run-id requires a value"))?;
            }
            "--output-root" => {
                output_root = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--output-root requires a value"))?;
            }
            "--run-root" => {
                run_root = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--run-root requires a value"))?;
            }
            "--git-ref" => {
                git_ref = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--git-ref requires a value"))?;
            }
            "--checkpoint-label" => {
                checkpoint_label = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--checkpoint-label requires a value"))?;
            }
            "--checkpoint-ref" => {
                checkpoint_ref = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--checkpoint-ref requires a value"))?;
            }
            "--optimizer-step" => {
                optimizer_step = Some(
                    args.next()
                        .ok_or_else(|| std::io::Error::other("--optimizer-step requires a value"))?
                        .parse::<u64>()?,
                );
            }
            "--checkpoint-object-digest" => {
                checkpoint_object_digest = args.next().ok_or_else(|| {
                    std::io::Error::other("--checkpoint-object-digest requires a value")
                })?;
            }
            "--checkpoint-total-bytes" => {
                checkpoint_total_bytes = Some(
                    args.next()
                        .ok_or_else(|| {
                            std::io::Error::other("--checkpoint-total-bytes requires a value")
                        })?
                        .parse::<u64>()?,
                );
            }
            "--hardware-observation" => {
                hardware_observation_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    std::io::Error::other("--hardware-observation requires a value")
                })?));
            }
            "--run-shape-observation" => {
                run_shape_observation_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    std::io::Error::other("--run-shape-observation requires a value")
                })?));
            }
            "--remote-host" => {
                remote_host = args
                    .next()
                    .ok_or_else(|| std::io::Error::other("--remote-host requires a value"))?;
            }
            "--secondary-remote-host" => {
                secondary_remote_host = args.next().ok_or_else(|| {
                    std::io::Error::other("--secondary-remote-host requires a value")
                })?;
            }
            "--planned-interruption-step" => {
                planned_interruption_step = Some(
                    args.next()
                        .ok_or_else(|| {
                            std::io::Error::other(
                                "--planned-interruption-step requires a value",
                            )
                        })?
                        .parse::<u64>()?,
                );
            }
            "--allow-dirty-tree" => allow_dirty_tree = true,
            "--dry-run" => dry_run = true,
            "--cleanup-remote" => cleanup_remote = true,
            "--inject-failed-upload" => inject_failed_upload = true,
            "--inject-eval-worker-unavailable" => inject_eval_worker_unavailable = true,
            "--help" | "-h" => {
                usage();
                std::process::exit(0);
            }
            other => {
                return Err(std::io::Error::other(format!("unknown argument `{other}`")).into());
            }
        }
    }

    let repo_root = workspace_root()?;
    let selected_git_ref = if git_ref.is_empty() {
        match git_output(repo_root.as_path(), &["symbolic-ref", "-q", "HEAD"]) {
            Ok(value) if !value.is_empty() => value,
            _ => String::from("HEAD"),
        }
    } else {
        git_ref
    };

    match command.as_str() {
        "start" => {
            let run_id = if run_id.is_empty() {
                format!(
                    "psion-actual-pretraining-{}",
                    timestamp_utc(repo_root.as_path())?
                )
            } else {
                run_id
            };
            let run_root = if output_root.is_empty() {
                PathBuf::from(env::var("HOME").unwrap_or_else(|_| String::from(".")))
                    .join("scratch/psion_actual_pretraining_runs")
                    .join(&run_id)
            } else {
                PathBuf::from(output_root)
            };
            Ok(Cli::Start {
                run_id,
                run_root,
                selected_git_ref,
                hardware_observation_path,
                run_shape_observation_path,
                allow_dirty_tree,
                dry_run,
            })
        }
        "resume" => {
            if run_root.is_empty() {
                return Err(std::io::Error::other("resume requires --run-root <path>").into());
            }
            Ok(Cli::Resume {
                run_root: PathBuf::from(run_root),
                selected_git_ref,
                hardware_observation_path,
                run_shape_observation_path,
                allow_dirty_tree,
                dry_run,
            })
        }
        "record-checkpoint" => {
            if run_root.is_empty() {
                return Err(
                    std::io::Error::other("record-checkpoint requires --run-root <path>").into(),
                );
            }
            if checkpoint_label.is_empty() {
                return Err(std::io::Error::other(
                    "record-checkpoint requires --checkpoint-label <label>",
                )
                .into());
            }
            let optimizer_step = optimizer_step.ok_or_else(|| {
                std::io::Error::other("record-checkpoint requires --optimizer-step <step>")
            })?;
            if checkpoint_ref.is_empty() {
                return Err(std::io::Error::other(
                    "record-checkpoint requires --checkpoint-ref <ref>",
                )
                .into());
            }
            Ok(Cli::RecordCheckpoint {
                run_root: PathBuf::from(run_root),
                selected_git_ref,
                checkpoint_label,
                optimizer_step,
                checkpoint_ref,
                checkpoint_object_digest: if checkpoint_object_digest.is_empty() {
                    None
                } else {
                    Some(checkpoint_object_digest)
                },
                checkpoint_total_bytes,
                inject_eval_worker_unavailable,
                allow_dirty_tree,
            })
        }
        "backup" => {
            if run_root.is_empty() {
                return Err(std::io::Error::other("backup requires --run-root <path>").into());
            }
            Ok(Cli::Backup {
                run_root: PathBuf::from(run_root),
                selected_git_ref,
                allow_dirty_tree,
                inject_failed_upload,
            })
        }
        "decide-continue-restart" => {
            if run_root.is_empty() {
                return Err(std::io::Error::other(
                    "decide-continue-restart requires --run-root <path>",
                )
                .into());
            }
            Ok(Cli::DecideContinueRestart {
                run_root: PathBuf::from(run_root),
                selected_git_ref,
                allow_dirty_tree,
            })
        }
        "rehearse-base-lane" => {
            let run_id = if run_id.is_empty() {
                format!(
                    "psion-actual-pretraining-rehearsal-{}",
                    timestamp_utc(repo_root.as_path())?
                )
            } else {
                run_id
            };
            let run_root = if output_root.is_empty() {
                PathBuf::from(env::var("HOME").unwrap_or_else(|_| String::from(".")))
                    .join("scratch/psion_actual_pretraining_runs")
                    .join(&run_id)
            } else {
                PathBuf::from(output_root)
            };
            Ok(Cli::RehearseBaseLane {
                run_id,
                run_root,
                selected_git_ref,
                hardware_observation_path,
                run_shape_observation_path,
                allow_dirty_tree,
                remote_host: if remote_host.is_empty() {
                    if secondary_remote_host.is_empty() {
                        None
                    } else {
                        Some(String::from("archlinux"))
                    }
                } else {
                    Some(remote_host)
                },
                secondary_remote_host: if secondary_remote_host.is_empty() {
                    None
                } else {
                    Some(secondary_remote_host)
                },
                planned_interruption_step,
                cleanup_remote,
            })
        }
        _ => {
            usage();
            Err(std::io::Error::other(format!("unsupported subcommand `{command}`")).into())
        }
    }
}

fn usage() {
    eprintln!(
        "Usage:\n  psion_actual_pretraining_operator start [--run-id <id>] [--output-root <path>] [--git-ref <ref>] [--hardware-observation <path>] [--run-shape-observation <path>] [--allow-dirty-tree] [--dry-run]\n  psion_actual_pretraining_operator record-checkpoint --run-root <path> --checkpoint-label <label> --optimizer-step <step> --checkpoint-ref <ref> [--checkpoint-object-digest <digest>] [--checkpoint-total-bytes <bytes>] [--git-ref <ref>] [--allow-dirty-tree] [--inject-eval-worker-unavailable]\n  psion_actual_pretraining_operator backup --run-root <path> [--git-ref <ref>] [--allow-dirty-tree] [--inject-failed-upload]\n  psion_actual_pretraining_operator resume --run-root <path> [--git-ref <ref>] [--hardware-observation <path>] [--run-shape-observation <path>] [--allow-dirty-tree] [--dry-run]\n  psion_actual_pretraining_operator decide-continue-restart --run-root <path> [--git-ref <ref>] [--allow-dirty-tree]\n  psion_actual_pretraining_operator rehearse-base-lane [--run-id <id>] [--output-root <path>] [--git-ref <ref>] [--hardware-observation <path>] [--run-shape-observation <path>] [--remote-host <host>] [--secondary-remote-host <host>] [--planned-interruption-step <step>] [--cleanup-remote] [--allow-dirty-tree]"
    );
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn load_frozen_contracts(repo_root: &Path) -> Result<FrozenContracts, Box<dyn Error>> {
    let pretrain_dir = repo_root.join("fixtures/psion/pretrain");
    let lane_spec_path = pretrain_dir.join("psion_actual_pretraining_lane_spec_v1.json");
    let recipe_path = pretrain_dir.join("psion_actual_pretraining_recipe_bundle_v1.json");
    let baseline_tools_path =
        pretrain_dir.join("psion_actual_pretraining_baseline_tools_bundle_v1.json");
    let scaling_path = pretrain_dir.join("psion_actual_pretraining_scaling_bundle_v1.json");
    let data_path = pretrain_dir.join("psion_actual_pretraining_data_bundle_v1.json");
    let topology_path =
        pretrain_dir.join("psion_actual_pretraining_topology_storage_bundle_v1.json");
    let systems_path = pretrain_dir.join("psion_actual_pretraining_systems_bundle_v1.json");
    let evidence_path = pretrain_dir.join("psion_actual_pretraining_evidence_contract_v1.json");
    let checkpoint_eval_benchmark_path =
        repo_root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH);
    let lane_spec: PsionActualPretrainingLaneSpec = load_json(&lane_spec_path)?;
    lane_spec.validate()?;
    let recipe: PsionActualPretrainingRecipeBundle = load_json(&recipe_path)?;
    recipe.validate()?;
    let baseline_tools_bundle: PsionActualPretrainingBaselineToolsBundle =
        load_json(&baseline_tools_path)?;
    baseline_tools_bundle.validate()?;
    let scaling_bundle: PsionActualPretrainingScalingBundle = load_json(&scaling_path)?;
    scaling_bundle.validate()?;
    let data_bundle: PsionActualPretrainingDataBundle = load_json(&data_path)?;
    data_bundle.validate()?;
    let plugin_stage_manifest_path = repo_root.join(
        &recipe
            .continuation_target
            .plugin_conditioned_stage_manifest
            .path,
    );
    let plugin_conditioned_stage_manifest: PsionPluginConditionedSftStageManifest =
        load_json(&plugin_stage_manifest_path)?;
    let topology: PsionActualPretrainingTopologyStorageBundle = load_json(&topology_path)?;
    topology.validate()?;
    let systems_bundle: PsionActualPretrainingSystemsBundle = load_json(&systems_path)?;
    systems_bundle.validate()?;
    let evidence: PsionActualPretrainingEvidenceContract = load_json(&evidence_path)?;
    evidence.validate().map_err(map_evidence_error)?;
    build_psion_actual_pretraining_checkpoint_eval_benchmark_package()?.validate()?;

    Ok(FrozenContracts {
        lane_spec_ref: artifact_ref(repo_root, &lane_spec_path)?,
        recipe_bundle_ref: artifact_ref(repo_root, &recipe_path)?,
        baseline_tools_bundle_ref: artifact_ref(repo_root, &baseline_tools_path)?,
        scaling_bundle_ref: artifact_ref(repo_root, &scaling_path)?,
        data_bundle_ref: artifact_ref(repo_root, &data_path)?,
        systems_bundle_ref: artifact_ref(repo_root, &systems_path)?,
        topology_storage_bundle_ref: artifact_ref(repo_root, &topology_path)?,
        evidence_contract_ref: artifact_ref(repo_root, &evidence_path)?,
        checkpoint_eval_benchmark_fixture_ref: artifact_ref(
            repo_root,
            &checkpoint_eval_benchmark_path,
        )?,
        baseline_tools_bundle,
        data_bundle,
        recipe_bundle: recipe,
        plugin_conditioned_stage_manifest,
        topology,
        systems_bundle,
        evidence_contract: evidence,
    })
}

fn map_evidence_error(error: PsionActualPretrainingEvidenceContractError) -> Box<dyn Error> {
    Box::new(std::io::Error::other(error.to_string()))
}

fn retained_paths() -> PsionActualPretrainingRetainedPathSet {
    psion_actual_pretraining_retained_paths()
}

fn launcher_surfaces() -> PsionActualPretrainingLauncherSurfaces {
    PsionActualPretrainingLauncherSurfaces {
        start_surface_id: String::from(PSION_ACTUAL_PRETRAINING_START_SURFACE_ID),
        dry_run_surface_id: String::from(PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID),
        resume_surface_id: String::from(PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID),
        status_surface_id: String::from(PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID),
    }
}

fn run_roots(
    run_root: &Path,
    run_id: &str,
    topology: &PsionActualPretrainingTopologyStorageBundle,
) -> PsionActualPretrainingRunRoots {
    PsionActualPretrainingRunRoots {
        local_run_root: run_root.display().to_string(),
        remote_run_root: topology
            .remote_run_root_template
            .replace("<run_id>", run_id),
        remote_checkpoint_root: topology
            .remote_checkpoint_root_template
            .replace("<run_id>", run_id),
        remote_manifest_root: topology
            .remote_manifest_root_template
            .replace("<run_id>", run_id),
        remote_log_root: topology
            .remote_log_root_template
            .replace("<run_id>", run_id),
    }
}

fn credential_bindings(
    topology: &PsionActualPretrainingTopologyStorageBundle,
) -> Vec<PsionActualPretrainingCredentialBinding> {
    topology
        .credential_sources
        .iter()
        .map(|source| PsionActualPretrainingCredentialBinding {
            kind: source.kind.clone(),
            source_name: source.source_name.clone(),
            retained_redaction: source.retained_redaction.clone(),
        })
        .collect()
}

fn dirty_tree_posture(
    repo_root: &Path,
    allow_dirty_tree: bool,
) -> Result<(String, Option<String>), Box<dyn Error>> {
    let porcelain = git_output(repo_root, &["status", "--porcelain"])?;
    if porcelain.is_empty() {
        return Ok((String::from("refuse_by_default"), None));
    }
    if !allow_dirty_tree {
        return Err(std::io::Error::other(
            "dirty working trees are refused by default; rerun with --allow-dirty-tree to override",
        )
        .into());
    }
    let status_snapshot = git_output(repo_root, &["status", "--short", "--branch"])?;
    Ok((
        String::from("allowed_by_operator_override"),
        Some(sha256_hex(status_snapshot.as_bytes())),
    ))
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "git command failed: git -C {} {}",
            repo_root.display(),
            args.join(" ")
        ))
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn now_utc(repo_root: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("failed to get UTC time").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn timestamp_utc(repo_root: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("date")
        .arg("-u")
        .arg("+%Y%m%dT%H%M%SZ")
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("failed to get UTC timestamp").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn build_hardware_qualification(
    repo_root: &Path,
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    hardware_observation_path: Option<&Path>,
    contracts: &FrozenContracts,
) -> Result<PsionActualPretrainingHardwareQualification, Box<dyn Error>> {
    let resolved_hardware_observation_path =
        hardware_observation_path.map(|path| resolve_repo_path(repo_root, path));
    let observation_artifact = resolved_hardware_observation_path
        .as_deref()
        .map(|path| artifact_ref(repo_root, path))
        .transpose()?;
    let observation = match resolved_hardware_observation_path.as_deref() {
        Some(path) => {
            let observation: PsionActualPretrainingHardwareObservation = load_json(path)?;
            observation.validate()?;
            observation
        }
        None => probe_local_hardware_observation(
            repo_root,
            &contracts.topology,
            &contracts.systems_bundle,
        )?,
    };
    Ok(derive_psion_actual_pretraining_hardware_qualification(
        run_id,
        selected_git_ref,
        git_commit_sha,
        dirty_tree_admission,
        &observation,
        observation_artifact,
        contracts.topology_storage_bundle_ref.clone(),
        contracts.systems_bundle_ref.clone(),
        contracts.evidence_contract_ref.clone(),
        &contracts.topology,
        &contracts.systems_bundle,
        &contracts.evidence_contract,
    )?)
}

fn build_run_shape_qualification(
    repo_root: &Path,
    run_id: &str,
    run_root: &Path,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    run_shape_observation_path: Option<&Path>,
    contracts: &FrozenContracts,
) -> Result<PsionActualPretrainingRunShapeQualification, Box<dyn Error>> {
    let resolved_run_shape_observation_path =
        run_shape_observation_path.map(|path| resolve_repo_path(repo_root, path));
    let observation_artifact = resolved_run_shape_observation_path
        .as_deref()
        .map(|path| artifact_ref(repo_root, path))
        .transpose()?;
    let observation = match resolved_run_shape_observation_path.as_deref() {
        Some(path) => {
            let observation: PsionActualPretrainingRunShapeObservation = load_json(path)?;
            observation.validate()?;
            observation
        }
        None => probe_local_run_shape_observation(
            repo_root,
            run_root,
            &contracts.baseline_tools_bundle,
            &contracts.data_bundle,
            &contracts.systems_bundle,
        )?,
    };
    Ok(derive_psion_actual_pretraining_run_shape_qualification(
        run_id,
        selected_git_ref,
        git_commit_sha,
        dirty_tree_admission,
        &observation,
        observation_artifact,
        contracts.baseline_tools_bundle_ref.clone(),
        contracts.data_bundle_ref.clone(),
        contracts.systems_bundle_ref.clone(),
        contracts.evidence_contract_ref.clone(),
        &contracts.baseline_tools_bundle,
        &contracts.data_bundle,
        &contracts.systems_bundle,
        &contracts.evidence_contract,
    )?)
}

fn resolve_repo_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn probe_local_hardware_observation(
    repo_root: &Path,
    topology: &PsionActualPretrainingTopologyStorageBundle,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
) -> Result<PsionActualPretrainingHardwareObservation, Box<dyn Error>> {
    let now = now_utc(repo_root)?;
    let workers = probe_local_workers(repo_root)?;
    let credential_sources = topology
        .credential_sources
        .iter()
        .map(observe_credential_source)
        .collect::<Result<Vec<_>, _>>()?;
    let checkpoint_restore_ready = PathBuf::from(repo_root)
        .join(&systems_bundle.resume_rehearsal_support.recovery_bundle.path)
        .is_file();
    let backend = if workers.is_empty() {
        String::from("unavailable")
    } else {
        String::from("cuda")
    };
    let mut observation = PsionActualPretrainingHardwareObservation {
        schema_version: String::from(
            psionic_train::PSION_ACTUAL_PRETRAINING_HARDWARE_OBSERVATION_SCHEMA_VERSION,
        ),
        observation_id: format!("psion_actual_pretraining_local_hardware_probe::{now}"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        observation_kind: String::from("local_runtime_probe"),
        observed_at_utc: now,
        backend,
        workers,
        credential_sources,
        checkpoint_restore_ready,
        summary: String::from(
            "Local actual-lane hardware probe derived from nvidia-smi when available plus redacted credential-source presence checks.",
        ),
        observation_digest: String::new(),
    };
    observation.observation_digest =
        psionic_train::stable_hardware_observation_digest(&observation)?;
    observation.validate()?;
    Ok(observation)
}

fn probe_local_run_shape_observation(
    repo_root: &Path,
    run_root: &Path,
    baseline_tools_bundle: &PsionActualPretrainingBaselineToolsBundle,
    data_bundle: &PsionActualPretrainingDataBundle,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
) -> Result<PsionActualPretrainingRunShapeObservation, Box<dyn Error>> {
    let now = now_utc(repo_root)?;
    let actual_lane_accounting = baseline_tools_bundle
        .resource_accounting_rows
        .iter()
        .find(|row| row.scope_kind == "actual_lane")
        .ok_or_else(|| {
            std::io::Error::other(
                "baseline-tools bundle is missing the actual_lane resource-accounting row",
            )
        })?;
    let throughput_source = systems_bundle
        .throughput_baselines
        .iter()
        .find(|baseline| baseline.baseline_kind == "trusted_cluster_anchor")
        .ok_or_else(|| {
            std::io::Error::other(
                "systems bundle is missing the trusted_cluster_anchor throughput baseline",
            )
        })?;
    let storage_probe = probe_local_storage_probe(run_root)?;
    let mut observation = PsionActualPretrainingRunShapeObservation {
        schema_version: String::from(
            psionic_train::PSION_ACTUAL_PRETRAINING_RUN_SHAPE_OBSERVATION_SCHEMA_VERSION,
        ),
        observation_id: format!("psion_actual_pretraining_local_run_shape_probe::{now}"),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        observation_kind: String::from("local_runtime_probe"),
        observed_at_utc: now,
        observed_run_root: run_root.display().to_string(),
        throughput_probe: PsionActualPretrainingThroughputProbe {
            source_receipt_id: String::from("local_runtime_probe_unmeasured"),
            source_receipt_digest: String::from("local_runtime_probe_unmeasured"),
            observed_tokens_per_second: 0,
            observed_step_latency_ms: u64::MAX,
            observed_checkpoint_write_throughput_bytes_per_second: storage_probe
                .observed_write_bytes_per_second,
            detail: format!(
                "Local probe does not claim admitted training throughput. It retains only an unmeasured placeholder against the frozen trusted-cluster anchor `{}` so non-dry-run launch can refuse honestly without a retained benchmark receipt.",
                throughput_source.baseline_id
            ),
        },
        storage_probe,
        dataloader_probe: PsionActualPretrainingDataloaderProbe {
            dataset_identity: data_bundle.replay_authority.dataset_identity.clone(),
            max_sequence_tokens: data_bundle.replay_authority.max_sequence_tokens,
            planned_optimizer_steps: actual_lane_accounting.optimizer_steps,
            planned_tokens_per_step: actual_lane_accounting.tokens_per_step,
            observed_horizon_steps: 0,
            observed_horizon_tokens: 0,
            observed_batches_per_second: 0,
            observed_stall_count: 0,
            deterministic_replay_observed: false,
            detail: String::from(
                "Local probe retains the frozen dataloader plan but does not claim admitted replay or horizon coverage without a retained actual-lane measurement bundle.",
            ),
        },
        summary: String::from(
            "Local run-shape probe retains storage writeability plus the frozen dataloader plan, but refuses long-run admission until retained actual-lane throughput and dataloader evidence is supplied.",
        ),
        observation_digest: String::new(),
    };
    observation.observation_digest =
        psionic_train::stable_run_shape_observation_digest(&observation)?;
    observation.validate()?;
    Ok(observation)
}

fn probe_local_storage_probe(
    run_root: &Path,
) -> Result<PsionActualPretrainingStorageProbe, Box<dyn Error>> {
    fs::create_dir_all(run_root)?;
    let available_bytes = filesystem_available_bytes(run_root).unwrap_or(0);
    let temp_path = run_root.join(".psion_run_shape_probe");
    let payload = vec![0u8; 4 * 1024 * 1024];
    let write_started = Instant::now();
    fs::write(&temp_path, &payload)?;
    let write_elapsed = write_started.elapsed().as_nanos();
    let read_started = Instant::now();
    let bytes = fs::read(&temp_path)?;
    let read_elapsed = read_started.elapsed().as_nanos();
    let _ = fs::remove_file(&temp_path);
    Ok(PsionActualPretrainingStorageProbe {
        storage_path: run_root.display().to_string(),
        available_bytes,
        observed_read_bytes_per_second: bytes_per_second(bytes.len() as u64, read_elapsed),
        observed_write_bytes_per_second: bytes_per_second(payload.len() as u64, write_elapsed),
        writable: true,
        detail: String::from(
            "Local storage probe retains one bounded read/write measurement against the selected run root before the actual lane is admitted.",
        ),
    })
}

fn probe_local_workers(
    repo_root: &Path,
) -> Result<Vec<PsionActualPretrainingObservedWorker>, Box<dyn Error>> {
    let host_label = hostname_short(repo_root)?;
    let gpu_rows = run_optional_command(
        repo_root,
        "nvidia-smi",
        &[
            "--query-gpu=index,name,memory.total,memory.free,temperature.gpu,ecc.errors.uncorrected.aggregate.total,uuid,mig.mode.current",
            "--format=csv,noheader,nounits",
        ],
    )?;
    let Some(gpu_rows) = gpu_rows else {
        return Ok(Vec::new());
    };
    let process_rows = run_optional_command(
        repo_root,
        "nvidia-smi",
        &[
            "--query-compute-apps=gpu_uuid,pid",
            "--format=csv,noheader,nounits",
        ],
    )?;
    let mut resident_counts = std::collections::BTreeMap::<String, u64>::new();
    if let Some(process_rows) = process_rows {
        for line in process_rows.lines().filter(|line| !line.trim().is_empty()) {
            let parts = split_csv_fields(line);
            if let Some(uuid) = parts.first() {
                *resident_counts.entry((*uuid).to_string()).or_default() += 1;
            }
        }
    }
    let mut workers = Vec::new();
    for line in gpu_rows.lines().filter(|line| !line.trim().is_empty()) {
        let parts = split_csv_fields(line);
        if parts.len() < 8 {
            continue;
        }
        let gpu_index = parts[0];
        let uuid = parts[6];
        let total_memory_bytes = parse_nvidia_memory_mebibytes(parts[2]).unwrap_or(0);
        let free_memory_bytes = parse_nvidia_memory_mebibytes(parts[3]).unwrap_or(0);
        let temperature_celsius = parts[4].parse::<u64>().ok();
        let ecc_uncorrected_error_count = parse_optional_nvidia_u64(parts[5]);
        let mig_partitioned = parts[7].to_ascii_lowercase().contains("enabled");
        workers.push(PsionActualPretrainingObservedWorker {
            worker_label: format!("{host_label}-gpu{gpu_index}"),
            backend: String::from("cuda"),
            device_name: String::from(parts[1]),
            total_memory_bytes,
            free_memory_bytes,
            temperature_celsius,
            ecc_uncorrected_error_count,
            throttling_observed: Some(false),
            resident_compute_process_count: Some(*resident_counts.get(uuid).unwrap_or(&0)),
            mig_partitioned,
            detail: String::from(
                "Local worker snapshot retained for actual-lane hardware qualification.",
            ),
        });
    }
    Ok(workers)
}

fn observe_credential_source(
    source: &psionic_train::PsionActualPretrainingCredentialSource,
) -> Result<PsionActualPretrainingObservedCredentialSource, Box<dyn Error>> {
    let raw_value = env::var(source.source_name.as_str()).ok();
    let redacted_digest = match source.kind.as_str() {
        "secret_file_env" => raw_value
            .as_deref()
            .and_then(|path| fs::read(path).ok())
            .map(|bytes| sha256_hex(&bytes)),
        _ => raw_value
            .as_deref()
            .map(|value| sha256_hex(value.as_bytes())),
    };
    Ok(PsionActualPretrainingObservedCredentialSource {
        source_name: source.source_name.clone(),
        kind: source.kind.clone(),
        present: raw_value.is_some(),
        redacted_digest,
        detail: String::from(
            "Credential source is retained by declared name and redacted digest only.",
        ),
    })
}

fn preflight_ref_from_qualification(
    qualification: &PsionActualPretrainingHardwareQualification,
    retained_paths: &PsionActualPretrainingRetainedPathSet,
) -> PsionActualPretrainingPreflightRef {
    PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.hardware_qualification_path.clone(),
        receipt_digest: qualification.receipt_digest.clone(),
        admission_state: qualification.admission_state.clone(),
    }
}

fn run_shape_ref_from_qualification(
    qualification: &PsionActualPretrainingRunShapeQualification,
    retained_paths: &PsionActualPretrainingRetainedPathSet,
) -> PsionActualPretrainingPreflightRef {
    PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.run_shape_qualification_path.clone(),
        receipt_digest: qualification.receipt_digest.clone(),
        admission_state: qualification.admission_state.clone(),
    }
}

fn write_preflight_receipts(
    run_root: &Path,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    launcher_log_line: &str,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(run_root.join("preflight"))?;
    fs::create_dir_all(run_root.join("logs"))?;
    fs::write(
        run_root.join("preflight/hardware_qualification.json"),
        serde_json::to_string_pretty(hardware_qualification)?,
    )?;
    fs::write(
        run_root.join("preflight/run_shape_qualification.json"),
        serde_json::to_string_pretty(run_shape_qualification)?,
    )?;
    append_launcher_log(run_root, launcher_log_line)?;
    Ok(())
}

fn run_optional_command(
    repo_root: &Path,
    program: &str,
    args: &[&str],
) -> Result<Option<String>, Box<dyn Error>> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output();
    match output {
        Ok(output) if output.status.success() => {
            Ok(Some(String::from_utf8(output.stdout)?.trim().to_string()))
        }
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Box::new(error)),
    }
}

fn hostname_short(repo_root: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("hostname")
        .arg("-s")
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        return Ok(String::from_utf8(output.stdout)?.trim().to_string());
    }
    let fallback = Command::new("hostname").current_dir(repo_root).output()?;
    if !fallback.status.success() {
        return Err(std::io::Error::other("failed to resolve hostname").into());
    }
    Ok(String::from_utf8(fallback.stdout)?.trim().to_string())
}

fn split_csv_fields(line: &str) -> Vec<&str> {
    line.split(',').map(|field| field.trim()).collect()
}

fn parse_nvidia_memory_mebibytes(value: &str) -> Option<u64> {
    value
        .parse::<u64>()
        .ok()
        .map(|mib| mib.saturating_mul(1024 * 1024))
}

fn parse_optional_nvidia_u64(value: &str) -> Option<u64> {
    let normalized = value.trim();
    if normalized.is_empty()
        || normalized.eq_ignore_ascii_case("[Not Supported]")
        || normalized.eq_ignore_ascii_case("N/A")
    {
        return None;
    }
    normalized.parse::<u64>().ok()
}

fn filesystem_available_bytes(path: &Path) -> Result<u64, Box<dyn Error>> {
    let output = Command::new("df").arg("-Pk").arg(path).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("failed to read filesystem capacity").into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let line = stdout
        .lines()
        .nth(1)
        .ok_or_else(|| std::io::Error::other("df output missing filesystem row"))?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let available_kib = fields
        .get(3)
        .ok_or_else(|| std::io::Error::other("df output missing available column"))?
        .parse::<u64>()?;
    Ok(available_kib.saturating_mul(1024))
}

fn bytes_per_second(bytes: u64, elapsed_nanos: u128) -> u64 {
    if elapsed_nanos == 0 {
        return bytes;
    }
    let bytes = bytes as u128;
    let per_second = bytes.saturating_mul(1_000_000_000u128) / elapsed_nanos;
    per_second.min(u64::MAX as u128) as u64
}

fn load_json<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn load_optional_json<T>(path: &Path) -> Result<Option<T>, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(load_json(path)?))
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: file_sha256(path)?,
    })
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

fn append_launcher_log(run_root: &Path, line: &str) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(run_root.join("logs"))?;
    let path = run_root.join("logs/launcher.log");
    let mut existing = if path.is_file() {
        fs::read_to_string(&path)?
    } else {
        String::new()
    };
    existing.push_str(line);
    fs::write(path, existing)?;
    Ok(())
}

fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn run_actual_pretraining_distributed_bringup_rehearsal(
    repo_root: &Path,
    run_root: &Path,
    actual_run_id: &str,
    selected_git_ref: &str,
    remote_host: &str,
    secondary_remote_host: Option<&str>,
    planned_interruption_step: Option<u64>,
    cleanup_remote: bool,
) -> Result<PsionActualPretrainingDistributedBringupRehearsal, Box<dyn Error>> {
    let total_target_steps = configured_actual_bringup_max_steps()?;
    if let Some(interruption_step) = planned_interruption_step {
        return run_actual_pretraining_distributed_bringup_recovery_drill(
            repo_root,
            run_root,
            actual_run_id,
            selected_git_ref,
            remote_host,
            secondary_remote_host,
            interruption_step,
            total_target_steps,
            cleanup_remote,
        );
    }
    let bringup_run_id = format!("{actual_run_id}-distributed-actual-pretraining-bringup");
    let relative_output_root = PathBuf::from("distributed_actual_pretraining_bringup");
    let output_root = run_root.join(&relative_output_root);
    let resolved_git_ref = git_output(repo_root, &["rev-parse", selected_git_ref])?;
    if output_root.exists() {
        fs::remove_dir_all(&output_root)?;
    }
    let mut command = Command::new(repo_root.join("scripts/train-psion-local-first.sh"));
    command
        .arg("--workload")
        .arg("actual_pretraining_bringup")
        .arg("--mode")
        .arg("distributed_reference")
        .arg("--remote-host")
        .arg(remote_host)
        .arg("--run-id")
        .arg(&bringup_run_id)
        .arg("--output-root")
        .arg(&output_root)
        .arg("--git-ref")
        .arg(&resolved_git_ref);
    if let Some(secondary_remote_host) = secondary_remote_host {
        command
            .arg("--secondary-remote-host")
            .arg(secondary_remote_host);
    }
    if cleanup_remote {
        command.arg("--cleanup-remote");
    }
    let output = command.output()?;
    if !output.status.success() {
        let local_log_tail = output_root.join("actual_pretraining_bringup_train.log");
        let log_excerpt = if local_log_tail.is_file() {
            let log = fs::read_to_string(&local_log_tail)?;
            let lines: Vec<&str> = log.lines().rev().take(80).collect();
            format!(
                "\nlocal_train_log_tail:\n{}",
                lines.into_iter().rev().collect::<Vec<_>>().join("\n")
            )
        } else {
            String::new()
        };
        return Err(std::io::Error::other(format!(
            "actual-lane distributed bringup rehearsal failed\nstdout:\n{}\nstderr:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            log_excerpt,
        ))
        .into());
    }

    let operator_manifest_path =
        output_root.join("actual_pretraining_bringup_operator_manifest.json");
    let operator_summary_path =
        output_root.join("actual_pretraining_bringup_operator_summary.json");
    let train_log_path = output_root.join("actual_pretraining_bringup_train.log");
    let artifact_dir = output_root.join("actual_pretraining_bringup_artifacts");
    let cluster_topology_receipt_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_topology_receipt.json")
        .is_file()
    {
        artifact_dir.join("psion_actual_pretraining_bringup_cluster_topology_receipt.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_dual_host_topology_receipt.json")
    };
    let cluster_contribution_receipts_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json")
        .is_file()
    {
        artifact_dir.join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_dual_host_contribution_receipts.json")
    };
    let contributor_continuity_receipt_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json")
        .is_file()
    {
        artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_contributor_continuity_receipt.json")
    };
    let progress_checkpoint_receipts_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json")
        .is_file()
    {
        artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoint_receipts.json")
    };
    let progress_checkpoint_directory =
        artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoints");
    let checkpoint_manifest_path =
        artifact_dir.join("psion_actual_pretraining_bringup_checkpoint_manifest.json");
    let checkpoint_weights_path =
        artifact_dir.join("psion_actual_pretraining_bringup_checkpoint.safetensors");

    let topology_receipt: PsionReferencePilotDualHostTopologyReceipt =
        load_json(&cluster_topology_receipt_path)?;
    let contribution_receipts: Vec<PsionReferencePilotJointContributionSummaryReceipt> =
        load_json(&cluster_contribution_receipts_path)?;
    let contributor_continuity_receipt: PsionReferencePilotContributorContinuityReceipt =
        load_json(&contributor_continuity_receipt_path)?;
    let progress_checkpoint_receipts: Vec<PsionReferencePilotProgressCheckpointReceipt> =
        load_json(&progress_checkpoint_receipts_path)?;
    let checkpoint_manifest: PsionReferencePilotCheckpointManifest =
        load_json(&checkpoint_manifest_path)?;
    let checkpoint_object_digest = file_sha256(&checkpoint_weights_path)?;
    let checkpoint_total_bytes = fs::metadata(&checkpoint_weights_path)?.len();

    if topology_receipt.contributor_count == 0 {
        return Err(std::io::Error::other(
            "distributed bringup rehearsal retained zero contributors",
        )
        .into());
    }
    if let Some(_) = secondary_remote_host {
        if topology_receipt.contributor_count < 3 {
            return Err(std::io::Error::other(
                "distributed bringup rehearsal did not retain the expected tri-host contributor count",
            )
            .into());
        }
    }
    if contribution_receipts.len()
        < topology_receipt
            .contributor_count
            .saturating_mul(checkpoint_manifest.step as usize)
    {
        return Err(std::io::Error::other(
            "distributed bringup rehearsal retained fewer contribution receipts than expected from the checkpoint step count",
        )
        .into());
    }
    if !contributor_continuity_receipt.all_configured_contributors_present_each_step {
        return Err(std::io::Error::other(
            "distributed bringup rehearsal retained contributor continuity gaps",
        )
        .into());
    }
    if progress_checkpoint_receipts.is_empty() {
        return Err(std::io::Error::other(
            "distributed bringup rehearsal retained no progress checkpoints",
        )
        .into());
    }
    if !progress_checkpoint_directory.is_dir() {
        return Err(std::io::Error::other(
            "distributed bringup rehearsal progress checkpoint directory is missing",
        )
        .into());
    }
    let progress_window_count = progress_checkpoint_receipts
        .iter()
        .map(|receipt| receipt.window_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let progress_cadence_count = progress_checkpoint_receipts
        .iter()
        .filter(|receipt| receipt.is_cadence_boundary)
        .map(|receipt| receipt.cadence_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let final_progress_receipt = progress_checkpoint_receipts.last().ok_or_else(|| {
        std::io::Error::other("distributed bringup rehearsal retained no final progress receipt")
    })?;

        let receipt = PsionActualPretrainingDistributedBringupRehearsal {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_DISTRIBUTED_BRINGUP_REHEARSAL_SCHEMA_VERSION,
        ),
        run_id: String::from(actual_run_id),
        bringup_run_id,
        rehearsal_summary_path: String::from(
            "distributed_execution/distributed_actual_pretraining_bringup.json",
        ),
        relative_output_root: relative_output_root.to_string_lossy().replace('\\', "/"),
        operator_manifest_path: operator_manifest_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        operator_summary_path: operator_summary_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        train_log_path: train_log_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        cluster_topology_receipt_path: cluster_topology_receipt_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        cluster_contribution_receipts_path: cluster_contribution_receipts_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        contributor_continuity_receipt_path: contributor_continuity_receipt_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        progress_checkpoint_receipts_path: progress_checkpoint_receipts_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
            progress_checkpoint_directory: progress_checkpoint_directory
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
            checkpoint_manifest_path: checkpoint_manifest_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
            checkpoint_weights_path: checkpoint_weights_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
            recovery_drill_receipt_path: None,
            execution_topology_classification: topology_receipt.execution_topology_classification,
        contributor_count: topology_receipt.contributor_count,
        worker_hosts: topology_receipt.worker_hosts,
        runtime_backends: topology_receipt.runtime_backends,
        checkpoint_ref: checkpoint_manifest.checkpoint_ref,
        checkpoint_object_digest,
        checkpoint_total_bytes,
        optimizer_steps: checkpoint_manifest.step,
        contribution_receipt_count: contribution_receipts.len(),
        progress_checkpoint_count: progress_checkpoint_receipts.len(),
        progress_window_count,
        progress_cadence_count,
        final_cumulative_train_tokens_processed: final_progress_receipt
            .cumulative_train_tokens_processed,
        final_cumulative_mean_tokens_per_second: final_progress_receipt
            .cumulative_mean_tokens_per_second,
        claim_boundary: String::from(
            "This retained rehearsal proves one bounded multi-host optimizer-bearing execution segment inside the actual pretraining operator path using the larger actual-pretraining model and canonical actual dataset identity. It does not by itself claim full broader long-run actual-lane cluster execution.",
        ),
        detail: String::from(
            "Distributed bringup binds the actual-lane run root to one retained multi-host actual-pretraining bringup proof, its progress checkpoints, continuity receipt, checkpoint bytes, and the exact operator/log surfaces used to produce that proof.",
        ),
    };
    receipt.validate()?;
    write_json_pretty(&run_root.join(&receipt.rehearsal_summary_path), &receipt)?;
    Ok(receipt)
}

fn configured_actual_bringup_max_steps() -> Result<u64, Box<dyn Error>> {
    match env::var("PSION_REFERENCE_PILOT_MAX_STEPS") {
        Ok(value) => Ok(value.parse::<u64>()?),
        Err(env::VarError::NotPresent) => Ok(
            psionic_train::PsionReferencePilotConfig::actual_pretraining_bringup()?
                .budget
                .max_steps,
        ),
        Err(error) => Err(Box::new(error)),
    }
}

fn configured_actual_bringup_step_duration_ms() -> Result<u64, Box<dyn Error>> {
    match env::var("PSION_REFERENCE_PILOT_STEP_DURATION_MS") {
        Ok(value) => Ok(value.parse::<u64>()?),
        Err(env::VarError::NotPresent) => Ok(
            psionic_train::PsionReferencePilotConfig::actual_pretraining_bringup()?
                .step_duration_ms,
        ),
        Err(error) => Err(Box::new(error)),
    }
}

fn run_distributed_bringup_segment(
    repo_root: &Path,
    output_root: &Path,
    actual_run_id: &str,
    selected_git_ref: &str,
    remote_host: &str,
    secondary_remote_host: Option<&str>,
    cleanup_remote: bool,
    max_steps_override: Option<u64>,
    total_max_steps: Option<u64>,
    resume_artifact_dir: Option<&Path>,
) -> Result<DistributedBringupSegmentArtifacts, Box<dyn Error>> {
    let segment_run_id = format!(
        "{}-{}",
        actual_run_id,
        output_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("segment")
    );
    let resolved_git_ref = git_output(repo_root, &["rev-parse", selected_git_ref])?;
    if output_root.exists() {
        fs::remove_dir_all(output_root)?;
    }
    let mut command = Command::new(repo_root.join("scripts/train-psion-local-first.sh"));
    command
        .arg("--workload")
        .arg("actual_pretraining_bringup")
        .arg("--mode")
        .arg("distributed_reference")
        .arg("--remote-host")
        .arg(remote_host)
        .arg("--run-id")
        .arg(&segment_run_id)
        .arg("--output-root")
        .arg(output_root)
        .arg("--git-ref")
        .arg(&resolved_git_ref);
    if let Some(secondary_remote_host) = secondary_remote_host {
        command
            .arg("--secondary-remote-host")
            .arg(secondary_remote_host);
    }
    if cleanup_remote {
        command.arg("--cleanup-remote");
    }
    if let Some(max_steps_override) = max_steps_override {
        command.env(
            "PSION_REFERENCE_PILOT_MAX_STEPS",
            max_steps_override.to_string(),
        );
    }
    if let Some(total_max_steps) = total_max_steps {
        command.env(
            "PSION_REFERENCE_PILOT_TOTAL_MAX_STEPS",
            total_max_steps.to_string(),
        );
    }
    if let Some(resume_artifact_dir) = resume_artifact_dir {
        command.env(
            "PSION_REFERENCE_PILOT_RESUME_FROM_ARTIFACT_DIR",
            resume_artifact_dir,
        );
    }
    let output = command.output()?;
    if !output.status.success() {
        let local_log_tail = output_root.join("actual_pretraining_bringup_train.log");
        let log_excerpt = if local_log_tail.is_file() {
            let log = fs::read_to_string(&local_log_tail)?;
            let lines: Vec<&str> = log.lines().rev().take(80).collect();
            format!(
                "\nlocal_train_log_tail:\n{}",
                lines.into_iter().rev().collect::<Vec<_>>().join("\n")
            )
        } else {
            String::new()
        };
        return Err(std::io::Error::other(format!(
            "actual-lane distributed bringup segment failed\nstdout:\n{}\nstderr:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            log_excerpt,
        ))
        .into());
    }
    load_distributed_bringup_segment_artifacts(output_root)
}

fn load_distributed_bringup_segment_artifacts(
    output_root: &Path,
) -> Result<DistributedBringupSegmentArtifacts, Box<dyn Error>> {
    let operator_manifest_path =
        output_root.join("actual_pretraining_bringup_operator_manifest.json");
    let operator_summary_path =
        output_root.join("actual_pretraining_bringup_operator_summary.json");
    let train_log_path = output_root.join("actual_pretraining_bringup_train.log");
    let artifact_dir = output_root.join("actual_pretraining_bringup_artifacts");
    let cluster_topology_receipt_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_topology_receipt.json")
        .is_file()
    {
        artifact_dir.join("psion_actual_pretraining_bringup_cluster_topology_receipt.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_dual_host_topology_receipt.json")
    };
    let cluster_step_receipts_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_step_receipts.json")
        .is_file()
    {
        artifact_dir.join("psion_actual_pretraining_bringup_cluster_step_receipts.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_dual_host_step_receipts.json")
    };
    let cluster_contribution_receipts_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json")
        .is_file()
    {
        artifact_dir.join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_dual_host_contribution_receipts.json")
    };
    let contributor_continuity_receipt_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json")
        .is_file()
    {
        artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_contributor_continuity_receipt.json")
    };
    let progress_checkpoint_receipts_path = if artifact_dir
        .join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json")
        .is_file()
    {
        artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json")
    } else {
        artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoint_receipts.json")
    };
    let progress_checkpoint_directory =
        artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoints");
    let checkpoint_manifest_path =
        artifact_dir.join("psion_actual_pretraining_bringup_checkpoint_manifest.json");
    let checkpoint_weights_path =
        artifact_dir.join("psion_actual_pretraining_bringup_checkpoint.safetensors");
    let checkpoint_optimizer_state_path =
        artifact_dir.join("psion_actual_pretraining_bringup_optimizer_state.json");
    let checkpoint_stage_receipt_path =
        artifact_dir.join("psion_actual_pretraining_bringup_stage_receipt.json");

    let topology_receipt: PsionReferencePilotDualHostTopologyReceipt =
        load_json(&cluster_topology_receipt_path)?;
    let joint_step_receipts: Vec<PsionReferencePilotDualHostStepReceipt> =
        load_json(&cluster_step_receipts_path)?;
    let contribution_receipts: Vec<PsionReferencePilotJointContributionSummaryReceipt> =
        load_json(&cluster_contribution_receipts_path)?;
    let contributor_continuity_receipt: PsionReferencePilotContributorContinuityReceipt =
        load_json(&contributor_continuity_receipt_path)?;
    let progress_checkpoint_receipts: Vec<PsionReferencePilotProgressCheckpointReceipt> =
        load_json(&progress_checkpoint_receipts_path)?;
    let checkpoint_manifest: PsionReferencePilotCheckpointManifest =
        load_json(&checkpoint_manifest_path)?;
    let checkpoint_stage_receipt: PsionPretrainStageRunReceipt =
        load_json(&checkpoint_stage_receipt_path)?;
    let checkpoint_object_digest = file_sha256(&checkpoint_weights_path)?;
    let checkpoint_total_bytes = fs::metadata(&checkpoint_weights_path)?.len();

    Ok(DistributedBringupSegmentArtifacts {
        output_root: output_root.to_path_buf(),
        operator_manifest_path,
        operator_summary_path,
        train_log_path,
        artifact_dir,
        cluster_topology_receipt_path,
        cluster_step_receipts_path,
        cluster_contribution_receipts_path,
        contributor_continuity_receipt_path,
        progress_checkpoint_receipts_path,
        progress_checkpoint_directory,
        checkpoint_manifest_path,
        checkpoint_weights_path,
        checkpoint_optimizer_state_path,
        checkpoint_stage_receipt_path,
        topology_receipt,
        joint_step_receipts,
        contribution_receipts,
        contributor_continuity_receipt,
        progress_checkpoint_receipts,
        checkpoint_manifest,
        checkpoint_stage_receipt,
        checkpoint_object_digest,
        checkpoint_total_bytes,
    })
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn build_combined_contributor_continuity_receipt(
    run_id: &str,
    stage_id: &str,
    checkpoint_family: &str,
    expected_contributor_hosts: &[String],
    contribution_receipts: &[PsionReferencePilotJointContributionSummaryReceipt],
    runtime_backends: &[String],
) -> PsionReferencePilotContributorContinuityReceipt {
    let mut observed_contribution_counts_by_host = std::collections::BTreeMap::new();
    let mut runtime_backends_by_host = std::collections::BTreeMap::new();
    for (host, backend) in expected_contributor_hosts.iter().zip(runtime_backends.iter()) {
        runtime_backends_by_host.insert(host.clone(), backend.clone());
    }
    for receipt in contribution_receipts {
        *observed_contribution_counts_by_host
            .entry(receipt.contributor_host.clone())
            .or_insert(0) += 1;
    }
    let mut missing_contributors_by_step = Vec::new();
    let max_step = contribution_receipts
        .iter()
        .map(|receipt| receipt.global_step)
        .max()
        .unwrap_or(0);
    for global_step in 1..=max_step {
        let observed = contribution_receipts
            .iter()
            .filter(|receipt| receipt.global_step == global_step)
            .map(|receipt| receipt.contributor_host.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let missing_hosts = expected_contributor_hosts
            .iter()
            .filter(|host| !observed.contains(*host))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_hosts.is_empty() {
            missing_contributors_by_step.push(psionic_train::PsionReferencePilotMissingContributorStep {
                global_step,
                missing_hosts,
            });
        }
    }
    let all_configured_contributors_present_each_step = missing_contributors_by_step.is_empty();
    let mut receipt = PsionReferencePilotContributorContinuityReceipt {
        schema_version: String::from("psion.reference_pilot_contributor_continuity_receipt.v1"),
        run_id: String::from(run_id),
        stage_id: String::from(stage_id),
        checkpoint_family: String::from(checkpoint_family),
        optimizer_steps_completed: max_step,
        expected_contributor_hosts: expected_contributor_hosts.to_vec(),
        observed_contribution_counts_by_host,
        runtime_backends_by_host,
        missing_contributors_by_step,
        all_configured_contributors_present_each_step,
        detail: format!(
            "Combined contributor continuity receipt proves the recovered actual-pretraining bringup retained {} contributors across {} optimizer steps.",
            expected_contributor_hosts.len(),
            max_step
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn write_refused_recovery_drill_receipt(
    run_root: &Path,
    output_root: &Path,
    actual_run_id: &str,
    interruption_step: u64,
    total_target_steps: u64,
    interrupted_segment: &DistributedBringupSegmentArtifacts,
    refusal_reason: impl Into<String>,
) -> Result<PathBuf, Box<dyn Error>> {
    let receipt = PsionActualPretrainingDistributedRecoveryDrillReceipt {
        schema_version: String::from("psion.actual_pretraining_distributed_recovery_drill.v1"),
        run_id: String::from(actual_run_id),
        recovery_state: String::from("refused"),
        interruption_kind: String::from("planned_checkpoint_boundary_interrupt"),
        interruption_step,
        total_target_optimizer_steps: total_target_steps,
        interrupted_segment_summary_path: interrupted_segment
            .operator_summary_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_segment_artifact_dir: interrupted_segment
            .artifact_dir
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_checkpoint_ref: interrupted_segment.checkpoint_manifest.checkpoint_ref.clone(),
        interrupted_checkpoint_manifest_path: interrupted_segment
            .checkpoint_manifest_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_checkpoint_object_digest: interrupted_segment.checkpoint_object_digest.clone(),
        recovered_segment_summary_path: None,
        recovered_segment_artifact_dir: None,
        recovered_checkpoint_ref: None,
        recovered_checkpoint_manifest_path: None,
        recovered_checkpoint_object_digest: None,
        lineage_preserved: false,
        refusal_reason: Some(refusal_reason.into()),
        detail: String::from(
            "Recovery drill retained the interrupted checkpoint lineage but refused continuation because the resumed segment could not honestly stay on the same accepted checkpoint family.",
        ),
    };
    receipt.validate()?;
    let recovery_receipt_path = output_root.join("actual_pretraining_bringup_recovery_drill.json");
    write_json_pretty(&recovery_receipt_path, &receipt)?;
    Ok(recovery_receipt_path)
}

fn run_actual_pretraining_distributed_bringup_recovery_drill(
    repo_root: &Path,
    run_root: &Path,
    actual_run_id: &str,
    selected_git_ref: &str,
    remote_host: &str,
    secondary_remote_host: Option<&str>,
    interruption_step: u64,
    total_target_steps: u64,
    cleanup_remote: bool,
) -> Result<PsionActualPretrainingDistributedBringupRehearsal, Box<dyn Error>> {
    if interruption_step == 0 || interruption_step >= total_target_steps {
        return Err(std::io::Error::other(
            "planned interruption step must be greater than zero and less than the total target steps",
        )
        .into());
    }
    let bringup_run_id = format!("{actual_run_id}-distributed-actual-pretraining-bringup");
    let relative_output_root = PathBuf::from("distributed_actual_pretraining_bringup");
    let output_root = run_root.join(&relative_output_root);
    if output_root.exists() {
        fs::remove_dir_all(&output_root)?;
    }
    fs::create_dir_all(&output_root)?;

    let interrupted_segment_root = output_root.join("interrupted_segment");
    let interrupted_segment = run_distributed_bringup_segment(
        repo_root,
        &interrupted_segment_root,
        actual_run_id,
        selected_git_ref,
        remote_host,
        secondary_remote_host,
        cleanup_remote,
        Some(interruption_step),
        Some(total_target_steps),
        None,
    )?;
    if interrupted_segment.checkpoint_manifest.step != interruption_step {
        return Err(std::io::Error::other(
            "interrupted segment checkpoint step drifted from the requested interruption boundary",
        )
        .into());
    }

    let recovered_segment_root = output_root.join("recovered_segment");
    let recovered_segment = match run_distributed_bringup_segment(
        repo_root,
        &recovered_segment_root,
        actual_run_id,
        selected_git_ref,
        remote_host,
        secondary_remote_host,
        cleanup_remote,
        Some(total_target_steps.saturating_sub(interruption_step)),
        Some(total_target_steps),
        Some(&interrupted_segment.artifact_dir),
    ) {
        Ok(segment) => segment,
        Err(error) => {
            let receipt_path = write_refused_recovery_drill_receipt(
                run_root,
                &output_root,
                actual_run_id,
                interruption_step,
                total_target_steps,
                &interrupted_segment,
                error.to_string(),
            )?;
            return Err(std::io::Error::other(format!(
                "distributed recovery drill refused continuation; see {}: {}",
                receipt_path.display(),
                error
            ))
            .into());
        }
    };

    let interrupted_checkpoint = &interrupted_segment
        .checkpoint_stage_receipt
        .checkpoint_lineage
        .promoted_checkpoint;
    let recovered_base_checkpoint = recovered_segment
        .checkpoint_stage_receipt
        .checkpoint_lineage
        .base_checkpoint
        .as_ref()
        .ok_or_else(|| {
            std::io::Error::other(
                "recovered segment is missing the expected base checkpoint lineage",
            )
        })?;
    if recovered_base_checkpoint.object_digest != interrupted_checkpoint.object_digest
        || recovered_base_checkpoint.checkpoint_ref != interrupted_checkpoint.checkpoint_ref
    {
        let receipt_path = write_refused_recovery_drill_receipt(
            run_root,
            &output_root,
            actual_run_id,
            interruption_step,
            total_target_steps,
            &interrupted_segment,
            "recovered segment base checkpoint drifted from the interrupted checkpoint lineage",
        )?;
        return Err(std::io::Error::other(format!(
            "distributed recovery drill refused continuation because lineage drifted; see {}",
            receipt_path.display()
        ))
        .into());
    }

    let combined_artifact_dir = output_root.join("actual_pretraining_bringup_artifacts");
    if combined_artifact_dir.exists() {
        fs::remove_dir_all(&combined_artifact_dir)?;
    }
    fs::create_dir_all(&combined_artifact_dir)?;

    for filename in [
        "psion_actual_pretraining_bringup_stage_config.json",
        "psion_actual_pretraining_bringup_stage_receipt.json",
        "psion_actual_pretraining_bringup_observability_receipt.json",
        "psion_actual_pretraining_bringup_checkpoint_manifest.json",
        "psion_actual_pretraining_bringup_optimizer_state.json",
        "psion_actual_pretraining_bringup_summary.json",
        "psion_actual_pretraining_bringup_checkpoint.safetensors",
    ] {
        fs::copy(
            recovered_segment.artifact_dir.join(filename),
            combined_artifact_dir.join(filename),
        )?;
    }

    let mut combined_step_receipts = interrupted_segment.joint_step_receipts.clone();
    combined_step_receipts.extend(recovered_segment.joint_step_receipts.clone());
    combined_step_receipts.sort_by_key(|receipt| receipt.global_step);

    let mut combined_contribution_receipts = interrupted_segment.contribution_receipts.clone();
    combined_contribution_receipts.extend(recovered_segment.contribution_receipts.clone());
    combined_contribution_receipts.sort_by_key(|receipt| {
        (receipt.global_step, receipt.contributor_index as u64)
    });

    let step_duration_ms = configured_actual_bringup_step_duration_ms()?;
    let mut combined_progress_checkpoint_receipts =
        interrupted_segment.progress_checkpoint_receipts.clone();
    let token_offset = interrupted_segment
        .progress_checkpoint_receipts
        .last()
        .map(|receipt| receipt.cumulative_train_tokens_processed)
        .unwrap_or(0);
    for mut receipt in recovered_segment.progress_checkpoint_receipts.clone() {
        receipt.cumulative_train_tokens_processed = receipt
            .cumulative_train_tokens_processed
            .saturating_add(token_offset);
        receipt.cumulative_mean_tokens_per_second =
            (receipt.cumulative_train_tokens_processed * 1000)
                / receipt
                    .optimizer_steps_completed
                    .saturating_mul(step_duration_ms)
                    .max(1);
        combined_progress_checkpoint_receipts.push(receipt);
    }
    combined_progress_checkpoint_receipts
        .sort_by_key(|receipt| receipt.optimizer_steps_completed);

    let combined_contributor_continuity_receipt = build_combined_contributor_continuity_receipt(
        actual_run_id,
        &recovered_segment.checkpoint_stage_receipt.stage_id,
        &recovered_segment.checkpoint_manifest.checkpoint_family,
        recovered_segment.topology_receipt.worker_hosts.as_slice(),
        combined_contribution_receipts.as_slice(),
        recovered_segment.topology_receipt.runtime_backends.as_slice(),
    );
    if !combined_contributor_continuity_receipt.all_configured_contributors_present_each_step {
        let receipt_path = write_refused_recovery_drill_receipt(
            run_root,
            &output_root,
            actual_run_id,
            interruption_step,
            total_target_steps,
            &interrupted_segment,
            "combined contributor continuity drifted during the recovered segment",
        )?;
        return Err(std::io::Error::other(format!(
            "distributed recovery drill refused continuation because contributor continuity drifted; see {}",
            receipt_path.display()
        ))
        .into());
    }

    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_cluster_topology_receipt.json"),
        &recovered_segment.topology_receipt,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_dual_host_topology_receipt.json"),
        &recovered_segment.topology_receipt,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_cluster_step_receipts.json"),
        &combined_step_receipts,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_dual_host_step_receipts.json"),
        &combined_step_receipts,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json"),
        &combined_contribution_receipts,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_dual_host_contribution_receipts.json"),
        &combined_contribution_receipts,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json"),
        &combined_contributor_continuity_receipt,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_contributor_continuity_receipt.json"),
        &combined_contributor_continuity_receipt,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json"),
        &combined_progress_checkpoint_receipts,
    )?;
    write_json_pretty(
        &combined_artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoint_receipts.json"),
        &combined_progress_checkpoint_receipts,
    )?;

    let combined_progress_dir =
        combined_artifact_dir.join("psion_actual_pretraining_bringup_progress_checkpoints");
    fs::create_dir_all(&combined_progress_dir)?;
    for source_dir in [
        &interrupted_segment.progress_checkpoint_directory,
        &recovered_segment.progress_checkpoint_directory,
    ] {
        for entry in fs::read_dir(source_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                copy_dir_recursive(&entry.path(), &combined_progress_dir.join(entry.file_name()))?;
            }
        }
    }

    let recovery_receipt = PsionActualPretrainingDistributedRecoveryDrillReceipt {
        schema_version: String::from("psion.actual_pretraining_distributed_recovery_drill.v1"),
        run_id: String::from(actual_run_id),
        recovery_state: String::from("recovered"),
        interruption_kind: String::from("planned_checkpoint_boundary_interrupt"),
        interruption_step,
        total_target_optimizer_steps: total_target_steps,
        interrupted_segment_summary_path: interrupted_segment
            .operator_summary_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_segment_artifact_dir: interrupted_segment
            .artifact_dir
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_checkpoint_ref: interrupted_segment.checkpoint_manifest.checkpoint_ref.clone(),
        interrupted_checkpoint_manifest_path: interrupted_segment
            .checkpoint_manifest_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        interrupted_checkpoint_object_digest: interrupted_segment.checkpoint_object_digest.clone(),
        recovered_segment_summary_path: Some(
            recovered_segment
                .operator_summary_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
        ),
        recovered_segment_artifact_dir: Some(
            recovered_segment
                .artifact_dir
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
        ),
        recovered_checkpoint_ref: Some(recovered_segment.checkpoint_manifest.checkpoint_ref.clone()),
        recovered_checkpoint_manifest_path: Some(
            recovered_segment
                .checkpoint_manifest_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
        ),
        recovered_checkpoint_object_digest: Some(recovered_segment.checkpoint_object_digest.clone()),
        lineage_preserved: true,
        refusal_reason: None,
        detail: String::from(
            "Recovery drill stopped the longer actual-lane bringup at one planned checkpoint boundary, resumed from the retained interrupted checkpoint artifact family, and completed the remaining optimizer steps without manual path surgery.",
        ),
    };
    recovery_receipt.validate()?;
    let recovery_receipt_path = output_root.join("actual_pretraining_bringup_recovery_drill.json");
    write_json_pretty(&recovery_receipt_path, &recovery_receipt)?;

    let final_progress_receipt = combined_progress_checkpoint_receipts.last().ok_or_else(|| {
        std::io::Error::other("recovered distributed bringup retained no final progress receipt")
    })?;
    let progress_window_count = combined_progress_checkpoint_receipts
        .iter()
        .map(|receipt| receipt.window_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let progress_cadence_count = combined_progress_checkpoint_receipts
        .iter()
        .filter(|receipt| receipt.is_cadence_boundary)
        .map(|receipt| receipt.cadence_index)
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    let receipt = PsionActualPretrainingDistributedBringupRehearsal {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_DISTRIBUTED_BRINGUP_REHEARSAL_SCHEMA_VERSION,
        ),
        run_id: String::from(actual_run_id),
        bringup_run_id,
        rehearsal_summary_path: String::from(
            "distributed_execution/distributed_actual_pretraining_bringup.json",
        ),
        relative_output_root: relative_output_root.to_string_lossy().replace('\\', "/"),
        operator_manifest_path: interrupted_segment
            .operator_manifest_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        operator_summary_path: recovered_segment
            .operator_summary_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        train_log_path: recovered_segment
            .train_log_path
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        cluster_topology_receipt_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_topology_receipt.json")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        cluster_contribution_receipts_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_contribution_receipts.json")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        contributor_continuity_receipt_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_contributor_continuity_receipt.json")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        progress_checkpoint_receipts_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_cluster_progress_checkpoint_receipts.json")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        progress_checkpoint_directory: combined_progress_dir
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        checkpoint_manifest_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_checkpoint_manifest.json")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        checkpoint_weights_path: combined_artifact_dir
            .join("psion_actual_pretraining_bringup_checkpoint.safetensors")
            .strip_prefix(run_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        recovery_drill_receipt_path: Some(
            recovery_receipt_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/"),
        ),
        execution_topology_classification: recovered_segment
            .topology_receipt
            .execution_topology_classification
            .clone(),
        contributor_count: recovered_segment.topology_receipt.contributor_count,
        worker_hosts: recovered_segment.topology_receipt.worker_hosts.clone(),
        runtime_backends: recovered_segment.topology_receipt.runtime_backends.clone(),
        checkpoint_ref: recovered_segment.checkpoint_manifest.checkpoint_ref.clone(),
        checkpoint_object_digest: recovered_segment.checkpoint_object_digest.clone(),
        checkpoint_total_bytes: recovered_segment.checkpoint_total_bytes,
        optimizer_steps: recovered_segment.checkpoint_manifest.step,
        contribution_receipt_count: combined_contribution_receipts.len(),
        progress_checkpoint_count: combined_progress_checkpoint_receipts.len(),
        progress_window_count,
        progress_cadence_count,
        final_cumulative_train_tokens_processed: final_progress_receipt
            .cumulative_train_tokens_processed,
        final_cumulative_mean_tokens_per_second: final_progress_receipt
            .cumulative_mean_tokens_per_second,
        claim_boundary: String::from(
            "This retained rehearsal proves one bounded multi-host actual-lane execution that was intentionally interrupted at a checkpoint boundary and then recovered on the same checkpoint lineage and artifact family. It does not by itself claim continuous live cluster operation or control-plane truth outside the bounded rehearsal surface.",
        ),
        detail: format!(
            "Distributed bringup recovery drill binds the interrupted segment, recovered segment, combined continuity/progress receipts, and retained recovery drill receipt into one operator-readable proof for the actual lane. Recovery receipt: {}",
            recovery_receipt_path
                .strip_prefix(run_root)?
                .to_string_lossy()
                .replace('\\', "/")
        ),
    };
    receipt.validate()?;
    write_json_pretty(&run_root.join(&receipt.rehearsal_summary_path), &receipt)?;
    Ok(receipt)
}

fn run_rehearsal_subcommand(
    current_exe: &Path,
    subcommand: &str,
    run_id: &str,
    run_root: &Path,
    selected_git_ref: &str,
    hardware_observation_path: Option<&Path>,
    run_shape_observation_path: Option<&Path>,
    allow_dirty_tree: bool,
    extra_args: &[String],
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(current_exe);
    command.args(rehearsal_command_args(current_exe, subcommand));
    match subcommand {
        "start" => {
            command.arg("--run-id").arg(run_id);
            command.arg("--output-root").arg(run_root);
        }
        _ => {
            command.arg("--run-root").arg(run_root);
        }
    }
    command.arg("--git-ref").arg(selected_git_ref);
    if let Some(path) = hardware_observation_path {
        command.arg("--hardware-observation").arg(path);
    }
    if let Some(path) = run_shape_observation_path {
        command.arg("--run-shape-observation").arg(path);
    }
    if allow_dirty_tree {
        command.arg("--allow-dirty-tree");
    }
    command.args(extra_args);
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "rehearsal subcommand `{subcommand}` failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
    .into())
}

fn rehearsal_command_args(current_exe: &Path, subcommand: &str) -> Vec<String> {
    let executable_stem = current_exe
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if executable_stem == "psionic-train" {
        vec![String::from("actual-pretraining"), String::from(subcommand)]
    } else {
        vec![String::from(subcommand)]
    }
}

fn build_base_lane_rehearsal_closeout_bundle(
    run_root: &Path,
    retained_paths: &PsionActualPretrainingRetainedPathSet,
    run_id: &str,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    checkpoint_manifest_path: &Path,
    failed_upload_drill_path: &Path,
    distributed_rehearsal: Option<&PsionActualPretrainingDistributedBringupRehearsal>,
) -> Result<PsionActualPretrainingCloseoutBundle, Box<dyn Error>> {
    let mut evidence_artifacts = vec![
        closeout_artifact(
            run_root,
            "launch_manifest",
            &run_root.join(&retained_paths.launch_manifest_path),
            "Launch manifest proves the actual lane started against the frozen lane, recipe, topology, and evidence contract.",
        )?,
        closeout_artifact(
            run_root,
            "hardware_qualification",
            &run_root.join(&retained_paths.hardware_qualification_path),
            "Hardware qualification proves the admitted machine class and provenance used for the rehearsal.",
        )?,
        closeout_artifact(
            run_root,
            "run_shape_qualification",
            &run_root.join(&retained_paths.run_shape_qualification_path),
            "Run-shape qualification proves the admitted throughput, storage, and dataloader shape consumed by the rehearsal.",
        )?,
        closeout_artifact(
            run_root,
            "checkpoint_pointer",
            &run_root.join(&retained_paths.latest_checkpoint_pointer_path),
            "Latest accepted checkpoint pointer proves the base lane retained one accepted checkpoint lineage.",
        )?,
        closeout_artifact(
            run_root,
            "checkpoint_manifest",
            checkpoint_manifest_path,
            "Checkpoint manifest proves the exact accepted checkpoint family selected for backup, eval, resume, and continuation handoff.",
        )?,
        closeout_artifact(
            run_root,
            "checkpoint_backup_receipt",
            &run_root.join(&retained_paths.latest_checkpoint_backup_receipt_path),
            "Backup receipt proves the latest accepted checkpoint ended in a durable retained backup after the drill replay.",
        )?,
        closeout_artifact(
            run_root,
            "checkpoint_eval_decision",
            &run_root.join(&retained_paths.latest_checkpoint_eval_decision_path),
            "Checkpoint eval decision proves the accepted checkpoint reached the automatic evaluation family.",
        )?,
        closeout_artifact(
            run_root,
            "checkpoint_comparison",
            &run_root.join(&retained_paths.latest_checkpoint_comparison_path),
            "Checkpoint comparison proves the accepted checkpoint was compared against eval, backup, and systems receipts before the long-run decision.",
        )?,
        closeout_artifact(
            run_root,
            "continue_restart_decision",
            &run_root.join(&retained_paths.latest_continue_restart_decision_path),
            "Continue-restart decision proves the base lane retained one machine-readable continue decision over the accepted checkpoint.",
        )?,
        closeout_artifact(
            run_root,
            "auto_resume_receipt",
            &run_root.join(&retained_paths.auto_resume_receipt_path),
            "Auto-resume receipt proves the canonical resume command reselected the accepted checkpoint from the retained family.",
        )?,
        closeout_artifact(
            run_root,
            "resume_manifest",
            &run_root.join(&retained_paths.resume_manifest_path),
            "Resume manifest proves the lane can re-enter the same frozen operator contract from retained checkpoint truth.",
        )?,
        closeout_artifact(
            run_root,
            "retained_summary",
            &run_root.join(&retained_paths.retained_summary_path),
            "Retained summary proves the final rehearsal phase and provenance stayed operator-legible.",
        )?,
        closeout_artifact(
            run_root,
            "current_status",
            &run_root.join(&retained_paths.current_status_path),
            "Current status proves the final rehearsal phase is retained under the canonical status surface.",
        )?,
        closeout_artifact(
            run_root,
            "dashboard_packet",
            &run_root.join(&retained_paths.current_dashboard_path),
            "Dashboard packet proves the retained operator dashboard stayed aligned with the final rehearsal summary.",
        )?,
        closeout_artifact(
            run_root,
            "active_alert_feed",
            &run_root.join(&retained_paths.active_alert_feed_path),
            "Active-alert feed proves the drill alert posture returned to a clean retained state after recovery.",
        )?,
        closeout_artifact(
            run_root,
            "continuation_handoff",
            &run_root.join(&retained_paths.continuation_handoff_path),
            "Continuation handoff proves the base lane ends in one explicit bounded continuation target without claiming continuation execution.",
        )?,
    ];
    if let Some(rehearsal) = distributed_rehearsal {
        evidence_artifacts.extend([
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup",
                &run_root.join(&rehearsal.rehearsal_summary_path),
                "Distributed bringup summary proves the actual-lane operator delegated one bounded multi-host optimizer-bearing segment through the actual-pretraining bringup workload.",
            )?,
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup_topology_receipt",
                &run_root.join(&rehearsal.cluster_topology_receipt_path),
                "Cluster topology receipt proves which hosts and runtime backends materially contributed to the bounded actual-pretraining bringup segment.",
            )?,
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup_contribution_receipts",
                &run_root.join(&rehearsal.cluster_contribution_receipts_path),
                "Contribution receipts prove that each configured contributor participated in the distributed optimizer path.",
            )?,
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup_contributor_continuity_receipt",
                &run_root.join(&rehearsal.contributor_continuity_receipt_path),
                "Contributor continuity receipt proves that every configured contributor stayed in the optimization path throughout the bounded actual-pretraining bringup segment.",
            )?,
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup_progress_checkpoint_receipts",
                &run_root.join(&rehearsal.progress_checkpoint_receipts_path),
                "Progress checkpoint receipts prove the bounded actual-pretraining bringup retained checkpoint evidence at window and cadence boundaries rather than only at terminal closeout.",
            )?,
            closeout_artifact(
                run_root,
                "distributed_actual_pretraining_bringup_checkpoint_manifest",
                &run_root.join(&rehearsal.checkpoint_manifest_path),
                "Distributed bringup checkpoint manifest proves the exact bounded checkpoint lineage promoted into the actual-lane checkpoint family.",
            )?,
        ]);
        if let Some(recovery_drill_receipt_path) = rehearsal.recovery_drill_receipt_path.as_deref()
        {
            evidence_artifacts.push(closeout_artifact(
                run_root,
                "distributed_actual_pretraining_recovery_drill",
                &run_root.join(recovery_drill_receipt_path),
                "Recovery drill receipt proves the longer actual-lane bringup was intentionally interrupted and then recovered on the same retained checkpoint lineage without manual path surgery.",
            )?);
        }
    }
    let mut closeout_gates = vec![
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("launch_preflight_admitted"),
            satisfied: true,
            detail: String::from(
                "The rehearsal started through the actual-lane operator command with admitted hardware and run-shape receipts.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("accepted_checkpoint_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal retained one accepted checkpoint pointer plus the matching checkpoint manifest.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("automatic_checkpoint_eval_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal retained one automatic checkpoint eval decision for the accepted checkpoint.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("checkpoint_backup_success_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal ended with a successful backup receipt after replaying the failed-upload drill.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("continue_decision_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal retained a machine-readable `continue` decision over the accepted checkpoint.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("resume_manifest_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal retained a resume manifest plus auto-resume receipt for the accepted checkpoint lineage.",
            ),
        },
        PsionActualPretrainingCloseoutGate {
            gate_id: String::from("failure_drill_retained"),
            satisfied: true,
            detail: String::from(
                "The rehearsal retained one injected failed-upload drill and then recovered to a successful backup state.",
            ),
        },
    ];
    if let Some(rehearsal) = distributed_rehearsal {
        closeout_gates.push(PsionActualPretrainingCloseoutGate {
            gate_id: String::from("bounded_distributed_training_proof_retained"),
            satisfied: true,
            detail: format!(
                "The actual-lane rehearsal retained one bounded {} proof with {} contributors, {} contribution receipts, and {} progress checkpoints across {} windows.",
                rehearsal.execution_topology_classification,
                rehearsal.contributor_count,
                rehearsal.contribution_receipt_count,
                rehearsal.progress_checkpoint_count,
                rehearsal.progress_window_count
            ),
        });
        if rehearsal.recovery_drill_receipt_path.is_some() {
            closeout_gates.push(PsionActualPretrainingCloseoutGate {
                gate_id: String::from("bounded_distributed_recovery_drill_retained"),
                satisfied: true,
                detail: String::from(
                    "The actual-lane rehearsal retained one planned interruption and successful recovery drill on the same accepted checkpoint lineage.",
                ),
            });
        }
    }
    let failure_drills = vec![PsionActualPretrainingCloseoutFailureDrill {
        drill_id: String::from("failed_upload_drill"),
        resolution_state: String::from("retained_and_recovered"),
        artifact: run_artifact_ref(run_root, failed_upload_drill_path)?,
        detail: String::from(
            "The base-lane rehearsal injected a failed checkpoint upload, retained the drill packet, and then replayed backup successfully without manual artifact edits.",
        ),
    }];

    let mut can_now_claim = vec![
        String::from(
            "The actual pretraining lane now has one retained operator proof that launch, accepted checkpoint retention, backup, automatic checkpoint eval, continue-vs-restart decision, and resume all compose into one base-lane path.",
        ),
        String::from(
            "The retained evidence family now carries one injected failed-upload drill plus recovery without manual artifact edits.",
        ),
        String::from(
            "The accepted checkpoint lineage now feeds one explicit continuation handoff target, even though continuation execution remains separately unproved.",
        ),
    ];
    if let Some(rehearsal) = distributed_rehearsal {
        can_now_claim.push(format!(
            "The actual pretraining lane now also retains one bounded {} training segment where {} configured contributors materially participated in model-progress-bearing work.",
            rehearsal.execution_topology_classification,
            rehearsal.contributor_count
        ));
        if rehearsal.recovery_drill_receipt_path.is_some() {
            can_now_claim.push(String::from(
                "The actual pretraining lane now retains one planned interruption and recovery proof where the longer tri-host bringup resumed from the interrupted checkpoint family without manual path surgery.",
            ));
        }
    }
    let mut still_out_of_scope = vec![
        String::from("External alert delivery or paging for actual-lane failures."),
        String::from("Streaming or cluster-connected dashboard publication."),
        String::from("Completed distributed broader-pretraining execution on the trusted cluster."),
        String::from(
            "Plugin-conditioned continuation execution beyond the declared handoff artifact.",
        ),
    ];
    if distributed_rehearsal.is_none() {
        still_out_of_scope.push(String::from(
            "Any retained bounded distributed training proof inside the actual pretraining operator path.",
        ));
    }

    Ok(PsionActualPretrainingCloseoutBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        closeout_state: String::from("base_lane_rehearsal_complete"),
        retained_paths: retained_paths.clone(),
        selected_git_ref: String::from(selected_git_ref),
        git_commit_sha: String::from(git_commit_sha),
        dirty_tree_admission: String::from(dirty_tree_admission),
        workspace_status_sha256,
        evidence_artifacts,
        closeout_gates,
        failure_drills,
        can_now_claim,
        still_out_of_scope,
        claim_boundary: match distributed_rehearsal {
            Some(rehearsal) => format!(
                "This closeout bundle proves one retained base-lane rehearsal for the actual pretraining operator path, including launch, one bounded {} distributed training segment, accepted checkpoint retention, backup recovery drill, automatic checkpoint eval, continue-vs-restart decision, and resume. It does not claim external alert delivery, streaming dashboard publication, broader actual-lane distributed cluster execution beyond that bounded rehearsal segment, or plugin-conditioned continuation execution.",
                rehearsal.execution_topology_classification
            ),
            None => String::from(
                "This closeout bundle proves one retained base-lane rehearsal for the actual pretraining operator path, including launch, accepted checkpoint retention, backup recovery drill, automatic checkpoint eval, continue-vs-restart decision, and resume. It does not claim external alert delivery, streaming dashboard publication, distributed broader-pretraining execution, or plugin-conditioned continuation execution.",
            ),
        },
        detail: match distributed_rehearsal {
            Some(rehearsal) => format!(
                "Base-lane closeout bundle ties the exact retained rehearsal artifacts, the bounded {} proof family, one injected backup failure drill, and one explicit claim boundary into a single operator-readable proof packet.",
                rehearsal.execution_topology_classification
            ),
            None => String::from(
                "Base-lane closeout bundle ties the exact retained rehearsal artifacts, one injected backup failure drill, and one explicit claim boundary into a single operator-readable proof packet.",
            ),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::rehearsal_command_args;
    use std::path::Path;

    #[test]
    fn rehearsal_uses_actual_pretraining_prefix_for_top_level_binary() {
        let args = rehearsal_command_args(Path::new("/tmp/psionic-train"), "start");
        assert_eq!(args, vec!["actual-pretraining", "start"]);
    }

    #[test]
    fn rehearsal_uses_direct_subcommand_for_example_binary() {
        let args = rehearsal_command_args(
            Path::new("/tmp/psion_actual_pretraining_operator"),
            "resume",
        );
        assert_eq!(args, vec!["resume"]);
    }
}

fn closeout_artifact(
    run_root: &Path,
    artifact_kind: &str,
    path: &Path,
    detail: &str,
) -> Result<PsionActualPretrainingCloseoutArtifact, Box<dyn Error>> {
    Ok(PsionActualPretrainingCloseoutArtifact {
        artifact_kind: String::from(artifact_kind),
        artifact: run_artifact_ref(run_root, path)?,
        detail: String::from(detail),
    })
}

fn remove_if_exists(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn run_artifact_ref(
    run_root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative = path
        .strip_prefix(run_root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: file_sha256(path)?,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn checkpoint_backup_pointer_path(run_root: &Path) -> PathBuf {
    run_root.join("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json")
}

fn checkpoint_backup_manifest_path(run_root: &Path, optimizer_step: u64) -> PathBuf {
    run_root.join(format!(
        "checkpoints/backups/step-{optimizer_step}/checkpoint_manifest.backup.json"
    ))
}

fn checkpoint_failure_drill_relative_path(drill_kind: &str) -> String {
    format!("checkpoints/failures/{drill_kind}_drill.json")
}

fn checkpoint_failure_drill_path(run_root: &Path, drill_kind: &str) -> PathBuf {
    run_root.join(checkpoint_failure_drill_relative_path(drill_kind))
}

fn checkpoint_failure_drill_kind_for_primary_pointer_state(
    primary_pointer_state: &str,
) -> Option<&'static str> {
    match primary_pointer_state {
        "corrupt" => Some("corrupt_pointer"),
        "stale" => Some("stale_pointer"),
        _ => None,
    }
}

fn load_checkpoint_manifest(
    run_root: &Path,
    relative_path: &str,
) -> Result<PsionActualPretrainingCheckpointManifest, Box<dyn Error>> {
    let manifest_path = run_root.join(relative_path);
    let manifest: PsionActualPretrainingCheckpointManifest = load_json(&manifest_path)?;
    manifest.validate()?;
    Ok(manifest)
}

fn validate_resume_candidate(
    run_root: &Path,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
) -> Result<PsionActualPretrainingCheckpointManifest, String> {
    checkpoint_pointer
        .validate()
        .map_err(|error| error.to_string())?;
    if checkpoint_pointer.pointer_state != "accepted" {
        return Err(String::from(
            "resume requires an accepted checkpoint pointer under checkpoints/latest_accepted_checkpoint_pointer.json",
        ));
    }
    let manifest_relative_path = checkpoint_pointer
        .checkpoint_manifest_relative_path
        .as_deref()
        .ok_or_else(|| {
            String::from("accepted checkpoint pointer is missing checkpoint manifest path")
        })?;
    let manifest = load_checkpoint_manifest(run_root, manifest_relative_path)
        .map_err(|error| error.to_string())?;
    if manifest.run_id != checkpoint_pointer.run_id {
        return Err(String::from(
            "checkpoint manifest run_id drifted from the accepted checkpoint pointer",
        ));
    }
    if manifest.checkpoint_label != checkpoint_pointer.checkpoint_label {
        return Err(String::from(
            "checkpoint manifest label drifted from the accepted checkpoint pointer",
        ));
    }
    if manifest.optimizer_step != checkpoint_pointer.optimizer_step {
        return Err(String::from(
            "checkpoint manifest optimizer_step drifted from the accepted checkpoint pointer",
        ));
    }
    if manifest.checkpoint_ref
        != checkpoint_pointer
            .checkpoint_ref
            .as_deref()
            .ok_or_else(|| String::from("accepted checkpoint pointer is missing checkpoint ref"))?
    {
        return Err(String::from(
            "checkpoint manifest ref drifted from the accepted checkpoint pointer",
        ));
    }
    Ok(manifest)
}

#[allow(clippy::too_many_arguments)]
fn materialize_checkpoint_backup(
    run_root: &Path,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    checkpoint_manifest: &PsionActualPretrainingCheckpointManifest,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
    contracts: &FrozenContracts,
    inject_failed_upload: bool,
) -> Result<
    (
        PsionActualPretrainingCheckpointBackupReceipt,
        Option<PsionActualPretrainingCheckpointFailureDrill>,
    ),
    Box<dyn Error>,
> {
    let primary_pointer_path = run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let primary_manifest_path = run_root.join(&checkpoint_manifest.relative_manifest_path);
    let backup_pointer_path = checkpoint_backup_pointer_path(run_root);
    let backup_manifest_path =
        checkpoint_backup_manifest_path(run_root, checkpoint_manifest.optimizer_step);
    write_json_pretty(&backup_pointer_path, checkpoint_pointer)?;
    write_json_pretty(&backup_manifest_path, checkpoint_manifest)?;
    let backup_state = if inject_failed_upload {
        "refused"
    } else {
        "backed_up"
    };
    let upload_outcome = if inject_failed_upload {
        "failed"
    } else {
        "succeeded"
    };
    let failure_reason = inject_failed_upload.then(|| {
        String::from(
            "Injected failed checkpoint upload drill refused durable remote backup confirmation without copying any secret payload into retained evidence.",
        )
    });
    let remote_backup_root = format!(
        "{}/backups",
        run_roots(run_root, &checkpoint_pointer.run_id, &contracts.topology).remote_checkpoint_root
    );
    let receipt = record_psion_actual_pretraining_checkpoint_backup_receipt(
        &checkpoint_pointer.run_id,
        &checkpoint_pointer.checkpoint_label,
        checkpoint_pointer.optimizer_step,
        checkpoint_pointer
            .checkpoint_ref
            .as_deref()
            .ok_or_else(|| {
                std::io::Error::other("accepted checkpoint pointer is missing checkpoint_ref")
            })?,
        selected_git_ref,
        git_commit_sha,
        dirty_tree_admission,
        workspace_status_sha256.clone(),
        run_artifact_ref(run_root, &primary_pointer_path)?,
        run_artifact_ref(run_root, &primary_manifest_path)?,
        run_artifact_ref(run_root, &backup_pointer_path)?,
        run_artifact_ref(run_root, &backup_manifest_path)?,
        &remote_backup_root,
        contracts
            .topology
            .credential_sources
            .iter()
            .map(|source| source.source_name.clone())
            .collect(),
        backup_state,
        upload_outcome,
        failure_reason.clone(),
        "This retained backup receipt binds the actual-lane latest accepted checkpoint to one durable backup contract and redacted credential-source posture. It does not claim that training continued or that automatic checkpoint eval already ran.",
        "Checkpoint backup receipt preserves the accepted pointer plus checkpoint manifest under one local backup family and one redacted remote-backup root.",
    )?;
    write_json_pretty(
        &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
        &receipt,
    )?;
    let failure_drill = if inject_failed_upload {
        let drill = record_psion_actual_pretraining_checkpoint_failure_drill(
            &checkpoint_pointer.run_id,
            &format!(
                "psion_actual_pretraining_checkpoint_failure_drill::{}::failed_upload",
                checkpoint_pointer.optimizer_step
            ),
            "failed_upload",
            "backup",
            selected_git_ref,
            git_commit_sha,
            dirty_tree_admission,
            workspace_status_sha256,
            "retained_refusal",
            vec![
                String::from("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
                String::from("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
                format!(
                    "checkpoints/backups/step-{}/checkpoint_manifest.backup.json",
                    checkpoint_pointer.optimizer_step
                ),
            ],
            failure_reason,
            "This retained failure drill proves that checkpoint-upload failures surface as explicit refusal evidence under the actual-lane family rather than silent launcher optimism.",
            "Injected failed-upload drill retained the refusal receipt and local backup copies without requiring manual log surgery.",
        )?;
        Some(drill)
    } else {
        None
    };
    Ok((receipt, failure_drill))
}

#[allow(clippy::too_many_arguments)]
fn resolve_resume_target(
    run_root: &Path,
    selected_git_ref: &str,
    git_commit_sha: &str,
    dirty_tree_admission: &str,
    workspace_status_sha256: Option<String>,
) -> Result<ResolvedResumeTarget, Box<dyn Error>> {
    let primary_pointer_path = run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let backup_receipt_path =
        run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json");
    let backup_pointer_path = checkpoint_backup_pointer_path(run_root);

    let primary_pointer_state;
    if primary_pointer_path.is_file() {
        match load_json::<PsionActualPretrainingCheckpointPointer>(&primary_pointer_path) {
            Ok(pointer) => match validate_resume_candidate(run_root, &pointer) {
                Ok(manifest) => {
                    let receipt = record_psion_actual_pretraining_auto_resume_receipt(
                        &pointer.run_id,
                        selected_git_ref,
                        git_commit_sha,
                        dirty_tree_admission,
                        workspace_status_sha256,
                        "accepted",
                        "accepted_primary_pointer",
                        "primary_pointer",
                        false,
                        Some(pointer.checkpoint_label.clone()),
                        Some(pointer.optimizer_step),
                        pointer.checkpoint_ref.clone(),
                        Some(run_artifact_ref(
                            run_root,
                            &run_root.join(&manifest.relative_manifest_path),
                        )?),
                        None,
                        "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
                        "Auto-resume accepted the primary retained checkpoint pointer without needing the backup copy.",
                    )?;
                    return Ok(ResolvedResumeTarget {
                        checkpoint_pointer: Some(pointer),
                        auto_resume_receipt: receipt,
                        failure_drill: None,
                    });
                }
                Err(_) => {
                    primary_pointer_state = String::from("stale");
                }
            },
            Err(_) => {
                primary_pointer_state = String::from("corrupt");
            }
        }
    } else {
        primary_pointer_state = String::from("missing");
    }

    let recovery = (|| -> Result<ResolvedResumeTarget, Box<dyn Error>> {
        let backup_receipt: PsionActualPretrainingCheckpointBackupReceipt =
            load_json(&backup_receipt_path)?;
        backup_receipt.validate()?;
        if backup_receipt.backup_state != "backed_up" {
            return Err(std::io::Error::other(
                "latest checkpoint backup receipt is not durable enough for auto-resume",
            )
            .into());
        }
        let checkpoint_pointer: PsionActualPretrainingCheckpointPointer =
            load_json(&backup_pointer_path)?;
        let backup_manifest_path = run_root.join(&backup_receipt.backup_checkpoint_manifest.path);
        let checkpoint_manifest: PsionActualPretrainingCheckpointManifest =
            load_json(&backup_manifest_path)?;
        checkpoint_manifest.validate()?;
        write_json_pretty(&primary_pointer_path, &checkpoint_pointer)?;
        write_json_pretty(
            &run_root.join(&checkpoint_manifest.relative_manifest_path),
            &checkpoint_manifest,
        )?;
        let auto_resume_receipt = record_psion_actual_pretraining_auto_resume_receipt(
            &checkpoint_pointer.run_id,
            selected_git_ref,
            git_commit_sha,
            dirty_tree_admission,
            workspace_status_sha256.clone(),
            &primary_pointer_state,
            "recovered_from_backup",
            "backup_receipt",
            true,
            Some(checkpoint_pointer.checkpoint_label.clone()),
            Some(checkpoint_pointer.optimizer_step),
            checkpoint_pointer.checkpoint_ref.clone(),
            Some(run_artifact_ref(
                run_root,
                &run_root.join(&checkpoint_manifest.relative_manifest_path),
            )?),
            None,
            "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
            "Auto-resume restored the primary pointer from the retained backup receipt without requiring manual file edits.",
        )?;
        let failure_drill = match checkpoint_failure_drill_kind_for_primary_pointer_state(
            &primary_pointer_state,
        ) {
            Some(drill_kind) => {
                let drill = record_psion_actual_pretraining_checkpoint_failure_drill(
                    &checkpoint_pointer.run_id,
                    &format!(
                        "psion_actual_pretraining_checkpoint_failure_drill::{}::{drill_kind}",
                        checkpoint_pointer.optimizer_step
                    ),
                    drill_kind,
                    "resume",
                    selected_git_ref,
                    git_commit_sha,
                    dirty_tree_admission,
                    workspace_status_sha256.clone(),
                    "recovered_without_manual_edit",
                    vec![
                        String::from("checkpoints/auto_resume_receipt.json"),
                        String::from("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
                        String::from(
                            "checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json",
                        ),
                        format!(
                            "checkpoints/backups/step-{}/checkpoint_manifest.backup.json",
                            checkpoint_pointer.optimizer_step
                        ),
                    ],
                    None,
                    "This retained failure drill proves that stale or corrupt primary resume pointers recover from the actual-lane backup family without manual editing.",
                    "Auto-resume repaired the primary checkpoint lineage from the retained backup copy after detecting a stale or corrupt primary pointer.",
                )?;
                Some(drill)
            }
            _ => None,
        };
        Ok(ResolvedResumeTarget {
            checkpoint_pointer: Some(checkpoint_pointer),
            auto_resume_receipt,
            failure_drill,
        })
    })();

    match recovery {
        Ok(resolved) => Ok(resolved),
        Err(_) => {
            let run_id = if let Ok(pointer) =
                load_json::<PsionActualPretrainingCheckpointPointer>(&backup_pointer_path)
            {
                pointer.run_id
            } else {
                String::from("unknown_run")
            };
            let auto_resume_receipt = record_psion_actual_pretraining_auto_resume_receipt(
                &run_id,
                selected_git_ref,
                git_commit_sha,
                dirty_tree_admission,
                workspace_status_sha256.clone(),
                &primary_pointer_state,
                "refused",
                "none",
                false,
                None,
                None,
                None,
                None,
                Some(String::from(
                    "primary pointer could not be resumed and no admitted backup receipt was available",
                )),
                "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
                "Auto-resume refused because neither the primary pointer nor the retained backup family could produce an admitted checkpoint selection.",
            )?;
            Ok(ResolvedResumeTarget {
                checkpoint_pointer: None,
                auto_resume_receipt,
                failure_drill: None,
            })
        }
    }
}

fn write_launcher_bundle(
    run_root: &Path,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    launch_manifest: Option<&PsionActualPretrainingLaunchManifest>,
    resume_manifest: Option<&PsionActualPretrainingResumeManifest>,
    current_status: &PsionActualPretrainingCurrentRunStatus,
    retained_summary: &PsionActualPretrainingRetainedSummary,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    continuation_handoff: Option<&PsionActualPretrainingContinuationHandoff>,
    closeout_bundle: &PsionActualPretrainingCloseoutBundle,
    launcher_log_line: &str,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(run_root.join("manifests"))?;
    fs::create_dir_all(run_root.join("status"))?;
    fs::create_dir_all(run_root.join("checkpoints"))?;
    fs::create_dir_all(run_root.join("preflight"))?;
    fs::create_dir_all(run_root.join("continuation"))?;
    fs::create_dir_all(run_root.join("decisions"))?;
    fs::create_dir_all(run_root.join("dashboard"))?;
    fs::create_dir_all(run_root.join("closeout"))?;
    fs::create_dir_all(run_root.join("logs"))?;
    fs::create_dir_all(run_root.join("alerts"))?;
    if let Some(launch_manifest) = launch_manifest {
        fs::write(
            run_root.join("manifests/launch_manifest.json"),
            serde_json::to_string_pretty(launch_manifest)?,
        )?;
    }
    if let Some(resume_manifest) = resume_manifest {
        fs::write(
            run_root.join("manifests/resume_manifest.json"),
            serde_json::to_string_pretty(resume_manifest)?,
        )?;
    }
    fs::write(
        run_root.join("status/current_run_status.json"),
        serde_json::to_string_pretty(current_status)?,
    )?;
    fs::write(
        run_root.join("status/retained_summary.json"),
        serde_json::to_string_pretty(retained_summary)?,
    )?;
    fs::write(
        run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        serde_json::to_string_pretty(checkpoint_pointer)?,
    )?;
    fs::write(
        run_root.join("preflight/hardware_qualification.json"),
        serde_json::to_string_pretty(hardware_qualification)?,
    )?;
    fs::write(
        run_root.join("preflight/run_shape_qualification.json"),
        serde_json::to_string_pretty(run_shape_qualification)?,
    )?;
    write_dashboard_bundle(
        run_root,
        current_status,
        retained_summary,
        checkpoint_pointer,
        hardware_qualification,
        run_shape_qualification,
        systems_bundle,
        None,
        None,
        None,
        None,
    )?;
    if let Some(continuation_handoff) = continuation_handoff {
        fs::write(
            run_root.join(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
            serde_json::to_string_pretty(continuation_handoff)?,
        )?;
    }
    fs::write(
        run_root.join("closeout/closeout_bundle.json"),
        serde_json::to_string_pretty(closeout_bundle)?,
    )?;
    append_launcher_log(run_root, launcher_log_line)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_dashboard_bundle(
    run_root: &Path,
    current_status: &PsionActualPretrainingCurrentRunStatus,
    retained_summary: &PsionActualPretrainingRetainedSummary,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    hardware_qualification: &PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    checkpoint_backup_receipt: Option<&PsionActualPretrainingCheckpointBackupReceipt>,
    checkpoint_eval_decision: Option<&PsionActualPretrainingCheckpointEvalDecision>,
    checkpoint_eval_failure: Option<&PsionActualPretrainingCheckpointEvalFailure>,
    latest_redacted_alert: Option<&PsionActualPretrainingRedactedAlert>,
) -> Result<(), Box<dyn Error>> {
    let (dashboard, alert_feed): (
        PsionActualPretrainingDashboardPacket,
        PsionActualPretrainingAlertFeed,
    ) = build_psion_actual_pretraining_dashboard_packet(
        current_status,
        retained_summary,
        checkpoint_pointer,
        hardware_qualification,
        run_shape_qualification,
        systems_bundle,
        checkpoint_backup_receipt,
        checkpoint_eval_decision,
        checkpoint_eval_failure,
        latest_redacted_alert,
    )?;
    write_json_pretty(
        &run_root.join(PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH),
        &dashboard,
    )?;
    write_json_pretty(
        &run_root.join(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        &alert_feed,
    )?;
    Ok(())
}
