use std::{collections::BTreeSet, error::Error, fs, path::Path, path::PathBuf};

use psionic_runtime::TrainingCheckpointReference;
use psionic_train::{
    build_psion_plugin_guest_plugin_benchmark_bundle, record_psion_plugin_mixed_capability_matrix,
    record_psion_plugin_mixed_served_posture, run_psion_plugin_mixed_reference_lane,
    TrainingStageProgramState,
};
use serde::Serialize;

#[derive(Serialize)]
struct PluginCheckpointEvidence {
    schema_version: String,
    run_id: String,
    lane_id: String,
    checkpoint_family: String,
    checkpoint_ref_count: u32,
    checkpoint_refs: Vec<String>,
    latest_checkpoint_ref: String,
    latest_checkpoint_step: u64,
    stage_receipt_digest: String,
    evaluation_receipt_digest: String,
    model_artifact_digest: String,
    detail: String,
}

#[derive(Serialize)]
struct PluginRunSummary {
    schema_version: String,
    run_id: String,
    lane_id: String,
    dataset_ref: String,
    stable_dataset_identity: String,
    training_example_count: u32,
    guest_artifact_training_example_count: u32,
    learned_plugin_ids: Vec<String>,
    benchmark_family_count: u32,
    guest_benchmark_receipt_digest: String,
    model_artifact_digest: String,
    evaluation_receipt_digest: String,
    capability_matrix_digest: String,
    served_posture_digest: String,
    bundle_digest: String,
    detail: String,
}

fn write_json<T: Serialize>(
    output_dir: &Path,
    file_name: &str,
    value: &T,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(file_name);
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n"))?;
    Ok(())
}

fn checkpoint_refs(stage_program: &TrainingStageProgramState) -> Vec<TrainingCheckpointReference> {
    let mut refs = Vec::new();
    let mut seen = BTreeSet::new();
    for stage in &stage_program.stages {
        if let Some(base_checkpoint) = &stage.base_checkpoint {
            if seen.insert(base_checkpoint.checkpoint_ref.clone()) {
                refs.push(base_checkpoint.clone());
            }
        }
    }
    for promotion in &stage_program.promotions {
        if seen.insert(promotion.checkpoint.checkpoint_ref.clone()) {
            refs.push(promotion.checkpoint.clone());
        }
    }
    refs.sort_by_key(|checkpoint| checkpoint.step);
    refs
}

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from example path");
    let output_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target/psion_google_plugin_mixed_reference_run"));

    let bundle = run_psion_plugin_mixed_reference_lane()?;
    let guest_benchmark_bundle = build_psion_plugin_guest_plugin_benchmark_bundle()?;
    let capability_matrix =
        record_psion_plugin_mixed_capability_matrix(&bundle, &guest_benchmark_bundle)?;
    let served_posture = record_psion_plugin_mixed_served_posture(
        &capability_matrix,
        &bundle,
        &guest_benchmark_bundle,
    )?;
    let refs = checkpoint_refs(&bundle.stage_bundle.stage_program);
    let latest_checkpoint = refs
        .last()
        .ok_or("mixed reference lane should retain one checkpoint reference")?;
    let checkpoint_refs = refs
        .iter()
        .map(|checkpoint| {
            checkpoint
                .checkpoint_ref
                .clone()
                .ok_or("mixed reference checkpoint ref should be present")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let latest_checkpoint_ref = latest_checkpoint
        .checkpoint_ref
        .clone()
        .ok_or("latest mixed reference checkpoint ref should be present")?;
    let latest_checkpoint_step = latest_checkpoint
        .step
        .ok_or("latest mixed reference checkpoint step should be present")?;
    let checkpoint_evidence = PluginCheckpointEvidence {
        schema_version: String::from("psion.google_plugin_checkpoint_evidence.v1"),
        run_id: bundle.stage_bundle.run_id.clone(),
        lane_id: bundle.lane_id.clone(),
        checkpoint_family: bundle.stage_bundle.checkpoint_family.clone(),
        checkpoint_ref_count: refs.len() as u32,
        checkpoint_refs,
        latest_checkpoint_ref,
        latest_checkpoint_step,
        stage_receipt_digest: bundle.stage_bundle.stage_receipt.receipt_digest.clone(),
        evaluation_receipt_digest: bundle.evaluation_receipt.receipt_digest.clone(),
        model_artifact_digest: bundle.model_artifact.artifact_digest.clone(),
        detail: String::from(
            "Logical checkpoint evidence preserves the bounded stage-program checkpoint refs for the mixed plugin-conditioned lane without implying a broader dense-checkpoint artifact than this mixed reference lane actually emits.",
        ),
    };
    let run_summary = PluginRunSummary {
        schema_version: String::from("psion.google_plugin_run_summary.v1"),
        run_id: bundle.stage_bundle.run_id.clone(),
        lane_id: bundle.lane_id.clone(),
        dataset_ref: bundle.stage_bundle.stage_manifest.dataset_binding.dataset_ref.clone(),
        stable_dataset_identity: bundle
            .stage_bundle
            .stage_manifest
            .dataset_binding
            .stable_dataset_identity
            .clone(),
        training_example_count: bundle.model_artifact.training_example_count,
        guest_artifact_training_example_count: bundle
            .model_artifact
            .guest_artifact_training_example_count,
        learned_plugin_ids: bundle.model_artifact.learned_plugin_ids.clone(),
        benchmark_family_count: bundle.evaluation_receipt.benchmark_comparisons.len() as u32,
        guest_benchmark_receipt_digest: guest_benchmark_bundle.receipt.receipt_digest.clone(),
        model_artifact_digest: bundle.model_artifact.artifact_digest.clone(),
        evaluation_receipt_digest: bundle.evaluation_receipt.receipt_digest.clone(),
        capability_matrix_digest: capability_matrix.matrix_digest.clone(),
        served_posture_digest: served_posture.posture_digest.clone(),
        bundle_digest: bundle.bundle_digest.clone(),
        detail: String::from(
            "Mixed Google run summary preserves the mixed dataset identity, one guest-artifact training example, the mixed comparison receipt, the guest benchmark receipt, and the run-derived mixed capability publication digests.",
        ),
    };

    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_run_bundle.json",
        &bundle,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_stage_bundle.json",
        &bundle.stage_bundle,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_stage_receipt.json",
        &bundle.stage_bundle.stage_receipt,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_model_artifact.json",
        &bundle.model_artifact,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_evaluation_receipt.json",
        &bundle.evaluation_receipt,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_checkpoint_evidence.json",
        &checkpoint_evidence,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_reference_run_summary.json",
        &run_summary,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_guest_plugin_benchmark_bundle.json",
        &guest_benchmark_bundle,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_capability_matrix_v2.json",
        &capability_matrix,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_mixed_served_posture_v2.json",
        &served_posture,
    )?;

    Ok(())
}
