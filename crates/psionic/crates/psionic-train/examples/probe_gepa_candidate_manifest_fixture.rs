use std::{env, fs, path::PathBuf};

use psionic_train::canonical_probe_gepa_stage_0_1_candidate_manifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/probe/gepa/probe_gepa_candidate_manifest_stage_0_1_seed_v1.json",
        )
    });
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest = canonical_probe_gepa_stage_0_1_candidate_manifest()?;
    fs::write(output_path, format!("{}\n", serde_json::to_string_pretty(&manifest)?))?;
    Ok(())
}
