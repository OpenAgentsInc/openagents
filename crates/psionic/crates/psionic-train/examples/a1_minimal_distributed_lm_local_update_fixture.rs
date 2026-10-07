use std::{env, error::Error, path::PathBuf};

use psionic_train::write_a1_minimal_distributed_lm_local_update_fixture;

fn main() -> Result<(), Box<dyn Error>> {
    let root = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or(workspace_root()?);

    write_a1_minimal_distributed_lm_local_update_fixture(root)?;
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
