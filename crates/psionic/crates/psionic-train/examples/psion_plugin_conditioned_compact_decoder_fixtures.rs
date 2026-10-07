use std::{error::Error, path::PathBuf};

use psionic_train::{
    psion_plugin_conditioned_compact_decoder_reference_config_path,
    record_psion_plugin_conditioned_compact_decoder_reference_config,
    PsionPluginConditionedSftStageManifest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let stage_manifest: PsionPluginConditionedSftStageManifest = serde_json::from_str(
        &std::fs::read_to_string(
            root.join(
                "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json",
            ),
        )?,
    )?;
    let config = record_psion_plugin_conditioned_compact_decoder_reference_config(
        &stage_manifest,
        "The first plugin-conditioned compact-decoder config freezes one pilot-sized descriptor, one no-custom-token JSON serialization posture, and lane-bound checkpoint naming.",
    )?;
    config.write_to_path(psion_plugin_conditioned_compact_decoder_reference_config_path())?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| String::from("failed to resolve workspace root"))?
        .to_path_buf();
    Ok(root)
}
