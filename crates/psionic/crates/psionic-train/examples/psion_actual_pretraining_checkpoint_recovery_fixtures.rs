use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    record_psion_actual_pretraining_auto_resume_receipt,
    record_psion_actual_pretraining_checkpoint_backup_receipt,
    record_psion_actual_pretraining_checkpoint_failure_drill,
    record_psion_actual_pretraining_checkpoint_manifest, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingCheckpointPointer, PsionActualPretrainingDataBundle,
    PsionActualPretrainingSystemsBundle, PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_LANE_ID,
};
use sha2::{Digest, Sha256};

const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;
    let data_bundle: PsionActualPretrainingDataBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"),
    )?;
    data_bundle.validate()?;
    let systems_bundle: PsionActualPretrainingSystemsBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json"),
    )?;
    systems_bundle.validate()?;

    let healthy_run_id = "run-psion-actual-20260402t030000z";
    let healthy_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_recovery_example")
        .join("healthy")
        .join(healthy_run_id);
    let healthy_manifest = write_accepted_checkpoint_bundle(
        &healthy_run_root,
        healthy_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
        systems_bundle.memory_qualification.checkpoint_total_bytes,
    )?;
    let healthy_pointer: PsionActualPretrainingCheckpointPointer =
        load_json(&healthy_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"))?;
    let healthy_backup = write_backup_bundle(
        &healthy_run_root,
        &healthy_pointer,
        &healthy_manifest,
        false,
    )?;
    let healthy_auto_resume = record_psion_actual_pretraining_auto_resume_receipt(
        healthy_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "accepted",
        "accepted_primary_pointer",
        "primary_pointer",
        false,
        Some(String::from("broader-pretrain-final")),
        Some(16384),
        Some(String::from("checkpoint://psion/broad/pretrain/final")),
        Some(run_artifact_ref(
            &healthy_run_root,
            &healthy_run_root.join(&healthy_manifest.relative_manifest_path),
        )?),
        None,
        "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
        "Fixture auto-resume receipt accepts the primary retained pointer for the healthy run root.",
    )?;
    write_json_pretty(
        &healthy_run_root.join("checkpoints/auto_resume_receipt.json"),
        &healthy_auto_resume,
    )?;

    let corrupt_run_id = "run-psion-actual-20260402t040000z";
    let corrupt_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_recovery_example")
        .join("corrupt_pointer")
        .join(corrupt_run_id);
    let corrupt_manifest = write_accepted_checkpoint_bundle(
        &corrupt_run_root,
        corrupt_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
        systems_bundle.memory_qualification.checkpoint_total_bytes,
    )?;
    let corrupt_pointer: PsionActualPretrainingCheckpointPointer =
        load_json(&corrupt_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"))?;
    write_backup_bundle(&corrupt_run_root, &corrupt_pointer, &corrupt_manifest, false)?;
    fs::write(
        corrupt_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        "{ this is not valid json\n",
    )?;
    let corrupt_auto_resume = record_psion_actual_pretraining_auto_resume_receipt(
        corrupt_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "corrupt",
        "recovered_from_backup",
        "backup_receipt",
        true,
        Some(String::from("broader-pretrain-final")),
        Some(16384),
        Some(String::from("checkpoint://psion/broad/pretrain/final")),
        Some(PsionActualPretrainingArtifactRef {
            path: String::from("checkpoints/step-16384/checkpoint_manifest.json"),
            sha256: healthy_backup.primary_checkpoint_manifest.sha256.clone(),
        }),
        None,
        "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
        "Fixture auto-resume receipt proves corrupt primary pointers can recover from the retained backup family.",
    )?;
    write_json_pretty(
        &corrupt_run_root.join("checkpoints/auto_resume_receipt.json"),
        &corrupt_auto_resume,
    )?;
    let corrupt_drill = record_psion_actual_pretraining_checkpoint_failure_drill(
        corrupt_run_id,
        "psion_actual_pretraining_checkpoint_failure_drill::16384::corrupt_pointer",
        "corrupt_pointer",
        "resume",
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "recovered_without_manual_edit",
        vec![
            String::from("checkpoints/auto_resume_receipt.json"),
            String::from("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
            String::from("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
            String::from("checkpoints/backups/step-16384/checkpoint_manifest.backup.json"),
        ],
        None,
        "This retained failure drill proves that corrupt primary resume pointers recover from the actual-lane backup family without manual editing.",
        "Fixture corrupt-pointer drill records recovery from the backup family after the primary pointer became invalid JSON.",
    )?;
    write_json_pretty(
        &corrupt_run_root.join("checkpoints/failures/corrupt_pointer_drill.json"),
        &corrupt_drill,
    )?;

    let stale_run_id = "run-psion-actual-20260402t050000z";
    let stale_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_recovery_example")
        .join("stale_pointer")
        .join(stale_run_id);
    let stale_manifest = write_accepted_checkpoint_bundle(
        &stale_run_root,
        stale_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
        systems_bundle.memory_qualification.checkpoint_total_bytes,
    )?;
    let mut stale_pointer: PsionActualPretrainingCheckpointPointer =
        load_json(&stale_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"))?;
    write_backup_bundle(&stale_run_root, &stale_pointer, &stale_manifest, false)?;
    stale_pointer.checkpoint_manifest_relative_path =
        Some(String::from("checkpoints/step-16384/missing_manifest.json"));
    write_json_pretty(
        &stale_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        &stale_pointer,
    )?;
    let stale_auto_resume = record_psion_actual_pretraining_auto_resume_receipt(
        stale_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "stale",
        "recovered_from_backup",
        "backup_receipt",
        true,
        Some(String::from("broader-pretrain-final")),
        Some(16384),
        Some(String::from("checkpoint://psion/broad/pretrain/final")),
        Some(PsionActualPretrainingArtifactRef {
            path: String::from("checkpoints/step-16384/checkpoint_manifest.json"),
            sha256: healthy_backup.primary_checkpoint_manifest.sha256.clone(),
        }),
        None,
        "The actual-lane auto-resume receipt records whether resume trusted the primary pointer or had to recover from the retained backup family. It does not claim that preflight admission or post-resume training succeeded.",
        "Fixture auto-resume receipt proves stale primary pointers can recover from the retained backup family.",
    )?;
    write_json_pretty(
        &stale_run_root.join("checkpoints/auto_resume_receipt.json"),
        &stale_auto_resume,
    )?;
    let stale_drill = record_psion_actual_pretraining_checkpoint_failure_drill(
        stale_run_id,
        "psion_actual_pretraining_checkpoint_failure_drill::16384::stale_pointer",
        "stale_pointer",
        "resume",
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "recovered_without_manual_edit",
        vec![
            String::from("checkpoints/auto_resume_receipt.json"),
            String::from("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
            String::from("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
            String::from("checkpoints/backups/step-16384/checkpoint_manifest.backup.json"),
        ],
        None,
        "This retained failure drill proves that stale primary resume pointers recover from the actual-lane backup family without manual editing.",
        "Fixture stale-pointer drill records recovery from the backup family after the primary pointer drifted from the admitted manifest path.",
    )?;
    write_json_pretty(
        &stale_run_root.join("checkpoints/failures/stale_pointer_drill.json"),
        &stale_drill,
    )?;

    let failed_upload_run_id = "run-psion-actual-20260402t060000z";
    let failed_upload_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_recovery_example")
        .join("failed_upload")
        .join(failed_upload_run_id);
    let failed_upload_manifest = write_accepted_checkpoint_bundle(
        &failed_upload_run_root,
        failed_upload_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
        systems_bundle.memory_qualification.checkpoint_total_bytes,
    )?;
    let failed_upload_pointer: PsionActualPretrainingCheckpointPointer = load_json(
        &failed_upload_run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
    )?;
    let _failed_upload_backup = write_backup_bundle(
        &failed_upload_run_root,
        &failed_upload_pointer,
        &failed_upload_manifest,
        true,
    )?;
    let failed_upload_drill = record_psion_actual_pretraining_checkpoint_failure_drill(
        failed_upload_run_id,
        "psion_actual_pretraining_checkpoint_failure_drill::16384::failed_upload",
        "failed_upload",
        "backup",
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "retained_refusal",
        vec![
            String::from("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
            String::from("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json"),
            String::from("checkpoints/backups/step-16384/checkpoint_manifest.backup.json"),
        ],
        Some(String::from(
            "Injected failed checkpoint upload drill refused durable remote backup confirmation without copying any secret payload into retained evidence.",
        )),
        "This retained failure drill proves that checkpoint-upload failures surface as explicit refusal evidence under the actual-lane family rather than silent launcher optimism.",
        "Fixture failed-upload drill records refusal evidence while preserving the local backup copies.",
    )?;
    write_json_pretty(
        &failed_upload_run_root.join("checkpoints/failures/failed_upload_drill.json"),
        &failed_upload_drill,
    )?;

    write_json_pretty(
        &fixtures_dir.join("psion_actual_pretraining_checkpoint_manifest_v1.json"),
        &healthy_manifest,
    )?;
    write_json_pretty(
        &fixtures_dir.join("psion_actual_pretraining_checkpoint_backup_receipt_v1.json"),
        &healthy_backup,
    )?;
    write_json_pretty(
        &fixtures_dir.join("psion_actual_pretraining_auto_resume_receipt_v1.json"),
        &corrupt_auto_resume,
    )?;
    write_json_pretty(
        &fixtures_dir.join(
            "psion_actual_pretraining_checkpoint_failure_drill_failed_upload_v1.json",
        ),
        &failed_upload_drill,
    )?;
    write_json_pretty(
        &fixtures_dir.join(
            "psion_actual_pretraining_checkpoint_failure_drill_corrupt_pointer_v1.json",
        ),
        &corrupt_drill,
    )?;
    write_json_pretty(
        &fixtures_dir.join(
            "psion_actual_pretraining_checkpoint_failure_drill_stale_pointer_v1.json",
        ),
        &stale_drill,
    )?;
    Ok(())
}

fn write_accepted_checkpoint_bundle(
    run_root: &Path,
    run_id: &str,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    dataset_identity: &str,
    checkpoint_total_bytes: u64,
) -> Result<psionic_train::PsionActualPretrainingCheckpointManifest, Box<dyn Error>> {
    let checkpoint_manifest = record_psion_actual_pretraining_checkpoint_manifest(
        run_id,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        &sha256_hex(format!("{run_id}|{checkpoint_label}|{optimizer_step}|{checkpoint_ref}").as_bytes()),
        checkpoint_total_bytes,
        dataset_identity,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "Fixture checkpoint manifest records one accepted actual-lane checkpoint under the retained evidence family.",
    )?;
    write_json_pretty(
        &run_root.join(&checkpoint_manifest.relative_manifest_path),
        &checkpoint_manifest,
    )?;
    let pointer = PsionActualPretrainingCheckpointPointer {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        run_id: String::from(run_id),
        pointer_state: String::from("accepted"),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: Some(String::from(checkpoint_ref)),
        checkpoint_manifest_relative_path: Some(checkpoint_manifest.relative_manifest_path.clone()),
        detail: String::from(
            "Fixture accepted checkpoint pointer binds resume to the retained checkpoint manifest.",
        ),
    };
    pointer.validate()?;
    write_json_pretty(
        &run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        &pointer,
    )?;
    Ok(checkpoint_manifest)
}

fn write_backup_bundle(
    run_root: &Path,
    pointer: &PsionActualPretrainingCheckpointPointer,
    manifest: &psionic_train::PsionActualPretrainingCheckpointManifest,
    inject_failed_upload: bool,
) -> Result<psionic_train::PsionActualPretrainingCheckpointBackupReceipt, Box<dyn Error>> {
    let backup_pointer_path =
        run_root.join("checkpoints/backups/latest_accepted_checkpoint_pointer.backup.json");
    let backup_manifest_path =
        run_root.join("checkpoints/backups/step-16384/checkpoint_manifest.backup.json");
    write_json_pretty(&backup_pointer_path, pointer)?;
    write_json_pretty(&backup_manifest_path, manifest)?;
    let receipt = record_psion_actual_pretraining_checkpoint_backup_receipt(
        &pointer.run_id,
        &pointer.checkpoint_label,
        pointer.optimizer_step,
        pointer
            .checkpoint_ref
            .as_deref()
            .ok_or_else(|| std::io::Error::other("accepted fixture pointer is missing checkpoint_ref"))?,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        run_artifact_ref(
            run_root,
            &run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json"),
        )?,
        run_artifact_ref(run_root, &run_root.join(&manifest.relative_manifest_path))?,
        run_artifact_ref(run_root, &backup_pointer_path)?,
        run_artifact_ref(run_root, &backup_manifest_path)?,
        &format!(
            "${{PSION_ACTUAL_PRETRAINING_BUCKET_URL}}/psion_actual_pretraining_runs/{}/checkpoints/backups",
            pointer.run_id
        ),
        vec![
            String::from("PSION_ACTUAL_PRETRAINING_BUCKET_URL"),
            String::from("PSION_ACTUAL_PRETRAINING_ACCESS_KEY"),
            String::from("PSION_ACTUAL_PRETRAINING_SECRET_FILE"),
        ],
        if inject_failed_upload {
            "refused"
        } else {
            "backed_up"
        },
        if inject_failed_upload {
            "failed"
        } else {
            "succeeded"
        },
        inject_failed_upload.then(|| {
            String::from(
                "Injected failed checkpoint upload drill refused durable remote backup confirmation without copying any secret payload into retained evidence.",
            )
        }),
        "This retained backup receipt binds the actual-lane latest accepted checkpoint to one durable backup contract and redacted credential-source posture. It does not claim that training continued or that automatic checkpoint eval already ran.",
        "Fixture checkpoint backup receipt preserves the accepted pointer plus checkpoint manifest under one local backup family and one redacted remote-backup root.",
    )?;
    write_json_pretty(
        &run_root.join("checkpoints/latest_accepted_checkpoint_backup_receipt.json"),
        &receipt,
    )?;
    Ok(receipt)
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

fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn run_artifact_ref(run_root: &Path, path: &Path) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative = path
        .strip_prefix(run_root)?
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

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}
