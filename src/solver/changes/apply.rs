use crate::solver::state::{Change, SolverState};
use crate::solver::types::ChangeError;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, error, info};

/// Result type for change operations
type ChangeResult<T> = Result<T, ChangeError>;

/// Applies a single change to a file
fn apply_change_to_file(change: &Change, base_path: &Path, file_path: &Path) -> ChangeResult<()> {
    // Get the full path by joining base_path and file_path
    let full_path = base_path.join(file_path);
    debug!("Full path: {}", full_path.display());

    // Read the file content
    let content = fs::read_to_string(&full_path).map_err(|e| {
        error!("Failed to read file {}: {}", full_path.display(), e);
        ChangeError::IoError(e)
    })?;

    // If search is empty, this is a new file
    if change.search.is_empty() {
        debug!("Creating new file {}", full_path.display());
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).map_err(ChangeError::IoError)?;
        }
        fs::write(&full_path, &change.replace).map_err(ChangeError::IoError)?;
        return Ok(());
    }

    // Find all matches of the search pattern
    let matches: Vec<_> = content.match_indices(&change.search).collect();

    // Validate number of matches
    match matches.len() {
        0 => {
            error!(
                "No matches found for search pattern in {}",
                full_path.display()
            );
            Err(ChangeError::NoMatch)
        }
        1 => {
            // Apply the change
            let (start, _) = matches[0];
            let mut new_content = content[..start].to_string();
            new_content.push_str(&change.replace);
            new_content.push_str(&content[start + change.search.len()..]);

            // Write the modified content back to the file
            debug!("Writing changes to {}", full_path.display());
            fs::write(&full_path, new_content).map_err(ChangeError::IoError)?;
            Ok(())
        }
        _ => {
            error!(
                "Multiple matches ({}) found for search pattern in {}",
                matches.len(),
                full_path.display()
            );
            Err(ChangeError::MultipleMatches)
        }
    }
}

/// Applies all changes in the solver state
pub fn apply_changes(state: &mut SolverState, repo_dir: &str) -> Result<()> {
    info!("Applying changes to files...");
    debug!("Using repo directory: {}", repo_dir);

    let base_path = Path::new(repo_dir);
    debug!("Base path: {}", base_path.display());

    for file in &state.files {
        let file_path = Path::new(&file.path);
        debug!("Processing file: {}", file_path.display());

        // Skip files with no changes
        if file.changes.is_empty() {
            debug!("Skipping {} - no changes", file_path.display());
            continue;
        }

        info!("Processing changes for {}", file_path.display());

        // Apply each change
        for change in &file.changes {
            match apply_change_to_file(change, base_path, file_path) {
                Ok(_) => {
                    info!("Applied change to {}: {}", file_path.display(), change.analysis);
                }
                Err(e) => {
                    error!(
                        "Failed to apply change to {}: {}",
                        file_path.display(),
                        e
                    );
                    return Err(anyhow!("Failed to apply changes: {}", e));
                }
            }
        }
    }

    info!("Successfully applied all changes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::state::{FileState, SolverState};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_apply_change_to_file() {
        let dir = tempdir().unwrap();
        let file_path = Path::new("test.txt");
        let full_path = dir.path().join(file_path);

        // Create test file
        fs::write(&full_path, "Hello World").unwrap();

        // Test simple replacement
        let change = Change {
            search: "World".to_string(),
            replace: "Rust".to_string(),
            analysis: "Test change".to_string(),
        };

        apply_change_to_file(&change, dir.path(), file_path).unwrap();
        assert_eq!(fs::read_to_string(&full_path).unwrap(), "Hello Rust");

        // Test no match
        let change = Change {
            search: "NotFound".to_string(),
            replace: "NewContent".to_string(),
            analysis: "Test change".to_string(),
        };

        assert!(matches!(
            apply_change_to_file(&change, dir.path(), file_path),
            Err(ChangeError::NoMatch)
        ));

        // Test multiple matches
        fs::write(&full_path, "test test").unwrap();
        let change = Change {
            search: "test".to_string(),
            replace: "new".to_string(),
            analysis: "Test change".to_string(),
        };

        assert!(matches!(
            apply_change_to_file(&change, dir.path(), file_path),
            Err(ChangeError::MultipleMatches)
        ));
    }

    #[test]
    fn test_apply_changes() {
        let dir = tempdir().unwrap();
        let file_path = Path::new("test.txt");
        let full_path = dir.path().join(file_path);

        // Create test file
        fs::write(&full_path, "Hello World").unwrap();

        // Create solver state with changes
        let mut state = SolverState::new("Test state".to_string());
        let mut file_state = FileState {
            id: "test".to_string(),
            path: file_path.to_str().unwrap().to_string(),
            analysis: "Test file".to_string(),
            relevance_score: 1.0,
            changes: vec![],
        };

        file_state.add_change(
            "World".to_string(),
            "Rust".to_string(),
            "Test change".to_string(),
        );
        state.files.push(file_state);

        // Apply changes
        apply_changes(&mut state, dir.path().to_str().unwrap()).unwrap();

        // Verify changes were applied
        assert_eq!(fs::read_to_string(&full_path).unwrap(), "Hello Rust");
    }
}