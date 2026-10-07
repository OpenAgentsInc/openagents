use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::build_psion_actual_pretraining_continuation_eval_benchmark_package;
use psionic_train::{
    AgenticSftRlReferenceProgramSpec, PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_FIXTURE_PATH,
    PsionActualPretrainingContinuationAlignmentBundle, PsionActualPretrainingContinuationHandoff,
    PsionPluginConditionedSftRunBundle, PsionReasoningSftRunBundle,
    record_psion_actual_pretraining_continuation_alignment_bundle,
    run_agentic_sft_rl_reference_program,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let handoff: PsionActualPretrainingContinuationHandoff = load_json(
        &root.join("fixtures/psion/pretrain/psion_actual_pretraining_continuation_handoff_v1.json"),
    )?;
    handoff.validate()?;
    let reasoning_bundle: PsionReasoningSftRunBundle =
        load_json(&root.join("fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json"))?;
    let plugin_run_bundle: PsionPluginConditionedSftRunBundle = load_json(&root.join(
        "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json",
    ))?;
    let continuation_eval_pack = build_psion_actual_pretraining_continuation_eval_benchmark_package()?;

    let workspace_root = std::env::temp_dir().join(format!(
        "openagents-psionic-continuation-alignment-fixture-{}",
        std::process::id()
    ));
    if workspace_root.exists() {
        fs::remove_dir_all(&workspace_root)?;
    }
    let post_training_report = run_agentic_sft_rl_reference_program(
        &AgenticSftRlReferenceProgramSpec::weather_default(workspace_root.clone()),
    )?;
    let bundle: PsionActualPretrainingContinuationAlignmentBundle =
        record_psion_actual_pretraining_continuation_alignment_bundle(
            &handoff,
            &reasoning_bundle,
            &plugin_run_bundle,
            &continuation_eval_pack,
            &post_training_report,
        )?;
    write_json_pretty(
        &root.join(PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_FIXTURE_PATH),
        &bundle,
    )?;
    if workspace_root.exists() {
        fs::remove_dir_all(&workspace_root)?;
    }
    println!(
        "wrote {}",
        PSION_ACTUAL_PRETRAINING_CONTINUATION_ALIGNMENT_BUNDLE_FIXTURE_PATH
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
