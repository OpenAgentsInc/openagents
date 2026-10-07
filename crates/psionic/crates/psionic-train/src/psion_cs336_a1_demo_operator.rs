use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Cs336A1ReferenceTrainer, Cs336A1ReferenceTrainingStepReport,
    PSION_CS336_A1_DEMO_CHECKPOINT_LABEL, PSION_CS336_A1_DEMO_CLAIM_BOUNDARY,
    PSION_CS336_A1_DEMO_CLOSEOUT_BUNDLE_SCHEMA_VERSION,
    PSION_CS336_A1_DEMO_CURRENT_RUN_STATUS_SCHEMA_VERSION, PSION_CS336_A1_DEMO_LANE_ID,
    PSION_CS336_A1_DEMO_RETAINED_SUMMARY_SCHEMA_VERSION, PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
    PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
    PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSIONIC_TRAIN_PACKAGED_RUNTIME_DIRTY_TREE_ADMISSION,
    PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE, PSIONIC_TRAIN_RUN_STATUS_PACKET_SCHEMA_VERSION,
    PSIONIC_TRAIN_RUNTIME_ATTESTATION_SCHEMA_VERSION, PSIONIC_TRAIN_RUNTIME_ROOT_ENV,
    PSIONIC_TRAIN_RUNTIME_SURFACE_ID, PSIONIC_TRAIN_WINDOW_STATUS_PACKET_SCHEMA_VERSION,
    PsionCs336A1DemoAutomaticExecutionRequest, PsionCs336A1DemoCurrentRunStatus,
    PsionCs336A1DemoLaunchManifest, PsionCs336A1DemoRetainedSummary,
    PsionicTrainArtifactSurfaceRefs, PsionicTrainAuthorityOwner, PsionicTrainCapabilityProjection,
    PsionicTrainCheckpointManifest, PsionicTrainCheckpointPointer, PsionicTrainInvocationManifest,
    PsionicTrainOperation, PsionicTrainOutcomeKind, PsionicTrainRole, PsionicTrainRunStatusPacket,
    PsionicTrainRuntimeAttestation, PsionicTrainWindowStatusPacket,
    build_psion_cs336_a1_demo_launch_manifest, inspect_psionic_train_checkpoint_surface,
    load_cs336_a1_reference_checkpoint, psion_cs336_a1_demo_retained_paths,
};

pub const PSION_CS336_A1_DEMO_VERIFICATION_REPORT_SCHEMA_VERSION: &str =
    "psion.cs336_a1_demo_verification_report.v1";

#[derive(Debug, Error)]
pub enum PsionCs336A1DemoOperatorError {
    #[error("the bounded A1 demo lane expected `{expected}` but found `{actual}`")]
    LaneMismatch { expected: String, actual: String },
    #[error("the bounded A1 demo lane does not support operation `{0}` yet")]
    UnsupportedOperation(String),
    #[error("missing required field `{0}`")]
    MissingField(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Training(#[from] crate::Cs336A1ReferenceTrainingError),
    #[error(transparent)]
    Launcher(#[from] crate::PsionCs336A1DemoLauncherError),
    #[error(transparent)]
    RuntimeContract(#[from] crate::PsionicTrainRuntimeContractError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoCloseoutBundle {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub outcome: String,
    pub claim_boundary: String,
    pub corpus_fixture_path: String,
    pub training_step_count: u64,
    pub launch_manifest: PsionCs336A1DemoLaunchManifest,
    pub initial_loss: f32,
    pub final_loss: f32,
    pub checkpoint_label: String,
    pub checkpoint_ref: String,
    pub checkpoint_digest: String,
    pub model_state_digest: String,
    pub optimizer_state_digest: String,
    pub step_reports: Vec<Cs336A1ReferenceTrainingStepReport>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionCs336A1DemoVerificationReport {
    pub schema_version: String,
    pub run_root: String,
    pub lane_id: String,
    pub release_id: String,
    pub environment_ref: String,
    pub current_phase: Option<String>,
    pub ready_for_demo: bool,
    pub has_status_packet: bool,
    pub has_window_status_packet: bool,
    pub has_checkpoint_surface: bool,
    pub has_closeout_bundle: bool,
    pub final_loss_descended: bool,
    pub checkpoint_ref: Option<String>,
    pub failures: Vec<String>,
    pub caveats: Vec<String>,
}

pub fn run_psion_cs336_a1_demo_cli(args: Vec<String>) -> Result<(), PsionCs336A1DemoOperatorError> {
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        print_usage();
        return Ok(());
    }

    match args[0].as_str() {
        "start" | "rehearse-base-lane" => {
            let parsed = parse_start_like_args(args[0].as_str(), &args[1..])?;
            let request = PsionCs336A1DemoAutomaticExecutionRequest {
                schema_version: String::from(
                    crate::PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
                ),
                role: PsionicTrainRole::Worker,
                operation: parsed.operation,
                coordination: crate::PsionicTrainCoordinationContext {
                    node_pubkey: Some(String::from("npub1-local-cs336-a1-demo")),
                    ..Default::default()
                },
                build_digest: String::from("local-rehearsal"),
                run_id: parsed.run_id,
                output_root: Some(parsed.output_root),
                run_root: None,
                selected_git_ref: parsed.selected_git_ref,
                allow_dirty_tree: parsed.allow_dirty_tree,
                dry_run: parsed.dry_run,
            };
            let manifest = request.to_invocation_manifest()?;
            run_psion_cs336_a1_demo_manifest(&manifest)
        }
        "status" => {
            let run_root = parse_run_root_args(&args[1..], "status")?;
            print_status(Path::new(run_root.as_str()))
        }
        "verify" => {
            let run_root = parse_run_root_args(&args[1..], "verify")?;
            print_verification_report(Path::new(run_root.as_str()))
        }
        other => Err(PsionCs336A1DemoOperatorError::UnsupportedOperation(
            other.to_string(),
        )),
    }
}

pub fn run_psion_cs336_a1_demo_manifest(
    manifest: &PsionicTrainInvocationManifest,
) -> Result<(), PsionCs336A1DemoOperatorError> {
    if manifest.lane_id != PSION_CS336_A1_DEMO_LANE_ID {
        return Err(PsionCs336A1DemoOperatorError::LaneMismatch {
            expected: String::from(PSION_CS336_A1_DEMO_LANE_ID),
            actual: manifest.lane_id.clone(),
        });
    }
    if manifest.role != PsionicTrainRole::Worker {
        return Err(PsionCs336A1DemoOperatorError::UnsupportedOperation(
            String::from("only worker role is packaged for the bounded A1 lane"),
        ));
    }

    match manifest.operation {
        PsionicTrainOperation::Start => run_start_like(manifest, false),
        PsionicTrainOperation::RehearseBaseLane => run_start_like(manifest, true),
        other => Err(PsionCs336A1DemoOperatorError::UnsupportedOperation(
            other.cli_subcommand().to_string(),
        )),
    }
}

fn run_start_like(
    manifest: &PsionicTrainInvocationManifest,
    rehearsal: bool,
) -> Result<(), PsionCs336A1DemoOperatorError> {
    let run_root =
        PathBuf::from(manifest.output_root.as_deref().ok_or_else(|| {
            PsionCs336A1DemoOperatorError::MissingField(String::from("output_root"))
        })?);
    let retained_paths = psion_cs336_a1_demo_retained_paths();
    let launch_manifest = build_psion_cs336_a1_demo_launch_manifest(
        manifest
            .run_id
            .clone()
            .unwrap_or_else(|| String::from("psion-cs336-a1-demo")),
        manifest
            .selected_git_ref
            .clone()
            .unwrap_or_else(|| String::from("HEAD")),
        rehearsal,
    );

    create_run_dirs(&run_root)?;
    write_json_pretty(
        run_root
            .join(&retained_paths.launch_manifest_path)
            .as_path(),
        &launch_manifest,
    )?;
    append_log(
        run_root.join(&retained_paths.launcher_log_path).as_path(),
        format!(
            "phase=launch_manifest_written run_id={} surface_id={}",
            launch_manifest.run_id, launch_manifest.surface_id
        )
        .as_str(),
    )?;

    let mut current_status = PsionCs336A1DemoCurrentRunStatus {
        schema_version: String::from(PSION_CS336_A1_DEMO_CURRENT_RUN_STATUS_SCHEMA_VERSION),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: launch_manifest.run_id.clone(),
        phase: if manifest.dry_run {
            String::from("dry_run_materialized")
        } else {
            String::from("launching")
        },
        completed_steps: 0,
        total_steps: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        latest_loss: None,
        checkpoint_ref: None,
        detail: String::from(
            "Bounded CS336 A1 demo lane is materializing retained launcher surfaces.",
        ),
    };
    write_json_pretty(
        run_root.join(&retained_paths.current_status_path).as_path(),
        &current_status,
    )?;

    if manifest.dry_run {
        let summary = PsionCs336A1DemoRetainedSummary {
            schema_version: String::from(PSION_CS336_A1_DEMO_RETAINED_SUMMARY_SCHEMA_VERSION),
            lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
            run_id: launch_manifest.run_id.clone(),
            claim_boundary: String::from(PSION_CS336_A1_DEMO_CLAIM_BOUNDARY),
            corpus_fixture_path: launch_manifest.corpus_fixture_path.clone(),
            training_config: launch_manifest.training_config.clone(),
            total_steps: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
            initial_loss: None,
            final_loss: None,
            latest_checkpoint_label: None,
            latest_checkpoint_ref: None,
            latest_checkpoint_path: None,
            model_state_digest: None,
            optimizer_state_digest: None,
            checkpoint_digest: None,
            detail: String::from(
                "Dry-run rehearsal materialized the packaged A1 lane without executing training.",
            ),
        };
        write_json_pretty(
            run_root
                .join(&retained_paths.retained_summary_path)
                .as_path(),
            &summary,
        )?;
        append_log(
            run_root.join(&retained_paths.launcher_log_path).as_path(),
            "phase=dry_run_complete detail=retained_launcher_surfaces_only",
        )?;
        write_runtime_packets(
            &run_root,
            manifest,
            &current_status,
            summary.detail.as_str(),
        )?;
        return Ok(());
    }

    let corpus_path = repo_root().join(launch_manifest.corpus_fixture_path.as_str());
    let mut trainer = Cs336A1ReferenceTrainer::from_corpus_path(
        &corpus_path,
        launch_manifest.training_config.clone(),
    )?;
    let initial_loss = trainer.current_loss()?;
    append_log(
        run_root.join(&retained_paths.launcher_log_path).as_path(),
        format!(
            "phase=training_started run_id={} initial_loss={initial_loss:.6}",
            launch_manifest.run_id
        )
        .as_str(),
    )?;

    let mut step_reports = Vec::with_capacity(PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT as usize);
    for _ in 0..PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT {
        let report = trainer.step()?;
        current_status.phase = String::from("training");
        current_status.completed_steps = report.step_number;
        current_status.latest_loss = Some(report.loss_after);
        current_status.detail = format!(
            "Bounded CS336 A1 demo lane completed step {} of {}.",
            report.step_number, PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT
        );
        write_json_pretty(
            run_root.join(&retained_paths.current_status_path).as_path(),
            &current_status,
        )?;
        append_log(
            run_root.join(&retained_paths.launcher_log_path).as_path(),
            format!(
                "phase=step_complete step={} loss_before={:.6} loss_after={:.6}",
                report.step_number, report.loss_before, report.loss_after
            )
            .as_str(),
        )?;
        step_reports.push(report);
    }

    let checkpoint_path = run_root.join(&retained_paths.checkpoint_payload_path);
    let checkpoint_receipt = trainer.save_checkpoint(&checkpoint_path)?;
    let checkpoint_total_bytes = fs::metadata(&checkpoint_path)?.len();
    let checkpoint_ref = format!(
        "checkpoint://psion/cs336_a1_demo/{}/step-{:06}",
        launch_manifest.run_id, PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT
    );
    let mut checkpoint_manifest = PsionicTrainCheckpointManifest {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: launch_manifest.run_id.clone(),
        window_id: manifest.coordination.window_id.clone(),
        assignment_id: manifest.coordination.assignment_id.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        checkpoint_label: String::from(PSION_CS336_A1_DEMO_CHECKPOINT_LABEL),
        optimizer_step: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        checkpoint_ref: checkpoint_ref.clone(),
        relative_manifest_path: retained_paths.checkpoint_manifest_path.clone(),
        checkpoint_object_digest: checkpoint_receipt.checkpoint_digest.clone(),
        checkpoint_total_bytes,
        manifest_digest: String::new(),
    };
    checkpoint_manifest.manifest_digest = checkpoint_manifest.stable_manifest_digest();

    let checkpoint_pointer = PsionicTrainCheckpointPointer {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: launch_manifest.run_id.clone(),
        window_id: manifest.coordination.window_id.clone(),
        assignment_id: manifest.coordination.assignment_id.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        pointer_state: String::from("accepted"),
        checkpoint_label: String::from(PSION_CS336_A1_DEMO_CHECKPOINT_LABEL),
        optimizer_step: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_manifest_relative_path: retained_paths.checkpoint_manifest_path.clone(),
        detail: String::from(
            "Bounded CS336 A1 demo lane retained one accepted generic checkpoint for Pylon and Nexus closeout ingestion.",
        ),
    };
    write_json_pretty(
        run_root
            .join(&retained_paths.checkpoint_manifest_path)
            .as_path(),
        &checkpoint_manifest,
    )?;
    write_json_pretty(
        run_root
            .join(&retained_paths.checkpoint_pointer_path)
            .as_path(),
        &checkpoint_pointer,
    )?;

    let final_loss = step_reports
        .last()
        .map(|value| value.loss_after)
        .unwrap_or(initial_loss);
    let checkpoint = load_cs336_a1_reference_checkpoint(&checkpoint_path)?;
    let summary = PsionCs336A1DemoRetainedSummary {
        schema_version: String::from(PSION_CS336_A1_DEMO_RETAINED_SUMMARY_SCHEMA_VERSION),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: launch_manifest.run_id.clone(),
        claim_boundary: String::from(PSION_CS336_A1_DEMO_CLAIM_BOUNDARY),
        corpus_fixture_path: launch_manifest.corpus_fixture_path.clone(),
        training_config: checkpoint.config.clone(),
        total_steps: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        initial_loss: Some(initial_loss),
        final_loss: Some(final_loss),
        latest_checkpoint_label: Some(String::from(PSION_CS336_A1_DEMO_CHECKPOINT_LABEL)),
        latest_checkpoint_ref: Some(checkpoint_ref.clone()),
        latest_checkpoint_path: Some(checkpoint_path.display().to_string()),
        model_state_digest: Some(checkpoint_receipt.model_state_digest.clone()),
        optimizer_state_digest: Some(checkpoint_receipt.optimizer_state_digest.clone()),
        checkpoint_digest: Some(checkpoint_receipt.checkpoint_digest.clone()),
        detail: if rehearsal {
            String::from(
                "Rehearsal completed the packaged bounded A1 lane and retained one accepted checkpoint and closeout bundle.",
            )
        } else {
            String::from(
                "Packaged bounded A1 lane completed and retained one accepted checkpoint and closeout bundle for shared Pylon ingestion.",
            )
        },
    };
    write_json_pretty(
        run_root
            .join(&retained_paths.retained_summary_path)
            .as_path(),
        &summary,
    )?;

    let closeout = PsionCs336A1DemoCloseoutBundle {
        schema_version: String::from(PSION_CS336_A1_DEMO_CLOSEOUT_BUNDLE_SCHEMA_VERSION),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        run_id: launch_manifest.run_id.clone(),
        outcome: String::from("accepted"),
        claim_boundary: String::from(PSION_CS336_A1_DEMO_CLAIM_BOUNDARY),
        corpus_fixture_path: launch_manifest.corpus_fixture_path.clone(),
        training_step_count: PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT,
        launch_manifest,
        initial_loss,
        final_loss,
        checkpoint_label: String::from(PSION_CS336_A1_DEMO_CHECKPOINT_LABEL),
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_digest: checkpoint_receipt.checkpoint_digest.clone(),
        model_state_digest: checkpoint_receipt.model_state_digest.clone(),
        optimizer_state_digest: checkpoint_receipt.optimizer_state_digest.clone(),
        step_reports,
    };
    write_json_pretty(
        run_root
            .join(&retained_paths.closeout_bundle_path)
            .as_path(),
        &closeout,
    )?;

    current_status.phase = if rehearsal {
        String::from("rehearsed")
    } else {
        String::from("completed")
    };
    current_status.completed_steps = PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT;
    current_status.latest_loss = Some(final_loss);
    current_status.checkpoint_ref = Some(checkpoint_ref);
    current_status.detail = String::from(
        "Bounded CS336 A1 demo lane completed with one accepted checkpoint and one closeout bundle.",
    );
    write_json_pretty(
        run_root.join(&retained_paths.current_status_path).as_path(),
        &current_status,
    )?;
    append_log(
        run_root.join(&retained_paths.launcher_log_path).as_path(),
        format!(
            "phase=complete run_id={} final_loss={final_loss:.6} checkpoint_digest={}",
            summary.run_id,
            summary
                .checkpoint_digest
                .as_deref()
                .unwrap_or("unknown_checkpoint_digest")
        )
        .as_str(),
    )?;
    write_runtime_packets(
        &run_root,
        manifest,
        &current_status,
        current_status.detail.as_str(),
    )?;
    Ok(())
}

fn print_status(run_root: &Path) -> Result<(), PsionCs336A1DemoOperatorError> {
    let retained_paths = psion_cs336_a1_demo_retained_paths();
    let current_status: PsionCs336A1DemoCurrentRunStatus =
        load_json_file(run_root.join(retained_paths.current_status_path).as_path())?;
    let summary: PsionCs336A1DemoRetainedSummary = load_json_file(
        run_root
            .join(retained_paths.retained_summary_path)
            .as_path(),
    )?;
    let closeout_path = run_root.join(retained_paths.closeout_bundle_path);
    let closeout_exists = closeout_path.is_file();
    let checkpoint_surface = inspect_psionic_train_checkpoint_surface(
        run_root,
        PsionicTrainRole::Worker,
        PsionicTrainOperation::Start,
    )
    .ok()
    .flatten();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "run_root": run_root.display().to_string(),
            "current_status": current_status,
            "retained_summary": summary,
            "closeout_bundle_path": closeout_exists.then(|| closeout_path.display().to_string()),
            "checkpoint_surface": checkpoint_surface,
        }))?
    );
    Ok(())
}

fn print_verification_report(run_root: &Path) -> Result<(), PsionCs336A1DemoOperatorError> {
    println!("{}", serde_json::to_string_pretty(&verify_run(run_root)?)?);
    Ok(())
}

fn verify_run(
    run_root: &Path,
) -> Result<PsionCs336A1DemoVerificationReport, PsionCs336A1DemoOperatorError> {
    let retained_paths = psion_cs336_a1_demo_retained_paths();
    let launch_manifest: PsionCs336A1DemoLaunchManifest = load_json_file(
        run_root
            .join(&retained_paths.launch_manifest_path)
            .as_path(),
    )?;
    let current_status: PsionCs336A1DemoCurrentRunStatus =
        load_json_file(run_root.join(&retained_paths.current_status_path).as_path())?;
    let summary: PsionCs336A1DemoRetainedSummary = load_json_file(
        run_root
            .join(&retained_paths.retained_summary_path)
            .as_path(),
    )?;
    let closeout_path = run_root.join(&retained_paths.closeout_bundle_path);
    let closeout: Option<PsionCs336A1DemoCloseoutBundle> = if closeout_path.is_file() {
        Some(load_json_file(closeout_path.as_path())?)
    } else {
        None
    };
    let checkpoint_surface = inspect_psionic_train_checkpoint_surface(
        run_root,
        PsionicTrainRole::Worker,
        launch_manifest_surface_operation(&launch_manifest),
    )
    .ok()
    .flatten();
    let mut failures = Vec::new();
    let mut caveats = vec![
        String::from(
            "This verifier only proves the bounded single-host lane contract. Fresh multi-host proof still depends on live Pylon and Nexus assignment intake.",
        ),
        String::from(
            "Treasury, payout reconciliation, and contribution validation remain downstream concerns outside this bounded lane checker.",
        ),
    ];

    for (field, actual) in [
        ("launch_manifest.lane_id", launch_manifest.lane_id.as_str()),
        ("current_status.lane_id", current_status.lane_id.as_str()),
        ("retained_summary.lane_id", summary.lane_id.as_str()),
    ] {
        if actual != PSION_CS336_A1_DEMO_LANE_ID {
            failures.push(format!(
                "{field} expected `{PSION_CS336_A1_DEMO_LANE_ID}` but found `{actual}`"
            ));
        }
    }

    if launch_manifest.training_step_count != PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT {
        failures.push(format!(
            "launch_manifest.training_step_count expected `{PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT}` but found `{}`",
            launch_manifest.training_step_count
        ));
    }
    if current_status.total_steps != PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT {
        failures.push(format!(
            "current_status.total_steps expected `{PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT}` but found `{}`",
            current_status.total_steps
        ));
    }
    if summary.total_steps != PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT {
        failures.push(format!(
            "retained_summary.total_steps expected `{PSION_CS336_A1_DEMO_TRAINING_STEP_COUNT}` but found `{}`",
            summary.total_steps
        ));
    }
    if launch_manifest.corpus_fixture_path != crate::CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH {
        failures.push(format!(
            "launch_manifest.corpus_fixture_path expected `{}` but found `{}`",
            crate::CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
            launch_manifest.corpus_fixture_path
        ));
    }

    let has_status_packet = run_root
        .join("status/psionic_train_run_status_packet.json")
        .is_file();
    if !has_status_packet {
        failures.push(String::from(
            "missing retained status/psionic_train_run_status_packet.json runtime packet",
        ));
    }
    let has_window_status_packet = run_root
        .join("status/psionic_train_window_status_packet.json")
        .is_file();
    if !has_window_status_packet {
        failures.push(String::from(
            "missing retained status/psionic_train_window_status_packet.json runtime packet",
        ));
    }

    let final_loss_descended = match (summary.initial_loss, summary.final_loss) {
        (Some(initial_loss), Some(final_loss)) if final_loss < initial_loss => true,
        (Some(_), Some(_)) => {
            failures.push(String::from(
                "retained_summary.final_loss did not descend below retained_summary.initial_loss",
            ));
            false
        }
        _ => {
            failures.push(String::from(
                "retained_summary is missing the initial_loss/final_loss pair required for a real bounded run",
            ));
            false
        }
    };

    let has_checkpoint_surface = checkpoint_surface.is_some();
    match checkpoint_surface.as_ref() {
        Some(surface) => {
            if surface.lane_id != PSION_CS336_A1_DEMO_LANE_ID {
                failures.push(format!(
                    "checkpoint_surface.lane_id expected `{PSION_CS336_A1_DEMO_LANE_ID}` but found `{}`",
                    surface.lane_id
                ));
            }
            if surface.pointer_state.as_deref() != Some("accepted") {
                failures.push(format!(
                    "checkpoint surface pointer_state expected `accepted` but found `{}`",
                    surface.pointer_state.as_deref().unwrap_or("missing")
                ));
            }
            if surface.checkpoint_ref != summary.latest_checkpoint_ref {
                failures.push(String::from(
                    "checkpoint surface checkpoint_ref does not match retained_summary.latest_checkpoint_ref",
                ));
            }
        }
        None => failures.push(String::from(
            "missing retained generic checkpoint surface for bounded lane run",
        )),
    }

    match closeout.as_ref() {
        Some(bundle) => {
            if bundle.lane_id != PSION_CS336_A1_DEMO_LANE_ID {
                failures.push(format!(
                    "closeout_bundle.lane_id expected `{PSION_CS336_A1_DEMO_LANE_ID}` but found `{}`",
                    bundle.lane_id
                ));
            }
            if bundle.outcome != "accepted" {
                failures.push(format!(
                    "closeout_bundle.outcome expected `accepted` but found `{}`",
                    bundle.outcome
                ));
            }
            if Some(bundle.checkpoint_ref.clone()) != summary.latest_checkpoint_ref {
                failures.push(String::from(
                    "closeout bundle checkpoint_ref does not match retained_summary.latest_checkpoint_ref",
                ));
            }
            if bundle.final_loss >= bundle.initial_loss {
                failures.push(String::from(
                    "closeout bundle final_loss did not descend below initial_loss",
                ));
            }
        }
        None => failures.push(String::from(
            "missing closeout/closeout_bundle.json required for a demo-valid bounded run",
        )),
    }

    if current_status.phase == "dry_run_materialized" {
        failures.push(String::from(
            "dry_run materialized launcher surfaces only; it is not a demo-valid bounded execution",
        ));
        caveats.push(String::from(
            "Dry runs are still useful for launch-contract rehearsal, but they do not prove checkpoint or closeout retention.",
        ));
    }

    Ok(PsionCs336A1DemoVerificationReport {
        schema_version: String::from(PSION_CS336_A1_DEMO_VERIFICATION_REPORT_SCHEMA_VERSION),
        run_root: run_root.display().to_string(),
        lane_id: String::from(PSION_CS336_A1_DEMO_LANE_ID),
        release_id: String::from(crate::PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID),
        environment_ref: String::from(crate::PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF),
        current_phase: Some(current_status.phase),
        ready_for_demo: failures.is_empty(),
        has_status_packet,
        has_window_status_packet,
        has_checkpoint_surface,
        has_closeout_bundle: closeout.is_some(),
        final_loss_descended,
        checkpoint_ref: summary.latest_checkpoint_ref,
        failures,
        caveats,
    })
}

fn launch_manifest_surface_operation(
    launch_manifest: &PsionCs336A1DemoLaunchManifest,
) -> PsionicTrainOperation {
    if launch_manifest.surface_id == crate::PSION_CS336_A1_DEMO_REHEARSAL_SURFACE_ID {
        PsionicTrainOperation::RehearseBaseLane
    } else {
        PsionicTrainOperation::Start
    }
}

#[derive(Clone, Debug)]
struct ParsedStartLikeArgs {
    operation: PsionicTrainOperation,
    run_id: String,
    output_root: String,
    selected_git_ref: String,
    allow_dirty_tree: bool,
    dry_run: bool,
}

fn parse_start_like_args(
    command: &str,
    args: &[String],
) -> Result<ParsedStartLikeArgs, PsionCs336A1DemoOperatorError> {
    let mut run_id = None;
    let mut output_root = None;
    let mut selected_git_ref = Some(String::from("HEAD"));
    let mut allow_dirty_tree = false;
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run-id" => {
                index += 1;
                run_id = Some(args.get(index).cloned().ok_or_else(|| {
                    PsionCs336A1DemoOperatorError::MissingField(String::from("run-id"))
                })?);
            }
            "--output-root" => {
                index += 1;
                output_root = Some(args.get(index).cloned().ok_or_else(|| {
                    PsionCs336A1DemoOperatorError::MissingField(String::from("output-root"))
                })?);
            }
            "--git-ref" => {
                index += 1;
                selected_git_ref = Some(args.get(index).cloned().ok_or_else(|| {
                    PsionCs336A1DemoOperatorError::MissingField(String::from("git-ref"))
                })?);
            }
            "--allow-dirty-tree" => allow_dirty_tree = true,
            "--dry-run" => dry_run = true,
            other => {
                return Err(PsionCs336A1DemoOperatorError::UnsupportedOperation(
                    format!("unknown argument `{other}`"),
                ));
            }
        }
        index += 1;
    }

    let run_id = run_id.unwrap_or_else(default_run_id);
    let output_root = output_root.unwrap_or_else(|| default_output_root(run_id.as_str()));
    Ok(ParsedStartLikeArgs {
        operation: if command == "rehearse-base-lane" {
            PsionicTrainOperation::RehearseBaseLane
        } else {
            PsionicTrainOperation::Start
        },
        run_id,
        output_root,
        selected_git_ref: selected_git_ref.unwrap_or_else(|| String::from("HEAD")),
        allow_dirty_tree,
        dry_run,
    })
}

fn parse_run_root_args(
    args: &[String],
    command: &str,
) -> Result<String, PsionCs336A1DemoOperatorError> {
    let mut run_root = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run-root" => {
                index += 1;
                run_root = Some(args.get(index).cloned().ok_or_else(|| {
                    PsionCs336A1DemoOperatorError::MissingField(String::from("run-root"))
                })?);
            }
            other => {
                return Err(PsionCs336A1DemoOperatorError::UnsupportedOperation(
                    format!("unknown argument `{other}` for `{command}`"),
                ));
            }
        }
        index += 1;
    }
    run_root.ok_or_else(|| PsionCs336A1DemoOperatorError::MissingField(String::from("run-root")))
}

fn create_run_dirs(run_root: &Path) -> Result<(), std::io::Error> {
    for relative in [
        "manifests",
        "status",
        "checkpoints/manifests",
        "checkpoints/step-000004",
        "closeout",
        "logs",
    ] {
        fs::create_dir_all(run_root.join(relative))?;
    }
    Ok(())
}

fn write_json_pretty(
    path: &Path,
    value: &impl Serialize,
) -> Result<(), PsionCs336A1DemoOperatorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn load_json_file<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<T, PsionCs336A1DemoOperatorError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn append_log(path: &Path, line: &str) -> Result<(), PsionCs336A1DemoOperatorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut message = if path.is_file() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    message.push_str(line);
    message.push('\n');
    fs::write(path, message)?;
    Ok(())
}

fn write_runtime_packets(
    run_root: &Path,
    manifest: &PsionicTrainInvocationManifest,
    current_status: &PsionCs336A1DemoCurrentRunStatus,
    detail: &str,
) -> Result<(), PsionCs336A1DemoOperatorError> {
    let retained_paths = psion_cs336_a1_demo_retained_paths();
    let runtime_attestation = runtime_attestation_for_manifest(manifest)?;
    let capability_projection = PsionicTrainCapabilityProjection::for_lane(
        manifest.lane_id.as_str(),
        manifest.role,
        manifest.admission_identity.environment_ref.clone(),
    )?;
    let checkpoint_surface =
        inspect_psionic_train_checkpoint_surface(run_root, manifest.role, manifest.operation)
            .ok()
            .flatten();
    let artifacts = checkpoint_surface
        .as_ref()
        .map(|value| PsionicTrainArtifactSurfaceRefs {
            launch_manifest_path: Some(
                run_root
                    .join(&retained_paths.launch_manifest_path)
                    .display()
                    .to_string(),
            ),
            checkpoint_surface_path: Some(
                run_root
                    .join("status/checkpoint_surface.json")
                    .display()
                    .to_string(),
            ),
            checkpoint_pointer_path: value.artifacts.checkpoint_pointer_path.clone(),
            checkpoint_manifest_path: value.artifacts.checkpoint_manifest_path.clone(),
            checkpoint_backup_receipt_path: value.artifacts.checkpoint_backup_receipt_path.clone(),
            checkpoint_handoff_receipt_path: value
                .artifacts
                .peer_checkpoint_handoff_receipt_path
                .clone(),
            recovery_receipt_path: value.artifacts.auto_resume_receipt_path.clone(),
            final_closeout_bundle_path: run_root
                .join(&retained_paths.closeout_bundle_path)
                .is_file()
                .then(|| {
                    run_root
                        .join(&retained_paths.closeout_bundle_path)
                        .display()
                        .to_string()
                }),
            ..Default::default()
        })
        .unwrap_or_else(|| PsionicTrainArtifactSurfaceRefs {
            launch_manifest_path: Some(
                run_root
                    .join(&retained_paths.launch_manifest_path)
                    .display()
                    .to_string(),
            ),
            final_closeout_bundle_path: run_root
                .join(&retained_paths.closeout_bundle_path)
                .is_file()
                .then(|| {
                    run_root
                        .join(&retained_paths.closeout_bundle_path)
                        .display()
                        .to_string()
                }),
            ..Default::default()
        });
    let run_status_path = run_root.join("status/psionic_train_run_status_packet.json");
    let window_status_path = run_root.join("status/psionic_train_window_status_packet.json");
    let run_packet = PsionicTrainRunStatusPacket {
        schema_version: String::from(PSIONIC_TRAIN_RUN_STATUS_PACKET_SCHEMA_VERSION),
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
        coordination: manifest.coordination.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        validator_target_work_class: manifest.validator_target_work_class,
        manifest_path: Some(
            run_root
                .join(&retained_paths.launch_manifest_path)
                .display()
                .to_string(),
        ),
        manifest_digest: manifest.manifest_digest.clone(),
        run_id: manifest.run_id.clone(),
        run_root: Some(run_root.display().to_string()),
        phase: Some(current_status.phase.clone()),
        runtime_attestation: runtime_attestation.clone(),
        capability_projection: capability_projection.clone(),
        artifacts: artifacts.clone(),
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
        detail: String::from(detail),
    };
    let window_packet = PsionicTrainWindowStatusPacket {
        schema_version: String::from(PSIONIC_TRAIN_WINDOW_STATUS_PACKET_SCHEMA_VERSION),
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
        coordination: manifest.coordination.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        validator_target_work_class: manifest.validator_target_work_class,
        manifest_digest: manifest.manifest_digest.clone(),
        run_id: manifest.run_id.clone(),
        run_root: Some(run_root.display().to_string()),
        window_state: manifest
            .coordination
            .window_id
            .as_ref()
            .map(|_| current_status.phase.clone()),
        runtime_attestation,
        capability_projection,
        artifacts,
        detail: if manifest.coordination.window_id.is_some() {
            String::from(detail)
        } else {
            format!(
                "{detail}; this admitted lane does not yet materialize dedicated sealed-window artifacts"
            )
        },
    };
    write_json_pretty(&run_status_path, &run_packet)?;
    write_json_pretty(&window_status_path, &window_packet)?;
    Ok(())
}

fn runtime_attestation_for_manifest(
    manifest: &PsionicTrainInvocationManifest,
) -> Result<PsionicTrainRuntimeAttestation, PsionCs336A1DemoOperatorError> {
    let packaged_revision = packaged_runtime_revision(repo_root().as_path())?;
    let git_commit_sha = packaged_revision
        .clone()
        .or_else(|| git_stdout(["rev-parse", "HEAD"]))
        .unwrap_or_else(|| String::from("unknown_git_commit"));
    let dirty_tree_admission = if packaged_revision.is_some() {
        String::from(PSIONIC_TRAIN_PACKAGED_RUNTIME_DIRTY_TREE_ADMISSION)
    } else if manifest.allow_dirty_tree {
        String::from("allowed_by_operator_override")
    } else {
        String::from("refuse_by_default")
    };
    let workspace_status_sha256 = if packaged_revision.is_some() {
        None
    } else if manifest.allow_dirty_tree {
        git_stdout(["status", "--porcelain=v1"])
            .map(|value| sha256_hex(value.as_bytes()))
            .or_else(|| Some(sha256_hex(b"dirty_tree_status_unavailable")))
    } else {
        None
    };
    Ok(PsionicTrainRuntimeAttestation {
        schema_version: String::from(PSIONIC_TRAIN_RUNTIME_ATTESTATION_SCHEMA_VERSION),
        release_id: manifest.admission_identity.release_id.clone(),
        build_digest: manifest.admission_identity.build_digest.clone(),
        git_commit_sha,
        dirty_tree_admission,
        workspace_status_sha256,
        environment_ref: manifest.admission_identity.environment_ref.clone(),
    })
}

fn git_stdout<const N: usize>(args: [&str; N]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| String::from(trimmed))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn default_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    format!("psion-cs336-a1-demo-{millis}")
}

fn default_output_root(run_id: &str) -> String {
    let base = env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root().join("scratch"));
    base.join("scratch/psion_cs336_a1_demo_runs")
        .join(run_id)
        .display()
        .to_string()
}

fn repo_root() -> PathBuf {
    if let Ok(value) = env::var(PSIONIC_TRAIN_RUNTIME_ROOT_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            if let Ok(root) = PathBuf::from(trimmed).canonicalize() {
                return root;
            }
        }
    }
    if let Ok(current_exe) = env::current_exe() {
        for ancestor in current_exe.ancestors().skip(1) {
            let root = ancestor.to_path_buf();
            if packaged_runtime_revision(root.as_path()).is_ok_and(|revision| revision.is_some())
                || root
                    .join(crate::CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH)
                    .is_file()
            {
                if let Ok(root) = root.canonicalize() {
                    return root;
                }
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn packaged_runtime_revision(root: &Path) -> Result<Option<String>, PsionCs336A1DemoOperatorError> {
    let revision_path = root.join(PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE);
    if !revision_path.exists() {
        return Ok(None);
    }
    let revision = fs::read_to_string(revision_path.as_path())?;
    let revision = revision.trim();
    if revision.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} is empty", revision_path.display()),
        )
        .into());
    }
    Ok(Some(revision.to_string()))
}

fn print_usage() {
    println!(
        "Usage:\n  psionic-train cs336-a1-demo start [--run-id <id>] [--output-root <path>] [--git-ref <ref>] [--allow-dirty-tree] [--dry-run]\n  psionic-train cs336-a1-demo rehearse-base-lane [--run-id <id>] [--output-root <path>] [--git-ref <ref>] [--allow-dirty-tree] [--dry-run]\n  psionic-train cs336-a1-demo status --run-root <path>\n  psionic-train cs336-a1-demo verify --run-root <path>\n\nThis is the bounded packaged CS336 A1 demo lane. It always uses the admitted tiny corpus and the fixed four-step training budget."
    );
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_CS336_A1_DEMO_CLOSEOUT_BUNDLE_SCHEMA_VERSION, PsionCs336A1DemoCloseoutBundle,
        run_psion_cs336_a1_demo_manifest, verify_run,
    };
    use crate::{
        PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
        PsionCs336A1DemoAutomaticExecutionRequest, PsionicTrainCoordinationContext,
        PsionicTrainOperation, PsionicTrainRole,
    };
    use tempfile::tempdir;

    fn request(output_root: &str) -> PsionCs336A1DemoAutomaticExecutionRequest {
        PsionCs336A1DemoAutomaticExecutionRequest {
            schema_version: String::from(
                PSION_CS336_A1_DEMO_AUTOMATIC_EXECUTION_REQUEST_SCHEMA_VERSION,
            ),
            role: PsionicTrainRole::Worker,
            operation: PsionicTrainOperation::Start,
            coordination: PsionicTrainCoordinationContext {
                node_pubkey: Some(String::from("npub1-demo-runner")),
                ..Default::default()
            },
            build_digest: String::from("sha256:test-build"),
            run_id: String::from("psion-cs336-a1-demo-test"),
            output_root: Some(String::from(output_root)),
            run_root: None,
            selected_git_ref: String::from("HEAD"),
            allow_dirty_tree: false,
            dry_run: false,
        }
    }

    #[test]
    fn packaged_demo_manifest_writes_checkpoint_and_closeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let run_root = temp.path().join("run");
        let manifest = request(run_root.to_str().expect("temp run root should be utf8"))
            .to_invocation_manifest()?;
        run_psion_cs336_a1_demo_manifest(&manifest)?;
        let surface = crate::inspect_psionic_train_checkpoint_surface(
            &run_root,
            manifest.role,
            manifest.operation,
        )?
        .expect("checkpoint surface should exist after a real bounded run");
        assert_eq!(surface.pointer_state.as_deref(), Some("accepted"));
        let closeout: PsionCs336A1DemoCloseoutBundle = serde_json::from_slice(&std::fs::read(
            run_root.join("closeout/closeout_bundle.json"),
        )?)?;
        assert_eq!(
            closeout.schema_version,
            PSION_CS336_A1_DEMO_CLOSEOUT_BUNDLE_SCHEMA_VERSION
        );
        assert!(
            closeout.final_loss < closeout.initial_loss,
            "the bounded demo run should descend over four steps"
        );
        Ok(())
    }

    #[test]
    fn packaged_demo_dry_run_skips_checkpoint_and_closeout()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let run_root = temp.path().join("run");
        let mut request = request(run_root.to_str().expect("temp run root should be utf8"));
        request.dry_run = true;
        let manifest = request.to_invocation_manifest()?;
        run_psion_cs336_a1_demo_manifest(&manifest)?;
        assert!(
            !run_root
                .join("checkpoints/latest_accepted_checkpoint_pointer.json")
                .is_file()
        );
        assert!(!run_root.join("closeout/closeout_bundle.json").is_file());
        Ok(())
    }

    #[test]
    fn packaged_demo_verify_reports_success_for_real_run() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let run_root = temp.path().join("run");
        let manifest = request(run_root.to_str().expect("temp run root should be utf8"))
            .to_invocation_manifest()?;
        run_psion_cs336_a1_demo_manifest(&manifest)?;
        let report = verify_run(&run_root)?;
        assert!(
            report.ready_for_demo,
            "fresh bounded run should verify cleanly"
        );
        assert!(report.failures.is_empty());
        assert!(report.has_checkpoint_surface);
        assert!(report.has_closeout_bundle);
        assert!(report.final_loss_descended);
        Ok(())
    }

    #[test]
    fn packaged_demo_verify_rejects_dry_run_as_demo_ready() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let run_root = temp.path().join("run");
        let mut request = request(run_root.to_str().expect("temp run root should be utf8"));
        request.dry_run = true;
        let manifest = request.to_invocation_manifest()?;
        run_psion_cs336_a1_demo_manifest(&manifest)?;
        let report = verify_run(&run_root)?;
        assert!(
            !report.ready_for_demo,
            "dry-run surfaces should not count as a demo-valid bounded execution"
        );
        assert!(
            report
                .failures
                .iter()
                .any(|value| value.contains("dry_run")),
            "verification report should explain why the dry run is insufficient"
        );
        Ok(())
    }
}
