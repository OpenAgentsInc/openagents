use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH;
use psionic_train::{
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_COMPARISON_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CONTINUE_RESTART_DECISION_PATH,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingBaselineToolsBundle,
    PsionActualPretrainingCheckpointPointer, PsionActualPretrainingDataBundle,
    PsionActualPretrainingEvidenceContract, PsionActualPretrainingHardwareObservation,
    PsionActualPretrainingRecipeBundle, PsionActualPretrainingRunShapeObservation,
    PsionActualPretrainingSystemsBundle, PsionActualPretrainingTopologyStorageBundle,
    checkpoint_comparison_relative_path, checkpoint_eval_decision_relative_path,
    continue_restart_decision_relative_path,
    derive_psion_actual_pretraining_hardware_qualification,
    derive_psion_actual_pretraining_run_shape_qualification,
    record_psion_actual_pretraining_checkpoint_backup_receipt,
    record_psion_actual_pretraining_checkpoint_comparison,
    record_psion_actual_pretraining_checkpoint_eval_decision,
    record_psion_actual_pretraining_checkpoint_manifest,
    record_psion_actual_pretraining_continue_restart_decision,
};
use sha2::{Digest, Sha256};

const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let recipe_bundle: PsionActualPretrainingRecipeBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json"),
    )?;
    recipe_bundle.validate()?;
    let baseline_tools_bundle: PsionActualPretrainingBaselineToolsBundle = load_json(
        &root
            .join("fixtures/psion/pretrain/psion_actual_pretraining_baseline_tools_bundle_v1.json"),
    )?;
    baseline_tools_bundle.validate()?;
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

    let run_id = "run-psion-actual-20260402t090000z";
    let run_root = fixtures_dir
        .join("psion_actual_pretraining_continue_restart_example")
        .join("continue")
        .join(run_id);
    fs::create_dir_all(&run_root)?;

    let hardware_qualification = derive_psion_actual_pretraining_hardware_qualification(
        run_id,
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
            &root.join(
                "fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json",
            ),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        artifact_ref(
            &root,
            &root.join(
                "fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json",
            ),
        )?,
        &topology,
        &systems_bundle,
        &evidence_contract,
    )?;
    write_json_pretty(
        &run_root.join("preflight/hardware_qualification.json"),
        &hardware_qualification,
    )?;

    let run_shape_qualification = derive_psion_actual_pretraining_run_shape_qualification(
        run_id,
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
            &root.join(
                "fixtures/psion/pretrain/psion_actual_pretraining_evidence_contract_v1.json",
            ),
        )?,
        &baseline_tools_bundle,
        &data_bundle,
        &systems_bundle,
        &evidence_contract,
    )?;
    write_json_pretty(
        &run_root.join("preflight/run_shape_qualification.json"),
        &run_shape_qualification,
    )?;

    let checkpoint_label = "broader-pretrain-final";
    let optimizer_step = 16_384;
    let checkpoint_ref = "checkpoint://psion/broad/pretrain/final";
    let checkpoint_manifest = record_psion_actual_pretraining_checkpoint_manifest(
        run_id,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        &sha256_hex(
            format!("{run_id}|{checkpoint_label}|{optimizer_step}|{checkpoint_ref}").as_bytes(),
        ),
        systems_bundle.memory_qualification.checkpoint_total_bytes,
        &data_bundle.replay_authority.dataset_identity,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "Checkpoint manifest records one accepted actual-lane checkpoint inside the frozen evidence family without claiming that the broader distributed training job ran inside this fixture generator.",
    )?;
    write_json_pretty(
        &run_root.join(&checkpoint_manifest.relative_manifest_path),
        &checkpoint_manifest,
    )?;
    let checkpoint_pointer = PsionActualPretrainingCheckpointPointer {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        pointer_state: String::from("accepted"),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: Some(String::from(checkpoint_ref)),
        checkpoint_manifest_relative_path: Some(checkpoint_manifest.relative_manifest_path.clone()),
        detail: String::from(
            "Accepted checkpoint pointer binds actual-lane resume to the latest admitted checkpoint manifest.",
        ),
    };
    checkpoint_pointer.validate()?;
    write_json_pretty(
        &run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        &checkpoint_pointer,
    )?;
    write_json_pretty(
        &run_root.join("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
        &checkpoint_pointer,
    )?;
    write_json_pretty(
        &run_root.join(format!(
            "checkpoints/backups/step-{optimizer_step}/checkpoint_manifest.backup.json"
        )),
        &checkpoint_manifest,
    )?;

    let checkpoint_manifest_artifact = run_artifact_ref(
        &run_root,
        &run_root.join(&checkpoint_manifest.relative_manifest_path),
    )?;
    let checkpoint_pointer_artifact = run_artifact_ref(
        &run_root,
        &run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
    )?;
    let checkpoint_backup_receipt = record_psion_actual_pretraining_checkpoint_backup_receipt(
        run_id,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        checkpoint_pointer_artifact.clone(),
        checkpoint_manifest_artifact.clone(),
        run_artifact_ref(
            &run_root,
            &run_root.join("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
        )?,
        run_artifact_ref(
            &run_root,
            &run_root.join(format!(
                "checkpoints/backups/step-{optimizer_step}/checkpoint_manifest.backup.json"
            )),
        )?,
        &format!(
            "{}/backups",
            topology
                .remote_checkpoint_root_template
                .replace("<run_id>", run_id)
        ),
        topology
            .credential_sources
            .iter()
            .map(|source| source.source_name.clone())
            .collect(),
        "backed_up",
        "succeeded",
        None,
        "This retained backup receipt binds the actual-lane latest accepted checkpoint to one durable backup contract and redacted credential-source posture. It does not claim that training continued or that automatic checkpoint eval already ran.",
        "Fixture checkpoint backup receipt preserves the accepted pointer plus checkpoint manifest under one local backup family and one redacted remote-backup root.",
    )?;
    write_json_pretty(
        &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
        &checkpoint_backup_receipt,
    )?;

    let benchmark_fixture_ref = artifact_ref(
        &root,
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH),
    )?;
    let checkpoint_eval_decision = record_psion_actual_pretraining_checkpoint_eval_decision(
        run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        checkpoint_manifest_artifact.clone(),
        benchmark_fixture_ref,
        &data_bundle,
        "This retained checkpoint-eval decision binds one accepted actual-lane checkpoint to the frozen checkpoint review pack and the frozen benchmark families already attached to the actual-lane data bundle. It is the automatic checkpoint review surface for later continue-vs-restart logic. It does not claim distributed broader-pretraining closure, dashboard fan-out, or final promotion review.",
        "Fixture checkpoint eval decision records one automatic retained review over the accepted checkpoint using the canonical actual-lane benchmark pack.",
    )?;
    write_json_pretty(
        &run_root.join(checkpoint_eval_decision_relative_path(optimizer_step)),
        &checkpoint_eval_decision,
    )?;
    write_json_pretty(
        &run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH),
        &checkpoint_eval_decision,
    )?;

    let checkpoint_comparison = record_psion_actual_pretraining_checkpoint_comparison(
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        checkpoint_pointer_artifact,
        &checkpoint_pointer,
        checkpoint_manifest_artifact,
        Some(run_artifact_ref(
            &run_root,
            &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
        )?),
        Some(&checkpoint_backup_receipt),
        Some(run_artifact_ref(
            &run_root,
            &run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH),
        )?),
        Some(&checkpoint_eval_decision),
        None,
        None,
        run_artifact_ref(
            &run_root,
            &run_root.join("preflight/hardware_qualification.json"),
        )?,
        &hardware_qualification,
        run_artifact_ref(
            &run_root,
            &run_root.join("preflight/run_shape_qualification.json"),
        )?,
        &run_shape_qualification,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        &systems_bundle,
        "This retained checkpoint comparison binds the latest accepted checkpoint to the frozen checkpoint-eval, backup, hardware, run-shape, and systems receipts before the actual lane decides whether to continue, hold, or restart. It does not claim that the operator already performed the chosen action.",
        "Checkpoint comparison records the explicit continue threshold against the trusted-cluster throughput anchor and the retained actual-lane checkpoint lineage.",
    )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_FIXTURE_PATH),
        &checkpoint_comparison,
    )?;
    write_json_pretty(
        &run_root.join(checkpoint_comparison_relative_path(optimizer_step)),
        &checkpoint_comparison,
    )?;
    write_json_pretty(
        &run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_COMPARISON_PATH),
        &checkpoint_comparison,
    )?;

    let continue_restart_decision = record_psion_actual_pretraining_continue_restart_decision(
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        &checkpoint_pointer,
        run_artifact_ref(
            &run_root,
            &run_root.join(checkpoint_comparison_relative_path(optimizer_step)),
        )?,
        &checkpoint_comparison,
        Some(run_artifact_ref(
            &run_root,
            &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
        )?),
        Some(run_artifact_ref(
            &run_root,
            &run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH),
        )?),
        Some(&checkpoint_eval_decision),
        None,
        None,
        run_artifact_ref(
            &run_root,
            &run_root.join("preflight/hardware_qualification.json"),
        )?,
        run_artifact_ref(
            &run_root,
            &run_root.join("preflight/run_shape_qualification.json"),
        )?,
        artifact_ref(
            &root,
            &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
        )?,
        "This retained continue-restart decision keeps long-run operator posture machine-readable under the actual-lane evidence family. It does not claim that the operator already restarted or continued the cluster run; it only records the bounded next action.",
        "Fixture continue-restart decision consumes retained eval, backup, hardware, run-shape, and systems evidence before the operator chooses the next long-run action.",
    )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_FIXTURE_PATH),
        &continue_restart_decision,
    )?;
    write_json_pretty(
        &run_root.join(continue_restart_decision_relative_path(optimizer_step)),
        &continue_restart_decision,
    )?;
    write_json_pretty(
        &run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CONTINUE_RESTART_DECISION_PATH),
        &continue_restart_decision,
    )?;

    println!(
        "wrote {} and {}",
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_COMPARISON_FIXTURE_PATH,
        PSION_ACTUAL_PRETRAINING_CONTINUE_RESTART_DECISION_FIXTURE_PATH
    );
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

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn artifact_ref(
    repo_root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative_path = path.strip_prefix(repo_root)?.to_string_lossy().to_string();
    let bytes = fs::read(path)?;
    Ok(PsionActualPretrainingArtifactRef {
        path: relative_path,
        sha256: sha256_hex(&bytes),
    })
}

fn run_artifact_ref(
    run_root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative_path = path.strip_prefix(run_root)?.to_string_lossy().to_string();
    let bytes = fs::read(path)?;
    Ok(PsionActualPretrainingArtifactRef {
        path: relative_path,
        sha256: sha256_hex(&bytes),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
