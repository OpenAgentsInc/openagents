use std::{error::Error, path::PathBuf};

use psionic_train::{
    write_cs336_a2_baseline_profile_bundle, CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let bundle = write_cs336_a2_baseline_profile_bundle(&root)?;
    println!(
        "wrote {} attention_route={} training_route={} distributed_route={}",
        root.join(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH)
            .display(),
        bundle.attention_baseline.route_id,
        bundle.training_step_baseline.route_id,
        bundle.distributed_step_baseline.route_id,
    );
    Ok(())
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve repo root".into())
}
