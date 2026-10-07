use std::{error::Error, path::PathBuf};

use psionic_train::write_builtin_executor_4080_decision_grade_artifacts;

fn main() -> Result<(), Box<dyn Error>> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .ok_or("failed to locate workspace root")?
        .to_path_buf();
    write_builtin_executor_4080_decision_grade_artifacts(&workspace_root)?;
    Ok(())
}
