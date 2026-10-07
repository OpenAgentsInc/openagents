use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use psionic_train::{
    PSION_APPLE_WINDOWED_TRAINING_LANE_ID, PSION_CS336_A1_DEMO_LANE_ID,
    PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF, PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
    PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
    PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID, PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF,
    PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID, PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION,
    PSIONIC_TRAIN_RUNTIME_SURFACE_ID, PsionicTrainAdmissionIdentity,
    PsionicTrainCheckpointHandoffReceipt, PsionicTrainCheckpointHandoffSourceKind,
    PsionicTrainCheckpointManifest, PsionicTrainCheckpointPointer, PsionicTrainCheckpointSurface,
    PsionicTrainContributionArtifactManifest, PsionicTrainContributionReceipt,
    PsionicTrainCoordinationContext, PsionicTrainGroupedReplicaRecoverySourceKind,
    PsionicTrainGroupedReplicaStageAssignment, PsionicTrainGroupedReplicaStageRecoveryReceipt,
    PsionicTrainGroupedReplicaStageRole, PsionicTrainInvocationManifest,
    PsionicTrainMembershipRevisionReceipt, PsionicTrainOperation, PsionicTrainOutcomeKind,
    PsionicTrainRefusalClass, PsionicTrainRole, PsionicTrainRunStatusPacket,
    PsionicTrainSealedWindowBundle, PsionicTrainStatusPacket, PsionicTrainValidatorHook,
    PsionicTrainValidatorQualityDriftSignal, PsionicTrainValidatorQualityDriftState,
    PsionicTrainValidatorRollbackPosture, PsionicTrainValidatorRollbackSignal,
    PsionicTrainValidatorScoreArtifact, PsionicTrainValidatorScoreReceipt,
    PsionicTrainWeakDeviceAcceptedOutcomeProof, PsionicTrainWindowExecution,
    PsionicTrainWindowStatusPacket, PsionicTrainWorkClass, TrainingExecutionValidatorDisposition,
    psionic_train_resolved_artifact_cache_candidates,
    record_psionic_train_weak_device_accepted_outcome_proof, runtime_build_digest,
};
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn binary_path() -> PathBuf {
    PathBuf::from(
        std::env::var("CARGO_BIN_EXE_psionic-train")
            .expect("cargo should provide the psionic-train binary path"),
    )
}

fn git_head() -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse HEAD should run");
    assert!(output.status.success(), "git rev-parse HEAD should succeed");
    String::from_utf8(output.stdout)
        .expect("git output should be utf8")
        .trim()
        .to_string()
}

fn artifact_binding(path: &Path) -> psionic_train::PsionicTrainArtifactBinding {
    let artifact_role = match path.file_name().and_then(|value| value.to_str()) {
        Some("artifact_manifest.json") => "contribution_artifact_manifest",
        Some("contribution_receipt.json") => "contribution_receipt",
        Some("peer_checkpoint_handoff_receipt.json") => "checkpoint_handoff_receipt",
        _ => "cli_test_artifact",
    };
    psionic_train::build_psionic_train_artifact_binding_from_path(artifact_role, path)
        .expect("artifact binding should build from path")
}

fn cache_artifact_binding(
    run_root: &Path,
    binding: &psionic_train::PsionicTrainArtifactBinding,
) -> PathBuf {
    let source_path = Path::new(
        binding
            .materialized_path
            .as_deref()
            .expect("artifact binding should carry a source path for cache staging"),
    );
    let cache_path = psionic_train_resolved_artifact_cache_candidates(
        run_root,
        binding.artifact_ref.artifact_id.as_str(),
    )
    .remove(0);
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).expect("resolver cache parent should exist");
    }
    fs::copy(source_path, &cache_path).expect("artifact should copy into the resolver cache");
    cache_path
}

fn cache_contribution_family(
    run_root: &Path,
    contribution_receipt_path: &Path,
    contribution_artifact_manifest_path: &Path,
) -> PsionicTrainContributionArtifactManifest {
    cache_artifact_binding(run_root, &artifact_binding(contribution_receipt_path));
    cache_artifact_binding(
        run_root,
        &artifact_binding(contribution_artifact_manifest_path),
    );
    let contribution_artifact_manifest: PsionicTrainContributionArtifactManifest =
        parse_json(contribution_artifact_manifest_path);
    for artifact in &contribution_artifact_manifest.artifacts {
        cache_artifact_binding(run_root, &artifact.binding);
    }
    contribution_artifact_manifest
}

fn remove_path_if_present(path: &Path) {
    if path.is_file() {
        fs::remove_file(path).expect("artifact file should remove");
    }
}

fn remove_contribution_family(
    contribution_receipt_path: &Path,
    contribution_artifact_manifest_path: &Path,
    contribution_artifact_manifest: &PsionicTrainContributionArtifactManifest,
) {
    remove_path_if_present(contribution_receipt_path);
    remove_path_if_present(contribution_artifact_manifest_path);
    for artifact in &contribution_artifact_manifest.artifacts {
        if let Some(path) = artifact.binding.materialized_path.as_deref() {
            remove_path_if_present(Path::new(path));
        }
    }
}

fn dirty_tree_build_inputs() -> (String, Option<String>) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let porcelain = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status --porcelain should run");
    assert!(
        porcelain.status.success(),
        "git status --porcelain should succeed"
    );
    if porcelain.stdout.is_empty() {
        return (String::from("refuse_by_default"), None);
    }
    let status_snapshot = Command::new("git")
        .arg("-C")
        .arg(&repo_root)
        .args(["status", "--short", "--branch"])
        .output()
        .expect("git status --short --branch should run");
    assert!(
        status_snapshot.status.success(),
        "git status --short --branch should succeed"
    );
    let status_snapshot = String::from_utf8(status_snapshot.stdout)
        .expect("git status snapshot should be utf8")
        .trim()
        .to_string();
    let mut digest = Sha256::new();
    digest.update(status_snapshot.as_bytes());
    (
        String::from("allowed_by_operator_override"),
        Some(format!("{:x}", digest.finalize())),
    )
}

fn admitted_identity_for_lane(
    git_commit_sha: &str,
    lane_id: &str,
) -> PsionicTrainAdmissionIdentity {
    let (dirty_tree_admission, workspace_status_sha256) = dirty_tree_build_inputs();
    let (release_id, environment_ref) = match lane_id {
        psionic_train::PSION_ACTUAL_PRETRAINING_LANE_ID => (
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
        ),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID => (
            PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID,
            PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
        ),
        PSION_CS336_A1_DEMO_LANE_ID => (
            PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID,
            PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF,
        ),
        other => panic!("unexpected lane for admitted identity: {other}"),
    };
    PsionicTrainAdmissionIdentity {
        release_id: String::from(release_id),
        build_digest: runtime_build_digest(
            release_id,
            PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
            lane_id,
            git_commit_sha,
            dirty_tree_admission.as_str(),
            workspace_status_sha256.as_deref(),
            environment_ref,
        ),
        environment_ref: String::from(environment_ref),
    }
}

fn base_manifest_for_lane(lane_id: &str) -> PsionicTrainInvocationManifest {
    let git_commit_sha = git_head();
    PsionicTrainInvocationManifest {
        schema_version: String::from(PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION),
        runtime_surface_id: String::from(PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
        lane_id: String::from(lane_id),
        role: PsionicTrainRole::Worker,
        operation: PsionicTrainOperation::Start,
        work_class: default_work_class_for_lane(lane_id),
        coordination: PsionicTrainCoordinationContext {
            network_id: Some(String::from("network.psionic.cli-test")),
            window_id: None,
            assignment_id: None,
            challenge_id: None,
            node_pubkey: Some(String::from("npub1-psionic-cli-test")),
            membership_revision: None,
        },
        grouped_stage_assignment: None,
        admission_identity: admitted_identity_for_lane(git_commit_sha.as_str(), lane_id),
        run_id: Some(String::from("psion-train-cli-test")),
        output_root: None,
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

fn default_work_class_for_lane(lane_id: &str) -> PsionicTrainWorkClass {
    if lane_id == PSION_APPLE_WINDOWED_TRAINING_LANE_ID || lane_id == PSION_CS336_A1_DEMO_LANE_ID {
        PsionicTrainWorkClass::SmallModelLocalTraining
    } else {
        PsionicTrainWorkClass::FullIslandLocalUpdateTraining
    }
}

fn base_manifest() -> PsionicTrainInvocationManifest {
    base_manifest_for_lane(psionic_train::PSION_ACTUAL_PRETRAINING_LANE_ID)
}

fn add_admitted_observations(manifest: &mut PsionicTrainInvocationManifest) {
    manifest.hardware_observation_path = Some(String::from(
        "fixtures/psion/pretrain/psion_actual_pretraining_hardware_observation_admitted_v1.json",
    ));
    manifest.run_shape_observation_path = Some(String::from(
        "fixtures/psion/pretrain/psion_actual_pretraining_run_shape_observation_admitted_v1.json",
    ));
}

fn write_manifest(path: &Path, manifest: &mut PsionicTrainInvocationManifest) {
    manifest
        .populate_manifest_digest()
        .expect("manifest digest should populate");
    fs::write(
        path,
        serde_json::to_string_pretty(manifest).expect("manifest should serialize"),
    )
    .expect("manifest should write");
}

fn run_machine_manifest(manifest_path: &Path) -> Output {
    Command::new(binary_path())
        .args(["manifest", "--manifest"])
        .arg(manifest_path)
        .output()
        .expect("psionic-train should run")
}

fn parse_json<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> T {
    serde_json::from_slice(&fs::read(path.as_ref()).expect("retained artifact should be readable"))
        .expect("retained artifact should parse")
}

fn write_json<T: serde::Serialize>(path: impl AsRef<Path>, value: &T) {
    fs::write(
        path.as_ref(),
        serde_json::to_string_pretty(value).expect("artifact should serialize"),
    )
    .expect("artifact should write");
}

fn build_launch_manifest(run_root: &Path) -> PsionicTrainInvocationManifest {
    let mut manifest = base_manifest();
    manifest.output_root = Some(run_root.display().to_string());
    manifest.allow_dirty_tree = true;
    add_admitted_observations(&mut manifest);
    manifest
}

fn build_apple_launch_manifest(run_root: &Path) -> PsionicTrainInvocationManifest {
    let mut manifest = base_manifest_for_lane(PSION_APPLE_WINDOWED_TRAINING_LANE_ID);
    manifest.output_root = Some(run_root.display().to_string());
    manifest.allow_dirty_tree = true;
    manifest
}

fn build_cs336_a1_demo_launch_manifest(run_root: &Path) -> PsionicTrainInvocationManifest {
    let mut manifest = base_manifest_for_lane(PSION_CS336_A1_DEMO_LANE_ID);
    manifest.output_root = Some(run_root.display().to_string());
    manifest.allow_dirty_tree = true;
    manifest.dry_run = false;
    manifest
}

fn build_retained_operation_manifest(
    run_root: &Path,
    role: PsionicTrainRole,
    operation: PsionicTrainOperation,
) -> PsionicTrainInvocationManifest {
    let mut manifest = base_manifest_for_lane(psionic_train::PSION_ACTUAL_PRETRAINING_LANE_ID);
    manifest.role = role;
    manifest.operation = operation;
    manifest.run_id = None;
    manifest.output_root = None;
    manifest.run_root = Some(run_root.display().to_string());
    manifest.peer_node_pubkey = None;
    manifest.peer_checkpoint_handoff_receipt = None;
    manifest.allow_dirty_tree = true;
    add_admitted_observations(&mut manifest);
    manifest
}

fn build_retained_operation_manifest_for_lane(
    lane_id: &str,
    run_root: &Path,
    role: PsionicTrainRole,
    operation: PsionicTrainOperation,
) -> PsionicTrainInvocationManifest {
    let mut manifest = base_manifest_for_lane(lane_id);
    manifest.role = role;
    manifest.operation = operation;
    manifest.run_id = None;
    manifest.output_root = None;
    manifest.run_root = Some(run_root.display().to_string());
    manifest.peer_node_pubkey = None;
    manifest.peer_checkpoint_handoff_receipt = None;
    manifest.allow_dirty_tree = true;
    if lane_id == psionic_train::PSION_ACTUAL_PRETRAINING_LANE_ID {
        add_admitted_observations(&mut manifest);
    }
    manifest
}

fn bind_window_context(
    manifest: &mut PsionicTrainInvocationManifest,
    window_id: &str,
    assignment_id: &str,
    membership_revision: u64,
) {
    manifest.coordination.window_id = Some(String::from(window_id));
    manifest.coordination.assignment_id = Some(String::from(assignment_id));
    manifest.coordination.membership_revision = Some(membership_revision);
}

fn grouped_stage_assignment(
    stage_id: &str,
    stage_index: u32,
    stage_count: u32,
    stage_role: PsionicTrainGroupedReplicaStageRole,
    upstream_stage_id: Option<&str>,
    downstream_stage_id: Option<&str>,
) -> PsionicTrainGroupedReplicaStageAssignment {
    PsionicTrainGroupedReplicaStageAssignment::new(
        "replica-apple-01",
        stage_id,
        stage_index,
        stage_count,
        stage_role,
        upstream_stage_id.map(String::from),
        downstream_stage_id.map(String::from),
    )
    .expect("grouped stage assignment should build")
}

fn build_validator_manifest(
    run_root: &Path,
    contribution_receipt_path: &Path,
    contribution_artifact_manifest_path: &Path,
    window_id: &str,
    assignment_id: &str,
    challenge_id: &str,
) -> PsionicTrainInvocationManifest {
    build_validator_manifest_for_lane(
        psionic_train::PSION_ACTUAL_PRETRAINING_LANE_ID,
        run_root,
        contribution_receipt_path,
        contribution_artifact_manifest_path,
        window_id,
        assignment_id,
        challenge_id,
    )
}

fn build_validator_manifest_for_lane(
    lane_id: &str,
    run_root: &Path,
    contribution_receipt_path: &Path,
    contribution_artifact_manifest_path: &Path,
    window_id: &str,
    assignment_id: &str,
    challenge_id: &str,
) -> PsionicTrainInvocationManifest {
    let mut manifest = build_retained_operation_manifest_for_lane(
        lane_id,
        run_root,
        PsionicTrainRole::Validator,
        PsionicTrainOperation::ValidateContribution,
    );
    manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
    manifest.coordination.window_id = Some(String::from(window_id));
    manifest.coordination.assignment_id = Some(String::from(assignment_id));
    manifest.coordination.challenge_id = Some(String::from(challenge_id));
    manifest.coordination.node_pubkey = Some(String::from("npub1-psionic-validator-cli-test"));
    manifest.validator_target_contribution_receipt =
        Some(artifact_binding(contribution_receipt_path));
    manifest.validator_target_contribution_artifact_manifest =
        Some(artifact_binding(contribution_artifact_manifest_path));
    manifest.validator_target_work_class = Some(if contribution_receipt_path.is_file() {
        parse_json::<PsionicTrainContributionReceipt>(contribution_receipt_path).work_class
    } else {
        default_work_class_for_lane(lane_id)
    });
    manifest
}

fn build_apple_record_checkpoint_manifest(
    run_root: &Path,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
) -> PsionicTrainInvocationManifest {
    let mut manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    manifest.checkpoint_label = Some(String::from(checkpoint_label));
    manifest.optimizer_step = Some(optimizer_step);
    manifest.checkpoint_ref = Some(String::from(checkpoint_ref));
    manifest
}

#[test]
fn machine_manifest_dry_run_emits_success_status_packet() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("run");
    let manifest_path = tempdir.path().join("invocation.json");
    let mut manifest = build_launch_manifest(&run_root);
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    assert_eq!(packet.outcome, PsionicTrainOutcomeKind::Succeeded);
    assert_eq!(packet.run_id.as_deref(), Some("psion-train-cli-test"));
    assert_eq!(
        packet.run_root.as_deref(),
        Some(run_root.to_string_lossy().as_ref())
    );
    assert!(packet.runtime_attestation.is_some());
    assert!(packet.capability_projection.is_some());
    assert!(packet.run_status_packet_path.is_some());
    assert!(packet.window_status_packet_path.is_some());
    assert!(run_root.join("manifests/launch_manifest.json").is_file());
    assert!(run_root.join("status/current_run_status.json").is_file());
    let run_status: PsionicTrainRunStatusPacket = serde_json::from_slice(
        &fs::read(
            packet
                .run_status_packet_path
                .as_ref()
                .expect("run status packet path should exist"),
        )
        .expect("run status packet should be readable"),
    )
    .expect("run status packet should parse");
    assert_eq!(run_status.run_id.as_deref(), Some("psion-train-cli-test"));
    let checkpoint_surface_path = run_status
        .artifacts
        .checkpoint_surface_path
        .as_ref()
        .expect("checkpoint surface path should exist");
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(checkpoint_surface_path);
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("pending_first_checkpoint")
    );
    assert_eq!(
        checkpoint_surface.checkpoint_label.as_deref(),
        Some("pending_first_checkpoint")
    );
    assert_eq!(checkpoint_surface.optimizer_step, None);
    let membership_revision_path = run_status
        .artifacts
        .membership_revision_path
        .as_ref()
        .expect("membership revision path should exist");
    let membership_receipt: PsionicTrainMembershipRevisionReceipt = serde_json::from_slice(
        &fs::read(membership_revision_path)
            .expect("membership revision receipt should be readable"),
    )
    .expect("membership revision receipt should parse");
    assert_eq!(
        membership_receipt.node_pubkey.as_str(),
        "npub1-psionic-cli-test"
    );
    assert_eq!(membership_receipt.run_id.as_str(), "psion-train-cli-test");
    let window_status: PsionicTrainWindowStatusPacket = serde_json::from_slice(
        &fs::read(
            packet
                .window_status_packet_path
                .as_ref()
                .expect("window status packet path should exist"),
        )
        .expect("window status packet should be readable"),
    )
    .expect("window status packet should parse");
    assert_eq!(
        window_status.run_id.as_deref(),
        Some("psion-train-cli-test")
    );
}

#[test]
fn machine_manifest_cs336_a1_demo_writes_checkpoint_and_closeout() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("run");
    let manifest_path = tempdir.path().join("cs336-a1-demo-invocation.json");
    let mut manifest = build_cs336_a1_demo_launch_manifest(&run_root);
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    assert_eq!(packet.outcome, PsionicTrainOutcomeKind::Succeeded);
    assert_eq!(packet.lane_id.as_str(), PSION_CS336_A1_DEMO_LANE_ID);
    assert_eq!(
        packet.detail.as_str(),
        "psionic-train completed start for lane `psion_cs336_a1_demo_v1` with retained phase `completed`"
    );
    let run_status: PsionicTrainRunStatusPacket = serde_json::from_slice(
        &fs::read(
            packet
                .run_status_packet_path
                .as_ref()
                .expect("run status packet path should exist"),
        )
        .expect("run status packet should be readable"),
    )
    .expect("run status packet should parse");
    assert_eq!(run_status.phase.as_deref(), Some("completed"));
    assert_eq!(run_status.lane_id.as_str(), PSION_CS336_A1_DEMO_LANE_ID);
    assert_eq!(
        run_status.artifacts.checkpoint_pointer_path.as_deref(),
        Some(
            run_root
                .join("checkpoints/latest_accepted_checkpoint_pointer.json")
                .to_string_lossy()
                .as_ref()
        )
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_deref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        checkpoint_surface.checkpoint_label.as_deref(),
        Some("bounded_step_000004")
    );
    assert_eq!(checkpoint_surface.optimizer_step, Some(4));
    assert!(run_root.join("status/retained_summary.json").is_file());
    assert!(run_root.join("closeout/closeout_bundle.json").is_file());
}

#[test]
fn machine_manifest_record_checkpoint_persists_checkpoint_surface() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&launch_output.stdout),
        String::from_utf8_lossy(&launch_output.stderr)
    );

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let output = run_machine_manifest(&checkpoint_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    assert_eq!(packet.outcome, PsionicTrainOutcomeKind::Succeeded);

    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.current_phase.as_deref(),
        Some("checkpoint_evaluated")
    );
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        checkpoint_surface.checkpoint_label.as_deref(),
        Some("broader-pretrain-final")
    );
    assert_eq!(checkpoint_surface.optimizer_step, Some(16_384));
    assert_eq!(
        checkpoint_surface.checkpoint_ref.as_deref(),
        Some("checkpoint://psion/broad/pretrain/final")
    );
    assert!(checkpoint_surface.checkpoint_manifest_digest.is_some());
    assert!(checkpoint_surface.checkpoint_object_digest.is_some());
    assert!(checkpoint_surface.checkpoint_total_bytes.is_some());
    assert_eq!(
        checkpoint_surface.backup_state.as_deref(),
        Some("backed_up")
    );
    assert_eq!(
        checkpoint_surface.upload_outcome.as_deref(),
        Some("succeeded")
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_manifest_path
                .as_deref()
                .expect("checkpoint manifest path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_backup_receipt_path
                .as_deref()
                .expect("checkpoint backup receipt path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_backup_pointer_path
                .as_deref()
                .expect("checkpoint backup pointer path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_backup_manifest_path
                .as_deref()
                .expect("checkpoint backup manifest path should exist"),
        )
        .is_file()
    );
}

#[test]
fn apple_manifest_start_emits_metal_capability_projection() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("apple-run");
    let manifest_path = tempdir.path().join("apple-start.json");
    let mut manifest = build_apple_launch_manifest(&run_root);
    bind_window_context(
        &mut manifest,
        "apple-window-0001",
        "apple-assignment-0001",
        1,
    );
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("apple status packet should parse");
    assert_eq!(packet.outcome, PsionicTrainOutcomeKind::Succeeded);
    let capability_projection = packet
        .capability_projection
        .as_ref()
        .expect("apple capability projection should exist");
    assert_eq!(capability_projection.backend_family.as_str(), "metal");
    assert_eq!(
        capability_projection.topology_class.as_str(),
        "homogeneous_apple_silicon_data_parallel"
    );
    assert_eq!(
        packet.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );

    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("apple run status path should exist"),
    );
    assert_eq!(
        run_status.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        run_status.capability_projection.backend_family.as_str(),
        "metal"
    );
    assert!(run_status.current_status_path.is_none());
    assert!(run_status.retained_summary_path.is_none());
}

#[test]
fn apple_manifest_record_checkpoint_persists_generic_checkpoint_artifacts() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("apple-run");
    let launch_manifest_path = tempdir.path().join("apple-start.json");
    let mut launch_manifest = build_apple_launch_manifest(&run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-window-0002",
        "apple-assignment-0002",
        2,
    );
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&launch_output.stdout),
        String::from_utf8_lossy(&launch_output.stderr)
    );

    let checkpoint_manifest_path = tempdir.path().join("apple-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &run_root,
        "apple-window-final",
        2_048,
        "checkpoint://psion/apple/window/final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-window-0002",
        "apple-assignment-0002",
        2,
    );
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);

    let output = run_machine_manifest(&checkpoint_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("apple checkpoint packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("apple run status path should exist"),
    );
    let checkpoint_surface_path = run_status
        .artifacts
        .checkpoint_surface_path
        .as_ref()
        .expect("apple checkpoint surface path should exist");
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(checkpoint_surface_path);
    assert_eq!(
        checkpoint_surface.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted")
    );
    assert_eq!(checkpoint_surface.optimizer_step, Some(2_048));
    assert_eq!(
        checkpoint_surface.checkpoint_ref.as_deref(),
        Some("checkpoint://psion/apple/window/final")
    );

    let checkpoint_pointer: PsionicTrainCheckpointPointer = parse_json(
        checkpoint_surface
            .artifacts
            .checkpoint_pointer_path
            .as_ref()
            .expect("apple checkpoint pointer path should exist"),
    );
    assert_eq!(
        checkpoint_pointer.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(checkpoint_pointer.optimizer_step, 2_048);

    let checkpoint_manifest: PsionicTrainCheckpointManifest = parse_json(
        checkpoint_surface
            .artifacts
            .checkpoint_manifest_path
            .as_ref()
            .expect("apple checkpoint manifest path should exist"),
    );
    assert_eq!(
        checkpoint_manifest.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        checkpoint_manifest.manifest_digest,
        checkpoint_manifest.stable_manifest_digest()
    );
}

#[test]
fn apple_manifest_resume_refuses_without_any_admitted_checkpoint() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("apple-run");

    let launch_manifest_path = tempdir.path().join("apple-start.json");
    let mut launch_manifest = build_apple_launch_manifest(&run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "apple launch should succeed"
    );

    let resume_manifest_path = tempdir.path().join("apple-resume.json");
    let mut resume_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(!output.status.success(), "apple resume should be refused");
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("apple refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::CheckpointMissing)
    );
}

#[test]
fn apple_manifest_serve_checkpoint_retains_primary_handoff_receipt() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("apple-source-run");

    let launch_manifest_path = tempdir.path().join("apple-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&source_run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-window-serve-0001",
        "apple-assignment-serve-0001",
        1,
    );
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "apple launch should succeed"
    );

    let checkpoint_manifest_path = tempdir.path().join("apple-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &source_run_root,
        "apple-serve-final",
        3_072,
        "checkpoint://psion/apple/serve/final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-window-serve-0001",
        "apple-assignment-serve-0001",
        2,
    );
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "apple record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("apple-serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-apple-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let output = run_machine_manifest(&serve_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("apple serve packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("apple serve run status path should exist"),
    );
    let handoff_receipt: PsionicTrainCheckpointHandoffReceipt = parse_json(
        run_status
            .artifacts
            .checkpoint_handoff_receipt_path
            .as_ref()
            .expect("apple handoff receipt path should exist"),
    );
    assert_eq!(
        handoff_receipt.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        handoff_receipt.source_kind,
        PsionicTrainCheckpointHandoffSourceKind::LivePrimaryPointer
    );
    assert_eq!(handoff_receipt.optimizer_step, 3_072);
    assert_eq!(
        handoff_receipt.peer_node_pubkey.as_str(),
        "npub1-apple-late-joiner"
    );
}

#[test]
fn apple_manifest_resume_can_seed_from_peer_checkpoint_handoff() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("apple-source-run");

    let launch_manifest_path = tempdir.path().join("apple-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&source_run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-window-resume-0001",
        "apple-assignment-resume-0001",
        1,
    );
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "apple launch should succeed"
    );

    let checkpoint_manifest_path = tempdir.path().join("apple-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &source_run_root,
        "apple-resume-final",
        4_096,
        "checkpoint://psion/apple/resume/final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-window-resume-0001",
        "apple-assignment-resume-0001",
        2,
    );
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "apple record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("apple-serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-apple-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let serve_output = run_machine_manifest(&serve_manifest_path);
    assert!(
        serve_output.status.success(),
        "apple serve-checkpoint should succeed"
    );
    let serve_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&serve_output.stdout).expect("apple serve packet should parse");
    let serve_run_status: PsionicTrainRunStatusPacket = parse_json(
        serve_packet
            .run_status_packet_path
            .as_ref()
            .expect("apple serve run status path should exist"),
    );
    let handoff_receipt_path = serve_run_status
        .artifacts
        .checkpoint_handoff_receipt_path
        .as_ref()
        .expect("apple handoff receipt path should exist")
        .clone();

    let joiner_run_root = tempdir.path().join("apple-joiner-run");
    let resume_manifest_path = tempdir.path().join("apple-resume-from-peer.json");
    let mut resume_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &joiner_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.coordination.node_pubkey = Some(String::from("npub1-apple-late-joiner"));
    resume_manifest.peer_checkpoint_handoff_receipt =
        Some(artifact_binding(Path::new(handoff_receipt_path.as_str())));
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("apple resume packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("apple resume run status path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("apple checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        checkpoint_surface.checkpoint_label.as_deref(),
        Some("apple-resume-final")
    );
    assert_eq!(checkpoint_surface.optimizer_step, Some(4_096));
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_pointer_path
                .as_deref()
                .expect("apple checkpoint pointer path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .peer_checkpoint_handoff_receipt_path
                .as_deref()
                .expect("apple handoff receipt path should exist"),
        )
        .is_file()
    );
}

#[test]
fn apple_grouped_stage_record_checkpoint_persists_grouped_checkpoint_surface() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("apple-grouped-run");

    let launch_manifest_path = tempdir.path().join("apple-grouped-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-grouped-window-0001",
        "apple-grouped-assignment-0001",
        1,
    );
    launch_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    launch_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "grouped apple launch should succeed"
    );

    let checkpoint_manifest_path = tempdir.path().join("apple-grouped-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &run_root,
        "apple-grouped-final",
        5_120,
        "checkpoint://psion/apple/grouped/final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-grouped-window-0001",
        "apple-grouped-assignment-0001",
        2,
    );
    checkpoint_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    checkpoint_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let output = run_machine_manifest(&checkpoint_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("grouped checkpoint packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("grouped run status path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("grouped checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.window_id.as_deref(),
        Some("apple-grouped-window-0001")
    );
    assert_eq!(
        checkpoint_surface.assignment_id.as_deref(),
        Some("apple-grouped-assignment-0001")
    );
    assert_eq!(
        checkpoint_surface
            .grouped_stage_assignment
            .as_ref()
            .expect("grouped checkpoint surface should carry stage assignment")
            .stage_id
            .as_str(),
        "stage-01"
    );

    let checkpoint_pointer: PsionicTrainCheckpointPointer = parse_json(
        checkpoint_surface
            .artifacts
            .checkpoint_pointer_path
            .as_ref()
            .expect("grouped checkpoint pointer path should exist"),
    );
    assert_eq!(
        checkpoint_pointer.window_id.as_deref(),
        Some("apple-grouped-window-0001")
    );
    assert_eq!(
        checkpoint_pointer.assignment_id.as_deref(),
        Some("apple-grouped-assignment-0001")
    );
    assert_eq!(
        checkpoint_pointer
            .grouped_stage_assignment
            .as_ref()
            .expect("grouped checkpoint pointer should carry stage assignment")
            .stage_id
            .as_str(),
        "stage-01"
    );

    let checkpoint_manifest: PsionicTrainCheckpointManifest = parse_json(
        checkpoint_surface
            .artifacts
            .checkpoint_manifest_path
            .as_ref()
            .expect("grouped checkpoint manifest path should exist"),
    );
    assert_eq!(
        checkpoint_manifest.window_id.as_deref(),
        Some("apple-grouped-window-0001")
    );
    assert_eq!(
        checkpoint_manifest.assignment_id.as_deref(),
        Some("apple-grouped-assignment-0001")
    );
    assert_eq!(
        checkpoint_manifest
            .grouped_stage_assignment
            .as_ref()
            .expect("grouped checkpoint manifest should carry stage assignment")
            .stage_id
            .as_str(),
        "stage-01"
    );

    let contribution_artifact_manifest: PsionicTrainContributionArtifactManifest = parse_json(
        run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_ref()
            .expect("grouped contribution artifact manifest path should exist"),
    );
    assert!(
        contribution_artifact_manifest
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_kind == "checkpoint_pointer")
    );
    assert!(
        contribution_artifact_manifest
            .artifacts
            .iter()
            .any(|artifact| artifact.artifact_kind == "checkpoint_manifest")
    );
}

#[test]
fn apple_grouped_stage_resume_can_seed_from_peer_checkpoint_handoff() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("apple-grouped-source-run");

    let launch_manifest_path = tempdir.path().join("apple-grouped-source-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&source_run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-grouped-window-0002",
        "apple-grouped-assignment-0002",
        1,
    );
    launch_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    launch_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "grouped source launch should succeed"
    );

    let checkpoint_manifest_path = tempdir
        .path()
        .join("apple-grouped-source-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &source_run_root,
        "apple-grouped-peer-final",
        6_144,
        "checkpoint://psion/apple/grouped/peer-final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-grouped-window-0002",
        "apple-grouped-assignment-0002",
        2,
    );
    checkpoint_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    checkpoint_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "grouped source record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("apple-grouped-serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-apple-grouped-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let serve_output = run_machine_manifest(&serve_manifest_path);
    assert!(
        serve_output.status.success(),
        "grouped serve-checkpoint should succeed"
    );
    let serve_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&serve_output.stdout).expect("grouped serve packet should parse");
    let serve_run_status: PsionicTrainRunStatusPacket = parse_json(
        serve_packet
            .run_status_packet_path
            .as_ref()
            .expect("grouped serve run status path should exist"),
    );
    let handoff_receipt: PsionicTrainCheckpointHandoffReceipt = parse_json(
        serve_run_status
            .artifacts
            .checkpoint_handoff_receipt_path
            .as_ref()
            .expect("grouped handoff receipt path should exist"),
    );
    assert_eq!(
        handoff_receipt
            .grouped_stage_assignment
            .as_ref()
            .expect("grouped handoff receipt should carry stage assignment")
            .stage_id
            .as_str(),
        "stage-01"
    );

    let joiner_run_root = tempdir.path().join("apple-grouped-joiner-run");
    let resume_manifest_path = tempdir.path().join("apple-grouped-resume-from-peer.json");
    let mut resume_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &joiner_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    bind_window_context(
        &mut resume_manifest,
        "apple-grouped-window-0002",
        "apple-grouped-assignment-0002",
        2,
    );
    resume_manifest.coordination.node_pubkey = Some(String::from("npub1-apple-grouped-joiner"));
    resume_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    resume_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    resume_manifest.peer_checkpoint_handoff_receipt = serve_run_status
        .artifacts
        .checkpoint_handoff_receipt_path
        .as_deref()
        .map(Path::new)
        .map(artifact_binding);
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("grouped resume packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("grouped resume run status path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("grouped resume checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.recovery_resolution_state.as_deref(),
        Some("accepted_peer_handoff_checkpoint")
    );
    assert_eq!(
        checkpoint_surface.recovery_source_kind.as_deref(),
        Some("peer_checkpoint_handoff")
    );
    assert_eq!(checkpoint_surface.restored_primary_pointer, Some(true));
    assert_eq!(
        checkpoint_surface
            .grouped_stage_assignment
            .as_ref()
            .expect("grouped checkpoint surface should keep stage assignment")
            .stage_id
            .as_str(),
        "stage-01"
    );

    let grouped_recovery_receipt: PsionicTrainGroupedReplicaStageRecoveryReceipt = parse_json(
        run_status
            .artifacts
            .recovery_receipt_path
            .as_ref()
            .expect("grouped recovery receipt path should exist"),
    );
    assert_eq!(
        grouped_recovery_receipt.recovery_source_kind,
        PsionicTrainGroupedReplicaRecoverySourceKind::PeerCheckpointHandoff
    );
    assert_eq!(
        grouped_recovery_receipt
            .grouped_stage_assignment
            .stage_id
            .as_str(),
        "stage-01"
    );
}

#[test]
fn apple_grouped_stage_resume_refuses_mismatched_stage_checkpoint() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("apple-grouped-mismatch-run");

    let launch_manifest_path = tempdir.path().join("apple-grouped-mismatch-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-grouped-window-0003",
        "apple-grouped-assignment-0003",
        1,
    );
    launch_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    launch_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "grouped mismatch launch should succeed"
    );

    let checkpoint_manifest_path = tempdir
        .path()
        .join("apple-grouped-mismatch-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &run_root,
        "apple-grouped-mismatch-final",
        7_168,
        "checkpoint://psion/apple/grouped/mismatch-final",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-grouped-window-0003",
        "apple-grouped-assignment-0003",
        2,
    );
    checkpoint_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    checkpoint_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "grouped mismatch record-checkpoint should succeed"
    );

    let resume_manifest_path = tempdir.path().join("apple-grouped-mismatch-resume.json");
    let mut resume_manifest = build_retained_operation_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    bind_window_context(
        &mut resume_manifest,
        "apple-grouped-window-0003",
        "apple-grouped-assignment-0003",
        2,
    );
    resume_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    resume_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-02",
        1,
        2,
        PsionicTrainGroupedReplicaStageRole::Egress,
        Some("stage-01"),
        None,
    ));
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        !output.status.success(),
        "grouped mismatch resume should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("grouped mismatch refusal should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::StaleAssignment)
    );
}

#[test]
fn machine_manifest_resume_recovers_primary_pointer_from_backup() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "record-checkpoint should succeed"
    );

    let primary_pointer_path = run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    fs::remove_file(&primary_pointer_path).expect("primary pointer should be removable");
    assert!(
        !primary_pointer_path.is_file(),
        "primary pointer should be gone"
    );

    let resume_manifest_path = tempdir.path().join("resume.json");
    let mut resume_manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.recovery_resolution_state.as_deref(),
        Some("recovered_from_backup")
    );
    assert_eq!(
        checkpoint_surface.recovery_source_kind.as_deref(),
        Some("backup_receipt")
    );
    assert_eq!(checkpoint_surface.restored_primary_pointer, Some(true));
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_pointer_path
                .as_deref()
                .expect("checkpoint pointer path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .auto_resume_receipt_path
                .as_deref()
                .expect("auto-resume receipt path should exist"),
        )
        .is_file()
    );
}

#[test]
fn machine_manifest_resume_refuses_without_any_admitted_checkpoint() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let resume_manifest_path = tempdir.path().join("resume.json");
    let mut resume_manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(!output.status.success(), "resume should be refused");
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::CheckpointMissing)
    );
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.recovery_resolution_state.as_deref(),
        Some("refused")
    );
    assert_eq!(
        checkpoint_surface.recovery_source_kind.as_deref(),
        Some("none")
    );
    assert_eq!(checkpoint_surface.restored_primary_pointer, Some(false));
}

#[test]
fn machine_manifest_window_context_emits_window_and_contribution_artifacts() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("windowed-run");
    let manifest_path = tempdir.path().join("windowed-launch.json");
    let mut manifest = build_launch_manifest(&run_root);
    bind_window_context(&mut manifest, "window-0001", "assignment-0001", 1);
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    assert!(run_status.artifacts.window_execution_path.is_some());
    assert!(run_status.artifacts.contribution_receipt_path.is_some());
    assert!(
        run_status
            .artifacts
            .contribution_artifact_manifest_path
            .is_some()
    );
    assert!(run_status.artifacts.sealed_window_bundle_path.is_some());

    let window_execution: PsionicTrainWindowExecution = parse_json(
        run_status
            .artifacts
            .window_execution_path
            .as_ref()
            .expect("window execution path should exist"),
    );
    assert_eq!(window_execution.window_id.as_str(), "window-0001");
    assert_eq!(
        window_execution.current_assignment.assignment_id.as_str(),
        "assignment-0001"
    );

    let sealed_window: PsionicTrainSealedWindowBundle = parse_json(
        run_status
            .artifacts
            .sealed_window_bundle_path
            .as_ref()
            .expect("sealed window bundle path should exist"),
    );
    assert_eq!(sealed_window.window_id.as_str(), "window-0001");
    assert_eq!(sealed_window.contribution_count, 1);
    assert_eq!(sealed_window.artifact_manifest_count, 1);
    assert_eq!(sealed_window.contributions.len(), 1);
}

#[test]
fn machine_manifest_second_assignment_updates_sealed_window_rollup() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("windowed-run");

    let launch_manifest_path = tempdir.path().join("windowed-launch.json");
    let mut launch_manifest = build_launch_manifest(&run_root);
    bind_window_context(&mut launch_manifest, "window-0001", "assignment-0001", 1);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("windowed-record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "window-0001",
        "assignment-0002",
        2,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let output = run_machine_manifest(&checkpoint_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let sealed_window: PsionicTrainSealedWindowBundle = parse_json(
        run_status
            .artifacts
            .sealed_window_bundle_path
            .as_ref()
            .expect("sealed window bundle path should exist"),
    );
    assert_eq!(sealed_window.window_id.as_str(), "window-0001");
    assert_eq!(sealed_window.contribution_count, 2);
    assert_eq!(sealed_window.artifact_manifest_count, 2);
    assert_eq!(sealed_window.contributions.len(), 2);
    assert_eq!(
        sealed_window.contributions[0].assignment_id.as_str(),
        "assignment-0001"
    );
    assert_eq!(
        sealed_window.contributions[1].assignment_id.as_str(),
        "assignment-0002"
    );
}

#[test]
fn machine_manifest_serve_checkpoint_retains_primary_handoff_receipt() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("source-run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&source_run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let output = run_machine_manifest(&serve_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let handoff_receipt: PsionicTrainCheckpointHandoffReceipt = parse_json(
        run_status
            .artifacts
            .checkpoint_handoff_receipt_path
            .as_ref()
            .expect("checkpoint handoff receipt path should exist"),
    );
    assert_eq!(
        handoff_receipt.source_kind,
        PsionicTrainCheckpointHandoffSourceKind::LivePrimaryPointer
    );
    assert_eq!(
        handoff_receipt.peer_node_pubkey.as_str(),
        "npub1-late-joiner"
    );
    assert_eq!(
        handoff_receipt.serving_node_pubkey.as_str(),
        "npub1-psionic-cli-test"
    );
    assert_eq!(handoff_receipt.optimizer_step, 16_384);
}

#[test]
fn machine_manifest_resume_can_seed_from_peer_checkpoint_handoff() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("source-run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&source_run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let serve_output = run_machine_manifest(&serve_manifest_path);
    assert!(
        serve_output.status.success(),
        "serve-checkpoint should succeed"
    );
    let serve_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&serve_output.stdout).expect("status packet should parse");
    let serve_run_status: PsionicTrainRunStatusPacket = parse_json(
        serve_packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let handoff_receipt_path = serve_run_status
        .artifacts
        .checkpoint_handoff_receipt_path
        .as_ref()
        .expect("checkpoint handoff receipt path should exist")
        .clone();

    let joiner_run_root = tempdir.path().join("joiner-run");
    let resume_manifest_path = tempdir.path().join("resume-from-peer.json");
    let mut resume_manifest = build_retained_operation_manifest(
        &joiner_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.coordination.node_pubkey = Some(String::from("npub1-late-joiner"));
    resume_manifest.peer_checkpoint_handoff_receipt =
        Some(artifact_binding(Path::new(handoff_receipt_path.as_str())));
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    assert_eq!(run_status.run_id.as_deref(), Some("psion-train-cli-test"));
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.pointer_state.as_deref(),
        Some("accepted")
    );
    assert_eq!(
        checkpoint_surface.recovery_resolution_state.as_deref(),
        Some("accepted_primary_pointer")
    );
    assert!(
        Path::new(
            run_status
                .artifacts
                .checkpoint_handoff_receipt_path
                .as_deref()
                .expect("joiner checkpoint handoff receipt path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_pointer_path
                .as_deref()
                .expect("checkpoint pointer path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_manifest_path
                .as_deref()
                .expect("checkpoint manifest path should exist"),
        )
        .is_file()
    );
}

#[test]
fn machine_manifest_resume_can_seed_from_resolver_backed_peer_checkpoint_handoff() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("source-run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&source_run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "record-checkpoint should succeed"
    );

    let serve_manifest_path = tempdir.path().join("serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let serve_output = run_machine_manifest(&serve_manifest_path);
    assert!(
        serve_output.status.success(),
        "serve-checkpoint should succeed"
    );
    let serve_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&serve_output.stdout).expect("status packet should parse");
    let serve_run_status: PsionicTrainRunStatusPacket = parse_json(
        serve_packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let handoff_receipt_path = serve_run_status
        .artifacts
        .checkpoint_handoff_receipt_path
        .as_ref()
        .expect("checkpoint handoff receipt path should exist")
        .clone();
    let handoff_receipt: PsionicTrainCheckpointHandoffReceipt =
        parse_json(Path::new(handoff_receipt_path.as_str()));
    let original_handoff_binding = artifact_binding(Path::new(handoff_receipt_path.as_str()));

    let joiner_run_root = tempdir.path().join("joiner-run");
    let joiner_cache_root = joiner_run_root.join("artifacts/resolved");
    fs::create_dir_all(&joiner_cache_root).expect("joiner cache root should exist");

    let cached_handoff_receipt_path = psionic_train_resolved_artifact_cache_candidates(
        &joiner_run_root,
        original_handoff_binding.artifact_ref.artifact_id.as_str(),
    )
    .remove(0);
    fs::copy(&handoff_receipt_path, &cached_handoff_receipt_path)
        .expect("handoff receipt should copy into the resolver cache");

    let source_pointer_path = handoff_receipt
        .source_checkpoint_pointer
        .materialized_path
        .as_ref()
        .expect("source checkpoint pointer path should exist")
        .clone();
    let cached_pointer_path = psionic_train_resolved_artifact_cache_candidates(
        &joiner_run_root,
        handoff_receipt
            .source_checkpoint_pointer
            .artifact_ref
            .artifact_id
            .as_str(),
    )
    .remove(0);
    fs::copy(&source_pointer_path, &cached_pointer_path)
        .expect("checkpoint pointer should copy into the resolver cache");

    let source_manifest_path = handoff_receipt
        .source_checkpoint_manifest
        .materialized_path
        .as_ref()
        .expect("source checkpoint manifest path should exist")
        .clone();
    let cached_manifest_path = psionic_train_resolved_artifact_cache_candidates(
        &joiner_run_root,
        handoff_receipt
            .source_checkpoint_manifest
            .artifact_ref
            .artifact_id
            .as_str(),
    )
    .remove(0);
    fs::copy(&source_manifest_path, &cached_manifest_path)
        .expect("checkpoint manifest should copy into the resolver cache");

    let mut resolver_backed_binding = original_handoff_binding;
    resolver_backed_binding.materialized_path = None;

    let resume_manifest_path = tempdir.path().join("resume-from-resolver-cache.json");
    let mut resume_manifest = build_retained_operation_manifest(
        &joiner_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.coordination.node_pubkey = Some(String::from("npub1-late-joiner"));
    resume_manifest.peer_checkpoint_handoff_receipt = Some(resolver_backed_binding);
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        run_status
            .artifacts
            .checkpoint_surface_path
            .as_ref()
            .expect("checkpoint surface path should exist"),
    );
    assert_eq!(
        checkpoint_surface.recovery_resolution_state.as_deref(),
        Some("accepted_primary_pointer")
    );
    assert!(
        Path::new(
            run_status
                .artifacts
                .checkpoint_handoff_receipt_path
                .as_deref()
                .expect("joiner checkpoint handoff receipt path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_pointer_path
                .as_deref()
                .expect("checkpoint pointer path should exist"),
        )
        .is_file()
    );
    assert!(
        Path::new(
            checkpoint_surface
                .artifacts
                .checkpoint_manifest_path
                .as_deref()
                .expect("checkpoint manifest path should exist"),
        )
        .is_file()
    );
}

#[test]
fn machine_manifest_resume_refuses_with_cache_guidance_when_resolver_artifact_is_missing() {
    let tempdir = tempdir().expect("tempdir should exist");
    let joiner_run_root = tempdir.path().join("joiner-run");
    let resume_manifest_path = tempdir.path().join("resume-missing-resolver-artifact.json");
    let mut resume_manifest = build_retained_operation_manifest(
        &joiner_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::Resume,
    );
    resume_manifest.coordination.node_pubkey = Some(String::from("npub1-late-joiner"));
    resume_manifest.peer_checkpoint_handoff_receipt =
        Some(psionic_train::PsionicTrainArtifactBinding {
            artifact_ref: psionic_train::PsionicTrainArtifactRef {
                artifact_id: String::from(
                    "psionic.train.artifact.checkpoint_handoff_receipt.missing",
                ),
                artifact_digest: None,
                artifact_bytes: None,
            },
            materialized_path: None,
        });
    resume_manifest.dry_run = true;
    write_manifest(&resume_manifest_path, &mut resume_manifest);
    let output = run_machine_manifest(&resume_manifest_path);

    assert!(!output.status.success(), "resume should be refused");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("artifacts/resolved"),
        "stderr should point at the canonical resolver cache path:\n{}",
        stderr
    );
    assert!(
        stderr.contains("checkpoint_missing"),
        "stderr should preserve the checkpoint_missing refusal class:\n{}",
        stderr
    );
}

#[test]
fn machine_manifest_serve_checkpoint_falls_back_to_backup_when_primary_is_missing() {
    let tempdir = tempdir().expect("tempdir should exist");
    let source_run_root = tempdir.path().join("source-run");

    let launch_manifest_path = tempdir.path().join("launch.json");
    let mut launch_manifest = build_launch_manifest(&source_run_root);
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(launch_output.status.success(), "launch should succeed");

    let checkpoint_manifest_path = tempdir.path().join("record-checkpoint.json");
    let mut checkpoint_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    checkpoint_manifest.checkpoint_label = Some(String::from("broader-pretrain-final"));
    checkpoint_manifest.optimizer_step = Some(16_384);
    checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/broad/pretrain/final"));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "record-checkpoint should succeed"
    );

    fs::remove_file(source_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"))
        .expect("primary pointer should be removable");

    let serve_manifest_path = tempdir.path().join("serve-checkpoint.json");
    let mut serve_manifest = build_retained_operation_manifest(
        &source_run_root,
        PsionicTrainRole::RecoverySource,
        PsionicTrainOperation::ServeCheckpoint,
    );
    serve_manifest.peer_node_pubkey = Some(String::from("npub1-late-joiner"));
    write_manifest(&serve_manifest_path, &mut serve_manifest);
    let output = run_machine_manifest(&serve_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("status packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("run status packet path should exist"),
    );
    let handoff_receipt: PsionicTrainCheckpointHandoffReceipt = parse_json(
        run_status
            .artifacts
            .checkpoint_handoff_receipt_path
            .as_ref()
            .expect("checkpoint handoff receipt path should exist"),
    );
    assert_eq!(
        handoff_receipt.source_kind,
        PsionicTrainCheckpointHandoffSourceKind::DurableBackupCopy
    );
    assert!(handoff_receipt.restored_from_backup);
}

#[test]
fn validator_manifest_emits_accepted_score_receipt_for_valid_contribution() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("worker-run");
    let worker_launch_manifest_path = tempdir.path().join("worker-windowed-launch.json");
    let mut worker_launch_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut worker_launch_manifest,
        "window-0001",
        "assignment-0001",
        1,
    );
    write_manifest(&worker_launch_manifest_path, &mut worker_launch_manifest);
    let worker_launch_output = run_machine_manifest(&worker_launch_manifest_path);
    assert!(
        worker_launch_output.status.success(),
        "worker launch should succeed"
    );

    let worker_checkpoint_manifest_path = tempdir.path().join("worker-record-checkpoint.json");
    let mut worker_checkpoint_manifest = build_retained_operation_manifest(
        &worker_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    bind_window_context(
        &mut worker_checkpoint_manifest,
        "window-0001",
        "assignment-0001",
        2,
    );
    worker_checkpoint_manifest.checkpoint_label = Some(String::from("validator-target"));
    worker_checkpoint_manifest.optimizer_step = Some(4_096);
    worker_checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/validator/target"));
    write_manifest(
        &worker_checkpoint_manifest_path,
        &mut worker_checkpoint_manifest,
    );
    let worker_output = run_machine_manifest(&worker_checkpoint_manifest_path);
    assert!(
        worker_output.status.success(),
        "worker checkpoint should succeed"
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stdout).expect("worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );

    let validator_run_root = tempdir.path().join("validator-run");
    let manifest_path = tempdir.path().join("validator-invocation.json");
    let mut manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("worker contribution receipt path should exist"),
        ),
        Path::new(
            worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("worker contribution artifact manifest path should exist"),
        ),
        "window-0001",
        "assignment-0001",
        "challenge-0001",
    );
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("validator packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("validator run status path should exist"),
    );
    let score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("validator score receipt path should exist"),
    );
    assert_eq!(
        score_receipt.disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(score_receipt.score_bps, 10_000);
    assert_eq!(
        score_receipt.validator_work_class,
        PsionicTrainWorkClass::ValidationReplay
    );
    assert_eq!(
        score_receipt.challenged_work_class,
        PsionicTrainWorkClass::FullIslandLocalUpdateTraining
    );
    assert!(
        score_receipt
            .verified_hooks
            .contains(&PsionicTrainValidatorHook::CheckpointLineage)
    );

    let score_artifact: PsionicTrainValidatorScoreArtifact =
        parse_json(&score_receipt.score_artifact_path);
    assert_eq!(
        score_artifact.disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(score_artifact.challenge_id.as_str(), "challenge-0001");
    assert_eq!(
        score_artifact.challenged_work_class,
        PsionicTrainWorkClass::FullIslandLocalUpdateTraining
    );
}

#[test]
fn validator_manifest_can_replay_from_resolver_backed_artifact_ids() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("resolver-worker-run");
    let worker_launch_manifest_path = tempdir.path().join("resolver-worker-windowed-launch.json");
    let mut worker_launch_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut worker_launch_manifest,
        "window-0005",
        "assignment-0005",
        1,
    );
    write_manifest(&worker_launch_manifest_path, &mut worker_launch_manifest);
    let worker_launch_output = run_machine_manifest(&worker_launch_manifest_path);
    assert!(
        worker_launch_output.status.success(),
        "worker launch should succeed"
    );

    let worker_checkpoint_manifest_path = tempdir
        .path()
        .join("resolver-worker-record-checkpoint.json");
    let mut worker_checkpoint_manifest = build_retained_operation_manifest(
        &worker_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    bind_window_context(
        &mut worker_checkpoint_manifest,
        "window-0005",
        "assignment-0005",
        2,
    );
    worker_checkpoint_manifest.checkpoint_label = Some(String::from("resolver-validator-target"));
    worker_checkpoint_manifest.optimizer_step = Some(8_192);
    worker_checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/resolver/validator/target"));
    write_manifest(
        &worker_checkpoint_manifest_path,
        &mut worker_checkpoint_manifest,
    );
    let worker_output = run_machine_manifest(&worker_checkpoint_manifest_path);
    assert!(
        worker_output.status.success(),
        "worker checkpoint should succeed"
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stdout).expect("worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );
    let contribution_receipt_path = PathBuf::from(
        worker_run_status
            .artifacts
            .contribution_receipt_path
            .as_deref()
            .expect("worker contribution receipt path should exist"),
    );
    let contribution_artifact_manifest_path = PathBuf::from(
        worker_run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_deref()
            .expect("worker contribution artifact manifest path should exist"),
    );

    let validator_run_root = tempdir.path().join("resolver-validator-run");
    let manifest_path = tempdir.path().join("resolver-validator-invocation.json");
    let mut manifest = build_validator_manifest(
        &validator_run_root,
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
        "window-0005",
        "assignment-0005",
        "challenge-0005",
    );
    manifest
        .validator_target_contribution_receipt
        .as_mut()
        .expect("validator receipt binding should exist")
        .materialized_path = None;
    manifest
        .validator_target_contribution_artifact_manifest
        .as_mut()
        .expect("validator artifact manifest binding should exist")
        .materialized_path = None;
    let cached_artifact_manifest = cache_contribution_family(
        &validator_run_root,
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
    );
    remove_contribution_family(
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
        &cached_artifact_manifest,
    );
    assert!(
        !contribution_receipt_path.is_file(),
        "source contribution receipt should be removed to force cache-backed replay"
    );
    let checkpoint_surface_path = PathBuf::from(
        cached_artifact_manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact_kind == "checkpoint_surface")
            .and_then(|artifact| artifact.binding.materialized_path.as_ref())
            .expect("checkpoint surface path should exist"),
    );
    assert!(
        !checkpoint_surface_path.is_file(),
        "checkpoint surface should be removed to force nested artifact rematerialization"
    );

    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("validator packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("validator run status path should exist"),
    );
    let score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("validator score receipt path should exist"),
    );
    assert_eq!(
        score_receipt.disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(
        score_receipt.challenged_work_class,
        PsionicTrainWorkClass::FullIslandLocalUpdateTraining
    );
    assert!(
        checkpoint_surface_path.is_file(),
        "validator replay should restore nested checkpoint evidence from the resolver cache"
    );
}

#[test]
fn apple_validator_manifest_emits_accepted_score_receipt_for_valid_contribution() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("apple-worker-run");
    let worker_launch_manifest_path = tempdir.path().join("apple-worker-windowed-launch.json");
    let mut worker_launch_manifest = build_apple_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut worker_launch_manifest,
        "apple-window-validate-0001",
        "apple-assignment-validate-0001",
        1,
    );
    write_manifest(&worker_launch_manifest_path, &mut worker_launch_manifest);
    let worker_launch_output = run_machine_manifest(&worker_launch_manifest_path);
    assert!(
        worker_launch_output.status.success(),
        "apple worker launch should succeed"
    );

    let worker_checkpoint_manifest_path =
        tempdir.path().join("apple-worker-record-checkpoint.json");
    let mut worker_checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &worker_run_root,
        "apple-validator-target",
        4_096,
        "checkpoint://psion/apple/validator/target",
    );
    bind_window_context(
        &mut worker_checkpoint_manifest,
        "apple-window-validate-0001",
        "apple-assignment-validate-0001",
        2,
    );
    write_manifest(
        &worker_checkpoint_manifest_path,
        &mut worker_checkpoint_manifest,
    );
    let worker_output = run_machine_manifest(&worker_checkpoint_manifest_path);
    assert!(
        worker_output.status.success(),
        "apple worker checkpoint should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&worker_output.stdout),
        String::from_utf8_lossy(&worker_output.stderr)
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stdout).expect("apple worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("apple worker run status path should exist"),
    );

    let validator_run_root = tempdir.path().join("apple-validator-run");
    let manifest_path = tempdir.path().join("apple-validator-invocation.json");
    let mut manifest = build_validator_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("apple worker contribution receipt path should exist"),
        ),
        Path::new(
            worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("apple worker contribution artifact manifest path should exist"),
        ),
        "apple-window-validate-0001",
        "apple-assignment-validate-0001",
        "apple-challenge-0001",
    );
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("apple validator packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("apple validator run status path should exist"),
    );
    let score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("apple validator score receipt path should exist"),
    );
    assert_eq!(
        score_receipt.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        score_receipt.disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(score_receipt.score_bps, 10_000);
    assert_eq!(
        score_receipt.validator_work_class,
        PsionicTrainWorkClass::ValidationReplay
    );
    assert_eq!(
        score_receipt.challenged_work_class,
        PsionicTrainWorkClass::SmallModelLocalTraining
    );

    let score_artifact: PsionicTrainValidatorScoreArtifact =
        parse_json(&score_receipt.score_artifact_path);
    assert_eq!(
        score_artifact.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        score_artifact.disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(score_artifact.challenge_id.as_str(), "apple-challenge-0001");
    assert_eq!(
        score_artifact.challenged_work_class,
        PsionicTrainWorkClass::SmallModelLocalTraining
    );
}

#[test]
fn validator_manifest_emits_multi_window_quality_drift_and_rollback_signals() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("worker-run");
    let validator_run_root = tempdir.path().join("validator-run");

    let first_launch_manifest_path = tempdir.path().join("worker-window-0001-launch.json");
    let mut first_launch_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut first_launch_manifest,
        "window-0001",
        "assignment-0001",
        1,
    );
    write_manifest(&first_launch_manifest_path, &mut first_launch_manifest);
    let first_launch_output = run_machine_manifest(&first_launch_manifest_path);
    assert!(
        first_launch_output.status.success(),
        "first worker launch should succeed"
    );

    let first_checkpoint_manifest_path = tempdir.path().join("worker-window-0001-checkpoint.json");
    let mut first_checkpoint_manifest = build_retained_operation_manifest(
        &worker_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    bind_window_context(
        &mut first_checkpoint_manifest,
        "window-0001",
        "assignment-0001",
        2,
    );
    first_checkpoint_manifest.checkpoint_label = Some(String::from("validator-target-0001"));
    first_checkpoint_manifest.optimizer_step = Some(4_096);
    first_checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/validator/target/0001"));
    write_manifest(
        &first_checkpoint_manifest_path,
        &mut first_checkpoint_manifest,
    );
    let first_checkpoint_output = run_machine_manifest(&first_checkpoint_manifest_path);
    assert!(
        first_checkpoint_output.status.success(),
        "first worker checkpoint should succeed"
    );
    let first_worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&first_checkpoint_output.stdout)
            .expect("first worker packet should parse");
    let first_worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        first_worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("first worker run status path should exist"),
    );

    let first_validator_manifest_path = tempdir.path().join("validator-window-0001.json");
    let mut first_validator_manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            first_worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("first contribution receipt path should exist"),
        ),
        Path::new(
            first_worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("first contribution artifact manifest path should exist"),
        ),
        "window-0001",
        "assignment-0001",
        "challenge-0001",
    );
    write_manifest(
        &first_validator_manifest_path,
        &mut first_validator_manifest,
    );
    let first_validator_output = run_machine_manifest(&first_validator_manifest_path);
    assert!(
        first_validator_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first_validator_output.stdout),
        String::from_utf8_lossy(&first_validator_output.stderr)
    );
    let first_validator_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&first_validator_output.stdout)
            .expect("first validator packet should parse");
    let first_validator_run_status: PsionicTrainRunStatusPacket = parse_json(
        first_validator_packet
            .run_status_packet_path
            .as_ref()
            .expect("first validator run status path should exist"),
    );
    let first_score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        first_validator_run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("first validator score receipt path should exist"),
    );
    assert_eq!(first_score_receipt.validation_index, 1);
    let first_quality_signal: PsionicTrainValidatorQualityDriftSignal = parse_json(
        first_validator_run_status
            .artifacts
            .validator_quality_drift_signal_path
            .as_ref()
            .expect("first validator quality drift signal path should exist"),
    );
    assert_eq!(
        first_quality_signal.drift_state,
        PsionicTrainValidatorQualityDriftState::Baseline
    );
    let first_rollback_signal: PsionicTrainValidatorRollbackSignal = parse_json(
        first_validator_run_status
            .artifacts
            .validator_rollback_signal_path
            .as_ref()
            .expect("first validator rollback signal path should exist"),
    );
    assert_eq!(
        first_rollback_signal.rollback_posture,
        PsionicTrainValidatorRollbackPosture::Hold
    );

    let second_launch_manifest_path = tempdir.path().join("worker-window-0002-launch.json");
    let mut second_launch_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut second_launch_manifest,
        "window-0002",
        "assignment-0002",
        3,
    );
    write_manifest(&second_launch_manifest_path, &mut second_launch_manifest);
    let second_launch_output = run_machine_manifest(&second_launch_manifest_path);
    assert!(
        second_launch_output.status.success(),
        "second worker launch should succeed"
    );

    let second_checkpoint_manifest_path = tempdir.path().join("worker-window-0002-checkpoint.json");
    let mut second_checkpoint_manifest = build_retained_operation_manifest(
        &worker_run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::RecordCheckpoint,
    );
    bind_window_context(
        &mut second_checkpoint_manifest,
        "window-0002",
        "assignment-0002",
        4,
    );
    second_checkpoint_manifest.checkpoint_label = Some(String::from("validator-target-0002"));
    second_checkpoint_manifest.optimizer_step = Some(8_192);
    second_checkpoint_manifest.checkpoint_ref =
        Some(String::from("checkpoint://psion/validator/target/0002"));
    write_manifest(
        &second_checkpoint_manifest_path,
        &mut second_checkpoint_manifest,
    );
    let second_checkpoint_output = run_machine_manifest(&second_checkpoint_manifest_path);
    assert!(
        second_checkpoint_output.status.success(),
        "second worker checkpoint should succeed"
    );
    let second_worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&second_checkpoint_output.stdout)
            .expect("second worker packet should parse");
    let second_worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        second_worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("second worker run status path should exist"),
    );
    let second_artifact_manifest: PsionicTrainContributionArtifactManifest = parse_json(
        second_worker_run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_ref()
            .expect("second contribution artifact manifest path should exist"),
    );
    let checkpoint_surface_artifact = second_artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == "checkpoint_surface")
        .expect("second contribution should retain checkpoint surface");
    let mut degraded_checkpoint_surface: PsionicTrainCheckpointSurface = parse_json(
        checkpoint_surface_artifact
            .binding
            .materialized_path
            .as_deref()
            .expect("checkpoint surface artifact path should exist"),
    );
    degraded_checkpoint_surface.upload_outcome = Some(String::from("refused"));
    write_json(
        checkpoint_surface_artifact
            .binding
            .materialized_path
            .as_deref()
            .expect("checkpoint surface artifact path should exist"),
        &degraded_checkpoint_surface,
    );

    let second_validator_manifest_path = tempdir.path().join("validator-window-0002.json");
    let mut second_validator_manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            second_worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("second contribution receipt path should exist"),
        ),
        Path::new(
            second_worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("second contribution artifact manifest path should exist"),
        ),
        "window-0002",
        "assignment-0002",
        "challenge-0002",
    );
    write_manifest(
        &second_validator_manifest_path,
        &mut second_validator_manifest,
    );
    let second_validator_output = run_machine_manifest(&second_validator_manifest_path);
    assert!(
        second_validator_output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second_validator_output.stdout),
        String::from_utf8_lossy(&second_validator_output.stderr)
    );
    let second_validator_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&second_validator_output.stdout)
            .expect("second validator packet should parse");
    let second_validator_run_status: PsionicTrainRunStatusPacket = parse_json(
        second_validator_packet
            .run_status_packet_path
            .as_ref()
            .expect("second validator run status path should exist"),
    );
    let second_score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        second_validator_run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("second validator score receipt path should exist"),
    );
    assert_eq!(second_score_receipt.validation_index, 2);
    assert_eq!(
        second_score_receipt.disposition,
        TrainingExecutionValidatorDisposition::ReplayRequired
    );
    assert_eq!(second_score_receipt.score_bps, 5_000);

    let second_quality_signal: PsionicTrainValidatorQualityDriftSignal = parse_json(
        second_validator_run_status
            .artifacts
            .validator_quality_drift_signal_path
            .as_ref()
            .expect("second validator quality drift signal path should exist"),
    );
    assert_eq!(
        second_quality_signal.drift_state,
        PsionicTrainValidatorQualityDriftState::Regressed
    );
    assert_eq!(
        second_quality_signal.previous_window_id.as_deref(),
        Some("window-0001")
    );
    assert_eq!(
        second_quality_signal.current_window_id.as_str(),
        "window-0002"
    );
    assert_eq!(second_quality_signal.score_bps_delta, Some(-5_000));
    assert_eq!(second_quality_signal.degraded_window_count, 1);
    assert_eq!(second_quality_signal.non_accepted_window_count, 1);

    let second_rollback_signal: PsionicTrainValidatorRollbackSignal = parse_json(
        second_validator_run_status
            .artifacts
            .validator_rollback_signal_path
            .as_ref()
            .expect("second validator rollback signal path should exist"),
    );
    assert_eq!(
        second_rollback_signal.rollback_posture,
        PsionicTrainValidatorRollbackPosture::Candidate
    );
    assert_eq!(
        second_rollback_signal
            .rollback_baseline_window_id
            .as_deref(),
        Some("window-0001")
    );
    assert_eq!(
        second_rollback_signal.consecutive_non_accepted_window_count,
        1
    );
}

#[test]
fn validator_manifest_emits_rejected_score_receipt_for_refused_contribution() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("worker-run");
    let worker_manifest_path = tempdir.path().join("worker-refused-launch.json");
    let mut worker_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(&mut worker_manifest, "window-0001", "assignment-0001", 1);
    worker_manifest.admission_identity.build_digest = String::from("sha256:not-the-real-build");
    write_manifest(&worker_manifest_path, &mut worker_manifest);
    let worker_output = run_machine_manifest(&worker_manifest_path);
    assert!(
        !worker_output.status.success(),
        "worker launch should be refused"
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stderr).expect("worker refusal packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );

    let validator_run_root = tempdir.path().join("validator-run");
    let validator_manifest_path = tempdir.path().join("validator-rejected.json");
    let mut validator_manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("worker contribution receipt path should exist"),
        ),
        Path::new(
            worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("worker contribution artifact manifest path should exist"),
        ),
        "window-0001",
        "assignment-0001",
        "challenge-0002",
    );
    write_manifest(&validator_manifest_path, &mut validator_manifest);

    let output = run_machine_manifest(&validator_manifest_path);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stdout).expect("validator packet should parse");
    let run_status: PsionicTrainRunStatusPacket = parse_json(
        packet
            .run_status_packet_path
            .as_ref()
            .expect("validator run status path should exist"),
    );
    let score_receipt: PsionicTrainValidatorScoreReceipt = parse_json(
        run_status
            .artifacts
            .validator_score_receipt_path
            .as_ref()
            .expect("validator score receipt path should exist"),
    );
    assert_eq!(
        score_receipt.disposition,
        TrainingExecutionValidatorDisposition::Rejected
    );
    assert_eq!(score_receipt.score_bps, 0);
}

#[test]
fn validator_manifest_refuses_stale_assignment_targets() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("worker-run");
    let worker_manifest_path = tempdir.path().join("worker-windowed-launch.json");
    let mut worker_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(&mut worker_manifest, "window-0001", "assignment-0001", 1);
    write_manifest(&worker_manifest_path, &mut worker_manifest);
    let worker_output = run_machine_manifest(&worker_manifest_path);
    assert!(
        worker_output.status.success(),
        "worker launch should succeed"
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stdout).expect("worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );

    let validator_run_root = tempdir.path().join("validator-run");
    let manifest_path = tempdir.path().join("validator-stale-assignment.json");
    let mut manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("worker contribution receipt path should exist"),
        ),
        Path::new(
            worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("worker contribution artifact manifest path should exist"),
        ),
        "window-0001",
        "assignment-stale",
        "challenge-0003",
    );
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        !output.status.success(),
        "stale validator target should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::StaleAssignment)
    );
    assert_eq!(packet.outcome, PsionicTrainOutcomeKind::Refused);
    assert_eq!(
        output.status.code(),
        Some(PsionicTrainRefusalClass::StaleAssignment.exit_code() as i32)
    );
}

#[test]
fn validator_manifest_refuses_missing_replay_inputs() {
    let tempdir = tempdir().expect("tempdir should exist");
    let run_root = tempdir.path().join("validator-run");
    let manifest_path = tempdir.path().join("validator-missing-inputs.json");
    let mut manifest = build_retained_operation_manifest(
        &run_root,
        PsionicTrainRole::Validator,
        PsionicTrainOperation::ValidateContribution,
    );
    manifest.work_class = PsionicTrainWorkClass::ValidationReplay;
    manifest.coordination.window_id = Some(String::from("window-0001"));
    manifest.coordination.assignment_id = Some(String::from("assignment-0001"));
    manifest.coordination.challenge_id = Some(String::from("challenge-0004"));
    manifest.coordination.node_pubkey = Some(String::from("npub1-psionic-validator-cli-test"));
    manifest.validator_target_contribution_receipt =
        Some(psionic_train::PsionicTrainArtifactBinding {
            artifact_ref: psionic_train::PsionicTrainArtifactRef {
                artifact_id: String::from("psionic.train.artifact.contribution_receipt.missing"),
                artifact_digest: None,
                artifact_bytes: None,
            },
            materialized_path: None,
        });
    manifest.validator_target_contribution_artifact_manifest =
        Some(psionic_train::PsionicTrainArtifactBinding {
            artifact_ref: psionic_train::PsionicTrainArtifactRef {
                artifact_id: String::from(
                    "psionic.train.artifact.contribution_artifact_manifest.missing",
                ),
                artifact_digest: None,
                artifact_bytes: None,
            },
            materialized_path: None,
        });
    manifest.validator_target_work_class =
        Some(PsionicTrainWorkClass::FullIslandLocalUpdateTraining);
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        !output.status.success(),
        "missing validator inputs should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::ArtifactIncomplete)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("artifacts/resolved"),
        "refusal should point at the canonical resolver cache path:\n{}",
        stderr
    );
}

#[test]
fn grouped_stage_validator_manifest_refuses_missing_stage_evidence() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("apple-grouped-validator-run");

    let launch_manifest_path = tempdir.path().join("apple-grouped-validator-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-grouped-validator-window-0001",
        "apple-grouped-validator-assignment-0001",
        1,
    );
    launch_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    launch_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "grouped worker launch should succeed"
    );

    let checkpoint_manifest_path = tempdir
        .path()
        .join("apple-grouped-validator-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &worker_run_root,
        "apple-grouped-validator-target",
        8_192,
        "checkpoint://psion/apple/grouped/validator-target",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-grouped-validator-window-0001",
        "apple-grouped-validator-assignment-0001",
        2,
    );
    checkpoint_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    checkpoint_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "grouped worker checkpoint should succeed"
    );

    let worker_packet: PsionicTrainStatusPacket = serde_json::from_slice(&checkpoint_output.stdout)
        .expect("grouped worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("grouped worker run status path should exist"),
    );
    let artifact_manifest: PsionicTrainContributionArtifactManifest = parse_json(
        worker_run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_ref()
            .expect("grouped artifact manifest path should exist"),
    );
    let grouped_stage_execution_summary_path = artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == "grouped_stage_execution_summary")
        .and_then(|artifact| artifact.binding.materialized_path.clone())
        .expect("grouped stage execution summary artifact should exist");
    fs::remove_file(&grouped_stage_execution_summary_path)
        .expect("grouped stage execution summary should delete");

    let validator_run_root = tempdir.path().join("apple-grouped-validator-replay-run");
    let validator_manifest_path = tempdir.path().join("apple-grouped-validator-replay.json");
    let mut validator_manifest = build_validator_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("grouped contribution receipt path should exist"),
        ),
        Path::new(
            worker_run_status
                .artifacts
                .contribution_artifact_manifest_path
                .as_deref()
                .expect("grouped contribution artifact manifest path should exist"),
        ),
        "apple-grouped-validator-window-0001",
        "apple-grouped-validator-assignment-0001",
        "apple-grouped-validator-challenge-0001",
    );
    write_manifest(&validator_manifest_path, &mut validator_manifest);

    let output = run_machine_manifest(&validator_manifest_path);
    assert!(
        !output.status.success(),
        "grouped validator with missing stage evidence should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("grouped validator refusal should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::ArtifactIncomplete)
    );
}

#[test]
fn validator_manifest_refuses_artifact_manifest_digest_drift() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("worker-run");
    let worker_manifest_path = tempdir.path().join("worker-windowed-launch.json");
    let mut worker_manifest = build_launch_manifest(&worker_run_root);
    bind_window_context(&mut worker_manifest, "window-0001", "assignment-0001", 1);
    write_manifest(&worker_manifest_path, &mut worker_manifest);
    let worker_output = run_machine_manifest(&worker_manifest_path);
    assert!(
        worker_output.status.success(),
        "worker launch should succeed"
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&worker_output.stdout).expect("worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );
    let artifact_manifest_path = PathBuf::from(
        worker_run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_deref()
            .expect("worker contribution artifact manifest path should exist"),
    );
    let mut artifact_manifest: PsionicTrainContributionArtifactManifest =
        parse_json(&artifact_manifest_path);
    artifact_manifest.artifact_manifest_digest = String::from("drifted-artifact-manifest-digest");
    fs::write(
        &artifact_manifest_path,
        serde_json::to_vec_pretty(&artifact_manifest).expect("artifact manifest should serialize"),
    )
    .expect("artifact manifest should rewrite");

    let validator_run_root = tempdir.path().join("validator-run");
    let manifest_path = tempdir.path().join("validator-drifted-manifest.json");
    let mut manifest = build_validator_manifest(
        &validator_run_root,
        Path::new(
            worker_run_status
                .artifacts
                .contribution_receipt_path
                .as_deref()
                .expect("worker contribution receipt path should exist"),
        ),
        &artifact_manifest_path,
        "window-0001",
        "assignment-0001",
        "challenge-0005",
    );
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        !output.status.success(),
        "drifted validator input should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::ArtifactDigestMismatch)
    );
}

#[test]
fn build_identity_mismatch_is_refused_before_launch() {
    let tempdir = tempdir().expect("tempdir should exist");
    let manifest_path = tempdir.path().join("mismatched-build.json");
    let mut manifest = base_manifest();
    manifest.output_root = Some(
        tempdir
            .path()
            .join("mismatched-build-run")
            .display()
            .to_string(),
    );
    manifest.allow_dirty_tree = true;
    manifest.admission_identity.build_digest = String::from("sha256:not-the-real-build");
    write_manifest(&manifest_path, &mut manifest);

    let output = run_machine_manifest(&manifest_path);

    assert!(
        !output.status.success(),
        "mismatched build should be refused"
    );
    let packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&output.stderr).expect("refusal packet should parse");
    assert_eq!(
        packet.refusal_class,
        Some(PsionicTrainRefusalClass::BuildRevoked)
    );
}

#[test]
fn apple_grouped_stage_records_weak_device_accepted_outcome_proof() {
    let tempdir = tempdir().expect("tempdir should exist");
    let worker_run_root = tempdir.path().join("apple-grouped-weak-device-run");

    let launch_manifest_path = tempdir.path().join("apple-grouped-weak-device-launch.json");
    let mut launch_manifest = build_apple_launch_manifest(&worker_run_root);
    bind_window_context(
        &mut launch_manifest,
        "apple-grouped-weak-device-window-0001",
        "apple-grouped-weak-device-assignment-0001",
        1,
    );
    launch_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    launch_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&launch_manifest_path, &mut launch_manifest);
    let launch_output = run_machine_manifest(&launch_manifest_path);
    assert!(
        launch_output.status.success(),
        "grouped weak-device launch should succeed"
    );

    let checkpoint_manifest_path = tempdir
        .path()
        .join("apple-grouped-weak-device-record-checkpoint.json");
    let mut checkpoint_manifest = build_apple_record_checkpoint_manifest(
        &worker_run_root,
        "apple-grouped-weak-device-accepted",
        6_144,
        "checkpoint://psion/apple/grouped/weak-device/accepted",
    );
    bind_window_context(
        &mut checkpoint_manifest,
        "apple-grouped-weak-device-window-0001",
        "apple-grouped-weak-device-assignment-0001",
        2,
    );
    checkpoint_manifest.work_class = PsionicTrainWorkClass::GroupedReplicaStageExecution;
    checkpoint_manifest.grouped_stage_assignment = Some(grouped_stage_assignment(
        "stage-01",
        0,
        2,
        PsionicTrainGroupedReplicaStageRole::Ingress,
        None,
        Some("stage-02"),
    ));
    write_manifest(&checkpoint_manifest_path, &mut checkpoint_manifest);
    let checkpoint_output = run_machine_manifest(&checkpoint_manifest_path);
    assert!(
        checkpoint_output.status.success(),
        "grouped weak-device checkpoint should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&checkpoint_output.stdout),
        String::from_utf8_lossy(&checkpoint_output.stderr)
    );

    let worker_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&checkpoint_output.stdout).expect("worker packet should parse");
    let worker_run_status: PsionicTrainRunStatusPacket = parse_json(
        worker_packet
            .run_status_packet_path
            .as_ref()
            .expect("worker run status path should exist"),
    );

    let validator_run_root = tempdir
        .path()
        .join("apple-grouped-weak-device-validator-run");
    let contribution_receipt_path = PathBuf::from(
        worker_run_status
            .artifacts
            .contribution_receipt_path
            .as_deref()
            .expect("worker contribution receipt path should exist"),
    );
    let contribution_artifact_manifest_path = PathBuf::from(
        worker_run_status
            .artifacts
            .contribution_artifact_manifest_path
            .as_deref()
            .expect("worker contribution artifact manifest path should exist"),
    );
    let cached_artifact_manifest = cache_contribution_family(
        &validator_run_root,
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
    );
    let validator_manifest_path = tempdir
        .path()
        .join("apple-grouped-weak-device-validator.json");
    let mut validator_manifest = build_validator_manifest_for_lane(
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
        &validator_run_root,
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
        "apple-grouped-weak-device-window-0001",
        "apple-grouped-weak-device-assignment-0001",
        "apple-grouped-weak-device-challenge-0001",
    );
    remove_contribution_family(
        contribution_receipt_path.as_path(),
        contribution_artifact_manifest_path.as_path(),
        &cached_artifact_manifest,
    );
    assert!(
        !contribution_receipt_path.is_file(),
        "worker contribution receipt should be removed before replay rematerialization"
    );
    assert!(
        !contribution_artifact_manifest_path.is_file(),
        "worker contribution artifact manifest should be removed before replay rematerialization"
    );
    write_manifest(&validator_manifest_path, &mut validator_manifest);
    let validator_output = run_machine_manifest(&validator_manifest_path);
    assert!(
        validator_output.status.success(),
        "grouped weak-device validator should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validator_output.stdout),
        String::from_utf8_lossy(&validator_output.stderr)
    );
    assert!(
        contribution_receipt_path.is_file(),
        "validator replay should restore the worker contribution receipt path"
    );
    assert!(
        contribution_artifact_manifest_path.is_file(),
        "validator replay should restore the worker contribution artifact manifest path"
    );

    let validator_packet: PsionicTrainStatusPacket =
        serde_json::from_slice(&validator_output.stdout).expect("validator packet should parse");
    let validator_run_status: PsionicTrainRunStatusPacket = parse_json(
        validator_packet
            .run_status_packet_path
            .as_ref()
            .expect("validator run status path should exist"),
    );

    let proof_path = tempdir
        .path()
        .join("apple-grouped-weak-device-accepted-outcome-proof.json");
    let proof = record_psionic_train_weak_device_accepted_outcome_proof(
        proof_path.as_path(),
        &worker_run_status,
        &validator_run_status,
    )
    .expect("weak-device accepted-outcome proof should record");

    assert_eq!(
        proof.lane_id.as_str(),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID
    );
    assert_eq!(
        proof.carrier.backend_family.as_str(),
        psionic_train::PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY
    );
    assert_eq!(
        proof.carrier.work_class,
        PsionicTrainWorkClass::GroupedReplicaStageExecution
    );
    assert_eq!(
        proof.validator.validator_disposition,
        TrainingExecutionValidatorDisposition::Accepted
    );
    assert_eq!(
        proof.validator.rollback_posture,
        PsionicTrainValidatorRollbackPosture::Hold
    );
    assert!(
        proof
            .validator
            .verified_hooks
            .contains(&PsionicTrainValidatorHook::GroupedStageIntegrity)
    );
    assert!(
        proof
            .cited_artifacts
            .iter()
            .any(|artifact| artifact.artifact_role == "grouped_stage_replay_evidence")
    );

    let persisted: PsionicTrainWeakDeviceAcceptedOutcomeProof = parse_json(&proof_path);
    assert_eq!(persisted.bundle_digest, proof.bundle_digest);
    persisted
        .validate()
        .expect("persisted weak-device proof should validate");
}
