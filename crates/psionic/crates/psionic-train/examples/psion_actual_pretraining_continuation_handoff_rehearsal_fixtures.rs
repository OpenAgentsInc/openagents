use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use psionic_eval::build_psion_actual_pretraining_continuation_eval_benchmark_package;
use psionic_train::{
    AgenticSftRlReferenceProgramSpec,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_FIXTURE_PATH,
    PsionActualPretrainingArtifactRef, PsionActualPretrainingCloseoutBundle,
    PsionActualPretrainingContinuationAlignmentBundle, PsionActualPretrainingContinuationHandoff,
    PsionActualPretrainingContinuationHandoffRefusalPacket,
    PsionActualPretrainingContinuationHandoffRehearsalBundle, PsionPluginConditionedSftRunBundle,
    PsionPluginConditionedSftStageManifest, PsionReasoningSftRunBundle,
    record_psion_actual_pretraining_continuation_alignment_bundle,
    record_psion_actual_pretraining_continuation_handoff_refusal_packet,
    record_psion_actual_pretraining_continuation_handoff_rehearsal_bundle,
    run_agentic_sft_rl_reference_program,
};
use sha2::{Digest, Sha256};

const BASE_RUN_ROOT: &str = "fixtures/psion/pretrain/psion_actual_pretraining_base_lane_rehearsal_example/run-psion-actual-20260402t160000z";
const REHEARSAL_RUN_ROOT: &str = "fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_rehearsal_example/run-psion-actual-20260402t160000z";
const ALIGNMENT_RELATIVE_PATH: &str = "continuation/continuation_alignment_bundle.json";
const REFUSED_ALIGNMENT_RELATIVE_PATH: &str =
    "continuation/failures/mismatched_alignment_candidate.json";
const REFUSAL_COPY_RELATIVE_PATH: &str = "continuation/failures/mismatched_alignment_refusal.json";
const BUNDLE_COPY_RELATIVE_PATH: &str = "continuation/continuation_handoff_rehearsal_bundle.json";

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let base_run_root = root.join(BASE_RUN_ROOT);
    let rehearsal_run_root = root.join(REHEARSAL_RUN_ROOT);

    let closeout_bundle: PsionActualPretrainingCloseoutBundle =
        load_json(&base_run_root.join("closeout/closeout_bundle.json"))?;
    let handoff: PsionActualPretrainingContinuationHandoff =
        load_json(&base_run_root.join("continuation/accepted_checkpoint_handoff.json"))?;
    closeout_bundle.validate()?;
    handoff.validate()?;

    let reasoning_bundle: PsionReasoningSftRunBundle =
        load_json(&root.join("fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json"))?;
    let plugin_run_bundle: PsionPluginConditionedSftRunBundle = load_json(&root.join(
        "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json",
    ))?;
    let plugin_stage_manifest: PsionPluginConditionedSftStageManifest = load_json(&root.join(
        "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json",
    ))?;
    let continuation_eval_pack =
        build_psion_actual_pretraining_continuation_eval_benchmark_package()?;

    let post_training_workspace_root = std::env::temp_dir().join(format!(
        "openagents-psionic-continuation-handoff-rehearsal-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    ));
    if post_training_workspace_root.exists() {
        fs::remove_dir_all(&post_training_workspace_root)?;
    }
    let post_training_report = run_agentic_sft_rl_reference_program(
        &AgenticSftRlReferenceProgramSpec::weather_default(post_training_workspace_root.clone()),
    )?;

    let alignment_bundle: PsionActualPretrainingContinuationAlignmentBundle =
        record_psion_actual_pretraining_continuation_alignment_bundle(
            &handoff,
            &reasoning_bundle,
            &plugin_run_bundle,
            &continuation_eval_pack,
            &post_training_report,
        )?;
    let alignment_path = rehearsal_run_root.join(ALIGNMENT_RELATIVE_PATH);
    write_json_pretty(&alignment_path, &alignment_bundle)?;

    let mut refused_alignment_candidate = alignment_bundle.clone();
    refused_alignment_candidate.accepted_checkpoint_ref =
        String::from("checkpoint://psion/actual-pretraining/mismatched-alignment");
    refused_alignment_candidate.bundle_digest =
        String::from("refused_candidate_intentionally_mismatched");
    let refused_alignment_path = rehearsal_run_root.join(REFUSED_ALIGNMENT_RELATIVE_PATH);
    write_json_pretty(&refused_alignment_path, &refused_alignment_candidate)?;

    let handoff_ref = closeout_bundle
        .evidence_artifacts
        .iter()
        .find(|artifact| artifact.artifact_kind == "continuation_handoff")
        .map(|artifact| artifact.artifact.clone())
        .ok_or_else(|| "base-lane closeout bundle is missing continuation_handoff evidence")?;
    let refused_alignment_ref = artifact_ref_for(&refused_alignment_path, &root)?;
    let refusal_packet: PsionActualPretrainingContinuationHandoffRefusalPacket =
        record_psion_actual_pretraining_continuation_handoff_refusal_packet(
            handoff_ref.clone(),
            &handoff,
            refused_alignment_ref,
            &refused_alignment_candidate,
        )?;
    let refusal_packet_path =
        root.join(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_FIXTURE_PATH);
    write_json_pretty(&refusal_packet_path, &refusal_packet)?;
    write_json_pretty(
        &rehearsal_run_root.join(REFUSAL_COPY_RELATIVE_PATH),
        &refusal_packet,
    )?;

    let closeout_bundle_ref =
        artifact_ref_for(&base_run_root.join("closeout/closeout_bundle.json"), &root)?;
    let alignment_ref = artifact_ref_for(&alignment_path, &root)?;
    let plugin_stage_manifest_ref = artifact_ref_for(
        &root.join(
            "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json",
        ),
        &root,
    )?;
    let refusal_packet_ref = artifact_ref_for(&refusal_packet_path, &root)?;
    let rehearsal_bundle: PsionActualPretrainingContinuationHandoffRehearsalBundle =
        record_psion_actual_pretraining_continuation_handoff_rehearsal_bundle(
            closeout_bundle_ref,
            &closeout_bundle,
            handoff_ref,
            &handoff,
            alignment_ref,
            &alignment_bundle,
            plugin_stage_manifest_ref,
            &plugin_stage_manifest,
            refusal_packet_ref,
            &refusal_packet,
        )?;
    let bundle_path =
        root.join(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_FIXTURE_PATH);
    write_json_pretty(&bundle_path, &rehearsal_bundle)?;
    write_json_pretty(
        &rehearsal_run_root.join(BUNDLE_COPY_RELATIVE_PATH),
        &rehearsal_bundle,
    )?;

    if post_training_workspace_root.exists() {
        fs::remove_dir_all(post_training_workspace_root)?;
    }
    println!(
        "wrote {}\nwrote {}",
        PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REHEARSAL_BUNDLE_FIXTURE_PATH,
        PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_REFUSAL_PACKET_FIXTURE_PATH
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

fn artifact_ref_for(
    path: &Path,
    workspace_root: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    Ok(PsionActualPretrainingArtifactRef {
        path: path
            .strip_prefix(workspace_root)?
            .to_string_lossy()
            .replace('\\', "/"),
        sha256: file_sha256(path)?,
    })
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
