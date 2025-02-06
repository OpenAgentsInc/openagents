use crate::solver::state::SolverState;
use anyhow::Result;
use std::path::Path;
use tracing::{debug, info};

pub fn apply_changes(state: &SolverState, repo_dir: &str) -> Result<()> {
    info!("Applying changes to files...");
    let base_path = Path::new(repo_dir);

    for (path, file) in &state.files {
        let file_path = base_path.join(path);
        debug!("Processing file: {}", file_path.display());

        if let Ok(mut content) = std::fs::read_to_string(&file_path) {
            for change in &file.changes {
                debug!("Applying change: {}", change.analysis);
                content = content.replace(&change.search, &change.replace);
            }
            std::fs::write(file_path, content)?;
        }
    }

    Ok(())
}