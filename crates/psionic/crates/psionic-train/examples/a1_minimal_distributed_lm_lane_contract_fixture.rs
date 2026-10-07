use std::{env, error::Error, path::PathBuf};

use psionic_train::{
    write_a1_minimal_distributed_lm_lane_contract,
    A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_FIXTURE_PATH,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_FIXTURE_PATH));

    write_a1_minimal_distributed_lm_lane_contract(output_path)?;
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
