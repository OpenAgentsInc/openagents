use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH,
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH,
    PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_DRY_RUN_SURFACE_ID, PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_COMPARISON_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_FAILURE_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CONTINUE_RESTART_DECISION_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH,
    PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_START_SURFACE_ID, PSION_ACTUAL_PRETRAINING_STATUS_SURFACE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingBaselineToolsBundle, PsionActualPretrainingCheckpointPointer,
    PsionActualPretrainingCloseoutBundle, PsionActualPretrainingContinuationHandoff,
    PsionActualPretrainingCredentialBinding, PsionActualPretrainingCurrentRunStatus,
    PsionActualPretrainingDataBundle, PsionActualPretrainingEvidenceContract,
    PsionActualPretrainingHardwareObservation, PsionActualPretrainingLaunchManifest,
    PsionActualPretrainingLauncherContractRefs, PsionActualPretrainingLauncherSurfaces,
    PsionActualPretrainingPreflightRef, PsionActualPretrainingRecipeBundle,
    PsionActualPretrainingResumeManifest, PsionActualPretrainingRetainedPathSet,
    PsionActualPretrainingRetainedSummary, PsionActualPretrainingRunRoots,
    PsionActualPretrainingRunShapeObservation, PsionActualPretrainingRunShapeQualification,
    PsionActualPretrainingScalingBundle, PsionActualPretrainingSystemsBundle,
    PsionActualPretrainingTopologyStorageBundle, PsionPluginConditionedSftStageManifest,
    build_psion_actual_pretraining_dashboard_packet,
    derive_psion_actual_pretraining_hardware_qualification,
    derive_psion_actual_pretraining_run_shape_qualification,
    record_psion_actual_pretraining_continuation_handoff,
};
use sha2::{Digest, Sha256};

const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let recipe: PsionActualPretrainingRecipeBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json"),
    )?;
    recipe.validate()?;
    let baseline_tools_bundle: PsionActualPretrainingBaselineToolsBundle = load_json(
        &root
            .join("fixtures/psion/pretrain/psion_actual_pretraining_baseline_tools_bundle_v1.json"),
    )?;
    baseline_tools_bundle.validate()?;
    let scaling_bundle: PsionActualPretrainingScalingBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_scaling_bundle_v1.json"),
    )?;
    scaling_bundle.validate()?;
    let data_bundle: PsionActualPretrainingDataBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"),
    )?;
    data_bundle.validate()?;
    let topology: PsionActualPretrainingTopologyStorageBundle =
        load_json(&root.join(
            "fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json",
        ))?;
    topology.validate()?;
    let systems_bundle: PsionActualPretrainingSystemsBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
    )?;
    systems_bundle.validate()?;
    let evidence_contract: PsionActualPretrainingEvidenceContract = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"),
    )?;
    evidence_contract.validate()?;
    let hardware_observation: PsionActualPretrainingHardwareObservation = load_json(&root.join(
        "fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json",
    ))?;
    hardware_observation.validate()?;
    let run_shape_observation: PsionActualPretrainingRunShapeObservation = load_json(&root.join(
        "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json",
    ))?;
    run_shape_observation.validate()?;
    let plugin_conditioned_stage_manifest: PsionPluginConditionedSftStageManifest = load_json(
        &root.join(
            &recipe
                .continuation_target
                .plugin_conditioned_stage_manifest
                .path,
        ),
    )?;

    let retained_paths = retained_paths();
    retained_paths.validate()?;
    let launcher_surfaces = launcher_surfaces();
    launcher_surfaces.validate()?;
    let contract_refs = contract_refs(&root)?;
    contract_refs.validate()?;
    let credential_sources = credential_sources(&topology);

    let launch_run_id = "run-psion-actual-20260402t010000z";
    let launch_hardware_qualification = derive_psion_actual_pretraining_hardware_qualification(
        launch_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        &hardware_observation,
        Some(artifact_ref(
            &root,
            &root.join(
                "fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json",
            ),
        )?),
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json"),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"),
        )?,
        &topology,
        &systems_bundle,
        &evidence_contract,
    )?;
    let launch_preflight_receipt = PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.hardware_qualification_path.clone(),
        receipt_digest: launch_hardware_qualification.receipt_digest.clone(),
        admission_state: launch_hardware_qualification.admission_state.clone(),
    };
    let launch_run_shape_qualification =
        derive_psion_actual_pretraining_run_shape_qualification(
            launch_run_id,
            "refs/heads/main",
            FIXTURE_GIT_SHA,
            "refuse_by_default",
            &run_shape_observation,
            Some(artifact_ref(
                &root,
                &root.join(
                    "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json",
                ),
            )?),
            artifact_ref(
                &root,
                &root.join(
                    "fixtures/psion/pretrain/psion_actual_pretraining_baseline_tools_bundle_v1.json",
                ),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"),
            )?,
            &baseline_tools_bundle,
            &data_bundle,
            &systems_bundle,
            &evidence_contract,
        )?;
    let launch_run_shape_receipt = PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.run_shape_qualification_path.clone(),
        receipt_digest: launch_run_shape_qualification.receipt_digest.clone(),
        admission_state: launch_run_shape_qualification.admission_state.clone(),
    };
    let launch_manifest = PsionActualPretrainingLaunchManifest {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_LAUNCH_MANIFEST_SCHEMA_VERSION),
        surface_id: String::from(PSION_ACTUAL_PRETRAINING_START_SURFACE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        topology_storage_bundle_id: String::from(
            PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
        ),
        evidence_contract_id: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
        run_id: String::from(launch_run_id),
        retained_paths: retained_paths.clone(),
        launcher_surfaces: launcher_surfaces.clone(),
        run_roots: run_roots(launch_run_id, &topology),
        preflight_receipt: launch_preflight_receipt.clone(),
        run_shape_receipt: launch_run_shape_receipt.clone(),
        contract_refs: contract_refs.clone(),
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from(FIXTURE_GIT_SHA),
        dirty_tree_admission: String::from("refuse_by_default"),
        workspace_status_sha256: None,
        credential_sources: credential_sources.clone(),
        claim_boundary: String::from(
            "The actual-lane launcher materializes the frozen launch manifest, retained status surfaces, checkpoint pointer, and provisional closeout bundle. It does not by itself execute the distributed broader-pretraining run.",
        ),
        detail: String::from(
            "Launch manifest binds the actual pretraining operator command to the frozen lane, recipe, baseline-tools, scaling, data, systems, topology/storage, evidence, and git-provenance surfaces.",
        ),
    };
    launch_manifest.validate()?;

    let pending_pointer = PsionActualPretrainingCheckpointPointer {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(launch_run_id),
        pointer_state: String::from("pending_first_checkpoint"),
        checkpoint_label: String::from("pending_first_checkpoint"),
        optimizer_step: 0,
        checkpoint_ref: None,
        checkpoint_manifest_relative_path: None,
        detail: String::from(
            "Launch-time checkpoint pointer records that the actual lane has not yet admitted the first durable checkpoint.",
        ),
    };
    pending_pointer.validate()?;

    let launch_status = PsionActualPretrainingCurrentRunStatus {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(launch_run_id),
        phase: String::from("launch_staged"),
        current_status_path: String::from("status/current_run_status.json"),
        retained_summary_path: String::from("status/retained_summary.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        latest_checkpoint_label: String::from("pending_first_checkpoint"),
        last_completed_step: 0,
        launcher_surfaces: launcher_surfaces.clone(),
        updated_at_utc: String::from("2026-04-02T15:00:00Z"),
        detail: String::from(
            "Launch-staged status records that the operator contract is materialized and waiting for the first admitted checkpoint.",
        ),
    };
    launch_status.validate()?;

    let launch_summary = PsionActualPretrainingRetainedSummary {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(launch_run_id),
        last_known_phase: String::from("launch_staged"),
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from(FIXTURE_GIT_SHA),
        dirty_tree_admission: String::from("refuse_by_default"),
        current_status_path: String::from("status/current_run_status.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        launcher_surfaces: launcher_surfaces.clone(),
        claim_boundary: String::from(
            "The retained summary records actual-lane start, dry-run, resume, and status surfaces plus the last known operator state. It does not claim that cluster execution, automatic eval, or durable backup are finished.",
        ),
        detail: String::from(
            "Retained summary keeps the last known actual-lane operator state legible outside the launch manifest.",
        ),
    };
    launch_summary.validate()?;

    let launch_closeout = PsionActualPretrainingCloseoutBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CLOSEOUT_BUNDLE_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(launch_run_id),
        closeout_state: String::from("launch_staged"),
        retained_paths: retained_paths.clone(),
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from(FIXTURE_GIT_SHA),
        dirty_tree_admission: String::from("refuse_by_default"),
        workspace_status_sha256: None,
        evidence_artifacts: Vec::new(),
        closeout_gates: Vec::new(),
        failure_drills: Vec::new(),
        can_now_claim: Vec::new(),
        still_out_of_scope: Vec::new(),
        claim_boundary: String::from(
            "This provisional closeout bundle repeats launcher provenance early so later closeout work can extend the same evidence family without losing source-state identity. It does not claim completed training.",
        ),
        detail: String::from(
            "Launch-staged closeout bundle seeds the actual-lane evidence family with repeated git provenance.",
        ),
    };
    launch_closeout.validate()?;

    let resume_run_id = "run-psion-actual-20260402t020000z";
    let resume_hardware_qualification = derive_psion_actual_pretraining_hardware_qualification(
        resume_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        &hardware_observation,
        Some(artifact_ref(
            &root,
            &root.join(
                "fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json",
            ),
        )?),
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json"),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"),
        )?,
        &topology,
        &systems_bundle,
        &evidence_contract,
    )?;
    let resume_preflight_receipt = PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.hardware_qualification_path.clone(),
        receipt_digest: resume_hardware_qualification.receipt_digest.clone(),
        admission_state: resume_hardware_qualification.admission_state.clone(),
    };
    let resume_run_shape_qualification =
        derive_psion_actual_pretraining_run_shape_qualification(
            resume_run_id,
            "refs/heads/main",
            FIXTURE_GIT_SHA,
            "refuse_by_default",
            &run_shape_observation,
            Some(artifact_ref(
                &root,
                &root.join(
                    "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json",
                ),
            )?),
            artifact_ref(
                &root,
                &root.join(
                    "fixtures/psion/pretrain/psion_actual_pretraining_baseline_tools_bundle_v1.json",
                ),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
            )?,
            artifact_ref(
                &root,
                &root.join("fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json"),
            )?,
            &baseline_tools_bundle,
            &data_bundle,
            &systems_bundle,
            &evidence_contract,
        )?;
    let resume_run_shape_receipt = PsionActualPretrainingPreflightRef {
        relative_path: retained_paths.run_shape_qualification_path.clone(),
        receipt_digest: resume_run_shape_qualification.receipt_digest.clone(),
        admission_state: resume_run_shape_qualification.admission_state.clone(),
    };
    let accepted_pointer = PsionActualPretrainingCheckpointPointer {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(resume_run_id),
        pointer_state: String::from("accepted"),
        checkpoint_label: String::from("broader-pretrain-final"),
        optimizer_step: 16384,
        checkpoint_ref: Some(String::from("checkpoint://psion/broad/pretrain/final")),
        checkpoint_manifest_relative_path: Some(String::from(
            "checkpoints/step-16384/checkpoint_manifest.json",
        )),
        detail: String::from(
            "Resume pointer binds the canonical resume path to the latest accepted broader-pretraining checkpoint lineage.",
        ),
    };
    accepted_pointer.validate()?;

    let resume_manifest = PsionActualPretrainingResumeManifest {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RESUME_MANIFEST_SCHEMA_VERSION),
        surface_id: String::from(PSION_ACTUAL_PRETRAINING_RESUME_SURFACE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        topology_storage_bundle_id: String::from(
            PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
        ),
        evidence_contract_id: String::from(PSION_ACTUAL_PRETRAINING_EVIDENCE_CONTRACT_ID),
        run_id: String::from(resume_run_id),
        retained_paths: retained_paths.clone(),
        launcher_surfaces: launcher_surfaces.clone(),
        run_roots: run_roots(resume_run_id, &topology),
        preflight_receipt: resume_preflight_receipt.clone(),
        run_shape_receipt: resume_run_shape_receipt.clone(),
        contract_refs,
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from(FIXTURE_GIT_SHA),
        dirty_tree_admission: String::from("refuse_by_default"),
        workspace_status_sha256: None,
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        checkpoint_label: String::from("broader-pretrain-final"),
        optimizer_step: 16384,
        checkpoint_ref: String::from("checkpoint://psion/broad/pretrain/final"),
        claim_boundary: String::from(
            "The actual-lane resume manifest binds the canonical resume command to the accepted checkpoint pointer inside the frozen evidence family. It does not claim post-resume training success by itself.",
        ),
        detail: String::from(
            "Resume manifest records the exact accepted checkpoint selection and repeats launcher provenance plus the frozen baseline-tools, scaling, data, and systems bundles for restart decisions.",
        ),
    };
    resume_manifest.validate()?;

    let resume_status = PsionActualPretrainingCurrentRunStatus {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_RUN_STATUS_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(resume_run_id),
        phase: String::from("resume_staged"),
        current_status_path: String::from("status/current_run_status.json"),
        retained_summary_path: String::from("status/retained_summary.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        latest_checkpoint_label: String::from("broader-pretrain-final"),
        last_completed_step: 16384,
        launcher_surfaces: launcher_surfaces.clone(),
        updated_at_utc: String::from("2026-04-02T15:10:00Z"),
        detail: String::from(
            "Resume-staged status records the accepted checkpoint selected by the canonical resume path.",
        ),
    };
    resume_status.validate()?;

    let resume_summary = PsionActualPretrainingRetainedSummary {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RETAINED_SUMMARY_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(resume_run_id),
        last_known_phase: String::from("resume_staged"),
        selected_git_ref: String::from("refs/heads/main"),
        git_commit_sha: String::from(FIXTURE_GIT_SHA),
        dirty_tree_admission: String::from("refuse_by_default"),
        current_status_path: String::from("status/current_run_status.json"),
        latest_checkpoint_pointer_path: String::from(
            "checkpoints/latest_accepted_checkpoint_pointer.json",
        ),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        launcher_surfaces,
        claim_boundary: String::from(
            "The retained summary records actual-lane start, dry-run, resume, and status surfaces plus the last known operator state. It does not claim that cluster execution, automatic eval, or durable backup are finished.",
        ),
        detail: String::from(
            "Retained summary keeps the accepted resume checkpoint legible for operator review.",
        ),
    };
    resume_summary.validate()?;
    let continuation_handoff = record_psion_actual_pretraining_continuation_handoff(
        &accepted_pointer,
        &recipe,
        &plugin_conditioned_stage_manifest,
    )?;
    continuation_handoff.validate()?;

    fs::write(
        fixtures_dir.join("psion_actual_pretraining_launch_manifest_v1.json"),
        serde_json::to_string_pretty(&launch_manifest)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_actual_pretraining_resume_manifest_v1.json"),
        serde_json::to_string_pretty(&resume_manifest)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_actual_pretraining_checkpoint_pointer_v1.json"),
        serde_json::to_string_pretty(&accepted_pointer)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_actual_pretraining_continuation_handoff_v1.json"),
        serde_json::to_string_pretty(&continuation_handoff)?,
    )?;

    write_run_root(
        &fixtures_dir
            .join("psion_actual_pretraining_launcher_example")
            .join("start")
            .join(launch_run_id),
        &launch_hardware_qualification,
        &launch_run_shape_qualification,
        &systems_bundle,
        Some(&launch_manifest),
        None,
        &launch_status,
        &launch_summary,
        &pending_pointer,
        None,
        &launch_closeout,
        "2026-04-02T15:00:00Z launch_staged surface_id=psion_actual_pretraining.start\n",
    )?;
    write_run_root(
        &fixtures_dir
            .join("psion_actual_pretraining_launcher_example")
            .join("resume")
            .join(resume_run_id),
        &resume_hardware_qualification,
        &resume_run_shape_qualification,
        &systems_bundle,
        None,
        Some(&resume_manifest),
        &resume_status,
        &resume_summary,
        &accepted_pointer,
        Some(&continuation_handoff),
        &PsionActualPretrainingCloseoutBundle {
            closeout_state: String::from("resume_staged"),
            run_id: String::from(resume_run_id),
            detail: String::from(
                "Resume-staged closeout bundle repeats launcher provenance after selecting an accepted checkpoint pointer.",
            ),
            ..launch_closeout
        },
        "2026-04-02T15:10:00Z resume_staged surface_id=psion_actual_pretraining.resume\n",
    )?;

    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn load_json<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn retained_paths() -> PsionActualPretrainingRetainedPathSet {
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
            PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH,
        ),
        latest_checkpoint_eval_failure_path: String::from(
            PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_FAILURE_PATH,
        ),
        latest_checkpoint_comparison_path: String::from(
            PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_COMPARISON_PATH,
        ),
        latest_continue_restart_decision_path: String::from(
            PSION_ACTUAL_PRETRAINING_LATEST_CONTINUE_RESTART_DECISION_PATH,
        ),
        current_dashboard_path: String::from(PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH),
        hardware_qualification_path: String::from("preflight/hardware_qualification.json"),
        run_shape_qualification_path: String::from("preflight/run_shape_qualification.json"),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        closeout_bundle_path: String::from("closeout/closeout_bundle.json"),
        launcher_log_path: String::from("logs/launcher.log"),
        latest_redacted_alert_path: String::from(
            PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH,
        ),
        active_alert_feed_path: String::from(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
    }
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
    run_id: &str,
    topology: &PsionActualPretrainingTopologyStorageBundle,
) -> PsionActualPretrainingRunRoots {
    PsionActualPretrainingRunRoots {
        local_run_root: format!("/tmp/psion_actual_pretraining_runs/{run_id}"),
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

fn contract_refs(
    root: &Path,
) -> Result<PsionActualPretrainingLauncherContractRefs, Box<dyn Error>> {
    let pretrain_dir = root.join("fixtures/psion/pretrain");
    Ok(PsionActualPretrainingLauncherContractRefs {
        lane_spec: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_lane_spec_v1.json"),
        )?,
        recipe_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_recipe_bundle_v1.json"),
        )?,
        baseline_tools_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_baseline_tools_bundle_v1.json"),
        )?,
        scaling_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_scaling_bundle_v1.json"),
        )?,
        data_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_data_bundle_v1.json"),
        )?,
        systems_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        topology_storage_bundle: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_topology_storage_bundle_v1.json"),
        )?,
        evidence_contract: artifact_ref(
            root,
            &pretrain_dir.join("psion_actual_pretraining_evidence_contract_v1.json"),
        )?,
    })
}

fn credential_sources(
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
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn write_run_root(
    run_root: &Path,
    hardware_qualification: &psionic_train::PsionActualPretrainingHardwareQualification,
    run_shape_qualification: &PsionActualPretrainingRunShapeQualification,
    systems_bundle: &PsionActualPretrainingSystemsBundle,
    launch_manifest: Option<&PsionActualPretrainingLaunchManifest>,
    resume_manifest: Option<&PsionActualPretrainingResumeManifest>,
    current_status: &PsionActualPretrainingCurrentRunStatus,
    retained_summary: &PsionActualPretrainingRetainedSummary,
    checkpoint_pointer: &PsionActualPretrainingCheckpointPointer,
    continuation_handoff: Option<&PsionActualPretrainingContinuationHandoff>,
    closeout_bundle: &PsionActualPretrainingCloseoutBundle,
    launcher_log: &str,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(run_root.join("manifests"))?;
    fs::create_dir_all(run_root.join("status"))?;
    fs::create_dir_all(run_root.join("checkpoints"))?;
    fs::create_dir_all(run_root.join("preflight"))?;
    fs::create_dir_all(run_root.join("continuation"))?;
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
    let (dashboard, alert_feed) = build_psion_actual_pretraining_dashboard_packet(
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
    fs::write(
        run_root.join(PSION_ACTUAL_PRETRAINING_CURRENT_DASHBOARD_PATH),
        serde_json::to_string_pretty(&dashboard)?,
    )?;
    fs::write(
        run_root.join(PSION_ACTUAL_PRETRAINING_ACTIVE_ALERT_FEED_PATH),
        serde_json::to_string_pretty(&alert_feed)?,
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
    fs::write(run_root.join("logs/launcher.log"), launcher_log)?;
    Ok(())
}
