use crate::solver::types::Change;
use crate::solver::state::SolverState;
use anyhow::Result;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info};

pub async fn apply_file_changes(state: &SolverState, repo_dir: &Path) -> Result<()> {
    info!("Applying file changes...");

    for (path, file) in &state.files {
        let file_path = Path::new(&file.path);
        let full_path = repo_dir.join(file_path);

        if file.changes.is_empty() {
            debug!("No changes for file: {}", file.path);
            continue;
        }

        info!("Applying changes to file: {}", file.path);
        let mut content = fs::read_to_string(&full_path)?;

        for change in &file.changes {
            debug!("Applying change: {} -> {}", change.search, change.replace);
            content = content.replace(&change.search, &change.replace);
        }

        fs::write(&full_path, content)?;
        info!("Successfully updated file: {}", file.path);
    }

    Ok(())
}