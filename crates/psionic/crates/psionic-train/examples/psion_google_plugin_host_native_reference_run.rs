use std::{collections::BTreeSet, error::Error, fs, path::Path, path::PathBuf};

use psionic_runtime::TrainingCheckpointReference;
use psionic_train::run_psion_plugin_host_native_reference_lane;
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
    learned_plugin_ids: Vec<String>,
    benchmark_family_count: u32,
    model_artifact_digest: String,
    evaluation_receipt_digest: String,
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

fn checkpoint_refs(
    stage_program: &psionic_train::TrainingStageProgramState,
) -> Vec<TrainingCheckpointReference> {
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
        .unwrap_or_else(|| repo_root.join("target/psion_google_plugin_host_native_reference_run"));

    let bundle = run_psion_plugin_host_native_reference_lane()?;
    let refs = checkpoint_refs(&bundle.stage_bundle.stage_program);
    let latest_checkpoint = refs
        .last()
        .ok_or("host-native reference lane should retain one checkpoint reference")?;
    let checkpoint_refs = refs
        .iter()
        .map(|checkpoint| {
            checkpoint
                .checkpoint_ref
                .clone()
                .ok_or("host-native reference checkpoint ref should be present")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let latest_checkpoint_ref = latest_checkpoint
        .checkpoint_ref
        .clone()
        .ok_or("latest host-native reference checkpoint ref should be present")?;
    let latest_checkpoint_step = latest_checkpoint
        .step
        .ok_or("latest host-native reference checkpoint step should be present")?;
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
            "Logical checkpoint evidence preserves the bounded stage-program checkpoint refs for the host-native plugin-conditioned lane without implying a broader dense-checkpoint artifact than this reference lane actually emits.",
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
        learned_plugin_ids: bundle.model_artifact.learned_plugin_ids.clone(),
        benchmark_family_count: bundle.evaluation_receipt.benchmark_deltas.len() as u32,
        model_artifact_digest: bundle.model_artifact.artifact_digest.clone(),
        evaluation_receipt_digest: bundle.evaluation_receipt.receipt_digest.clone(),
        bundle_digest: bundle.bundle_digest.clone(),
        detail: String::from(
            "Host-native Google run summary preserves the dataset identity, learned local-deterministic plugin set, benchmark family coverage, and artifact digests for the bounded plugin-conditioned lane.",
        ),
    };

    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_run_bundle.json",
        &bundle,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_stage_bundle.json",
        &bundle.stage_bundle,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_stage_receipt.json",
        &bundle.stage_bundle.stage_receipt,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_model_artifact.json",
        &bundle.model_artifact,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_evaluation_receipt.json",
        &bundle.evaluation_receipt,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_checkpoint_evidence.json",
        &checkpoint_evidence,
    )?;
    write_json(
        output_dir.as_path(),
        "psion_plugin_host_native_reference_run_summary.json",
        &run_summary,
    )?;

    Ok(())
}
