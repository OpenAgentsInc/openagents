use std::{error::Error, path::PathBuf};

use psionic_train::write_builtin_executor_mlx_forward_load_parity_packet;

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    write_builtin_executor_mlx_forward_load_parity_packet(&root)?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "failed to locate workspace root".into())
}
