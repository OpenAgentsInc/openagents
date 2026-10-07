use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::{
    build_psion_reference_pilot_joint_contribution, PsionReferencePilotJointContributionRequest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let request_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("missing request path")?;
    let output_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .ok_or("missing output path")?;
    let request: PsionReferencePilotJointContributionRequest =
        serde_json::from_slice(&fs::read(&request_path)?)?;
    let receipt = build_psion_reference_pilot_joint_contribution(root.as_path(), &request)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "psion reference pilot joint contribution completed: step={} contributor={} backend={} output={}",
        receipt.global_step,
        receipt.contributor_id,
        receipt.runtime_backend,
        output_path.display()
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
