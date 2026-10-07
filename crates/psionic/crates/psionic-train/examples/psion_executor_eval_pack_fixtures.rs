use std::{error::Error, path::PathBuf};

use psionic_train::write_builtin_executor_eval_pack_catalog;

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    write_builtin_executor_eval_pack_catalog(&root)?;
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
