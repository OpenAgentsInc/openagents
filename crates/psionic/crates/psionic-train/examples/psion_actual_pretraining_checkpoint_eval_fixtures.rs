use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH,
    build_psion_actual_pretraining_checkpoint_eval_benchmark_package,
};
use psionic_train::{
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_FAILURE_PATH,
    PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH,
    PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_FIXTURE_PATH, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingCheckpointManifest, PsionActualPretrainingDataBundle,
    checkpoint_eval_decision_relative_path, checkpoint_eval_failure_relative_path,
    record_psion_actual_pretraining_checkpoint_eval_decision,
    record_psion_actual_pretraining_checkpoint_eval_failure,
    record_psion_actual_pretraining_checkpoint_manifest,
    record_psion_actual_pretraining_redacted_alert,
};
use sha2::{Digest, Sha256};

const FIXTURE_GIT_SHA: &str = "1111222233334444555566667777888899990000";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let benchmark_package = build_psion_actual_pretraining_checkpoint_eval_benchmark_package()?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH),
        &benchmark_package,
    )?;
    let benchmark_fixture_ref = artifact_ref(
        &root,
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH),
    )?;

    let data_bundle: PsionActualPretrainingDataBundle = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json"),
    )?;
    data_bundle.validate()?;

    let success_run_id = "run-psion-actual-20260402t070000z";
    let success_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_eval_example")
        .join("success")
        .join(success_run_id);
    let success_manifest = write_checkpoint_manifest(
        &success_run_root,
        success_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
    )?;
    let success_manifest_ref = run_artifact_ref(
        &success_run_root,
        &success_run_root.join(&success_manifest.relative_manifest_path),
    )?;
    let decision = record_psion_actual_pretraining_checkpoint_eval_decision(
        success_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        success_manifest_ref,
        benchmark_fixture_ref.clone(),
        &data_bundle,
        "This retained checkpoint-eval decision binds one accepted actual-lane checkpoint to the frozen checkpoint review pack and the frozen benchmark families already attached to the actual-lane data bundle. It is the automatic checkpoint review surface for later continue-vs-restart logic. It does not claim distributed broader-pretraining closure, dashboard fan-out, or final promotion review.",
        "Fixture checkpoint eval decision records one automatic retained review over the accepted checkpoint using the canonical actual-lane benchmark pack.",
    )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_FIXTURE_PATH),
        &decision,
    )?;
    write_json_pretty(
        &success_run_root.join(checkpoint_eval_decision_relative_path(16384)),
        &decision,
    )?;
    write_json_pretty(
        &success_run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_DECISION_PATH),
        &decision,
    )?;

    let failure_run_id = "run-psion-actual-20260402t080000z";
    let failure_run_root = fixtures_dir
        .join("psion_actual_pretraining_checkpoint_eval_example")
        .join("worker_unavailable")
        .join(failure_run_id);
    let failure_manifest = write_checkpoint_manifest(
        &failure_run_root,
        failure_run_id,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        &data_bundle.replay_authority.dataset_identity,
    )?;
    let failure_manifest_ref = run_artifact_ref(
        &failure_run_root,
        &failure_run_root.join(&failure_manifest.relative_manifest_path),
    )?;
    let failure = record_psion_actual_pretraining_checkpoint_eval_failure(
        failure_run_id,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "broader-pretrain-final",
        16384,
        "checkpoint://psion/broad/pretrain/final",
        failure_manifest_ref,
        benchmark_fixture_ref,
        "eval_worker_unavailable",
        "This retained checkpoint-eval failure proves the actual-lane operator path does not silently skip automatic checkpoint review when the eval worker is unavailable. It retains an explicit retry requirement and a redacted alert instead.",
        "Fixture checkpoint eval failure records that the automatic checkpoint-review worker was unavailable after the accepted checkpoint entered the retained backup family.",
    )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_FIXTURE_PATH),
        &failure,
    )?;
    write_json_pretty(
        &failure_run_root.join(checkpoint_eval_failure_relative_path(16384)),
        &failure,
    )?;
    write_json_pretty(
        &failure_run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_CHECKPOINT_EVAL_FAILURE_PATH),
        &failure,
    )?;
    let alert = record_psion_actual_pretraining_redacted_alert(
        failure_run_id,
        16384,
        &checkpoint_eval_failure_relative_path(16384),
        "Fixture checkpoint eval retry alert keeps the failed trigger explicit under one redacted operator alert surface instead of silently dropping the missing eval.",
    )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_FIXTURE_PATH),
        &alert,
    )?;
    write_json_pretty(
        &failure_run_root.join(PSION_ACTUAL_PRETRAINING_LATEST_REDACTED_ALERT_PATH),
        &alert,
    )?;

    println!(
        "wrote {}, {}, {}, and {}",
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_BENCHMARK_FIXTURE_PATH,
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_DECISION_FIXTURE_PATH,
        PSION_ACTUAL_PRETRAINING_CHECKPOINT_EVAL_FAILURE_FIXTURE_PATH,
        PSION_ACTUAL_PRETRAINING_REDACTED_ALERT_FIXTURE_PATH
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

fn write_checkpoint_manifest(
    run_root: &Path,
    run_id: &str,
    checkpoint_label: &str,
    optimizer_step: u64,
    checkpoint_ref: &str,
    dataset_identity: &str,
) -> Result<PsionActualPretrainingCheckpointManifest, Box<dyn Error>> {
    let checkpoint_manifest = record_psion_actual_pretraining_checkpoint_manifest(
        run_id,
        checkpoint_label,
        optimizer_step,
        checkpoint_ref,
        &sha256_hex(
            format!("{run_id}|{checkpoint_label}|{optimizer_step}|{checkpoint_ref}").as_bytes(),
        ),
        4_294_967_296,
        dataset_identity,
        "refs/heads/main",
        FIXTURE_GIT_SHA,
        "refuse_by_default",
        None,
        "Checkpoint manifest records one accepted actual-lane checkpoint inside the frozen evidence family without claiming that the broader distributed training job ran inside this launcher process.",
    )?;
    write_json_pretty(
        &run_root.join(&checkpoint_manifest.relative_manifest_path),
        &checkpoint_manifest,
    )?;
    Ok(checkpoint_manifest)
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
