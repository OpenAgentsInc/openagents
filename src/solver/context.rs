use crate::repo::{cleanup_temp_dir, clone_repository, RepoContext};
use crate::solver::types::{Change, ChangeError, ChangeResult};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Context for generating and applying solutions to GitHub issues
pub struct SolutionContext {
    /// Temporary directory for working with repository files
    pub temp_dir: PathBuf,
    /// Repository context for Git operations
    pub repo_context: RepoContext,
    /// List of files that have been modified
    pub modified_files: Vec<String>,
}

impl SolutionContext {
    /// Creates a new SolutionContext with a temporary directory
    pub fn new(
        issue_number: i32,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("solver_{}", issue_number));

        // Clean up any existing temp directory first
        cleanup_temp_dir(&temp_dir);

        // Create the temporary directory
        fs::create_dir_all(&temp_dir)?;
        tracing::info!("Temporary directory created at: {:?}", temp_dir);

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key, github_token);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
        })
    }

    /// Creates a new SolutionContext with a specified directory (for testing)
    #[cfg(test)]
    pub fn new_with_dir(
        temp_dir: PathBuf,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        fs::create_dir_all(&temp_dir)?;
        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key, github_token);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
        })
    }

    /// Clones a repository into the temporary directory
    pub fn clone_repository(&self, repo_url: &str) -> Result<()> {
        clone_repository(repo_url, &self.repo_context.temp_dir)?;
        Ok(())
    }

    /// Applies a set of changes to files
    pub fn apply_changes(&mut self, changes: &[Change]) -> ChangeResult<()> {
        for change in changes {
            // Validate the change
            change.validate()?;

            let file_path = self.temp_dir.join(&change.path);

            // Handle new file creation
            if change.search.is_empty() {
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_path, &change.replace)?;
                self.modified_files.push(change.path.clone());
                continue;
            }

            // Read existing file
            let content = fs::read_to_string(&file_path)
                .map_err(|_| ChangeError::FileNotFound(file_path.clone()))?;

            // Find and replace content
            let new_content = if let Some(idx) = content.find(&change.search) {
                let mut result = content.clone();
                result.replace_range(idx..idx + change.search.len(), &change.replace);
                result
            } else {
                return Err(ChangeError::NoMatch);
            };

            // Write updated content
            fs::write(&file_path, new_content)?;
            
            // Track modified file
            if !self.modified_files.contains(&change.path) {
                self.modified_files.push(change.path.clone());
            }
        }

        Ok(())
    }

    /// Cleans up temporary files
    pub fn cleanup(&self) {
        cleanup_temp_dir(&self.temp_dir);
        tracing::info!("Temporary directory removed.");
    }
}