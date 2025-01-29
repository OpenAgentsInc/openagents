use crate::repo::{cleanup_temp_dir, clone_repository, RepoContext};
use crate::repomap::generate_repo_map;
use crate::solver::file_list::generate_file_list;
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

    /// Creates a new SolutionContext with a specified directory
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

    /// Generates a map of the repository structure
    pub fn generate_repo_map(&self) -> String {
        generate_repo_map(&self.temp_dir)
    }

    /// Generates a list of files that need to be modified
    pub async fn generate_file_list(
        &self,
        title: &str,
        description: &str,
    ) -> Result<(Vec<String>, String)> {
        let repo_map = self.generate_repo_map();
        generate_file_list(title, description, &repo_map, &self.repo_context.api_key).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_dir() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let context = SolutionContext::new_with_dir(
            temp_dir.path().to_path_buf(),
            "test_key".to_string(),
            Some("test_token".to_string()),
        )?;
        assert!(context.temp_dir.exists());
        assert!(context.modified_files.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_generate_file_list() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let context = SolutionContext::new_with_dir(
            temp_dir.path().to_path_buf(),
            "test_key".to_string(),
            Some("test_token".to_string()),
        )?;

        // Create test files
        fs::create_dir_all(context.temp_dir.join("src"))?;
        fs::write(
            context.temp_dir.join("src/main.rs"),
            "fn main() { println!(\"Hello\"); }",
        )?;
        fs::write(
            context.temp_dir.join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )?;

        let (files, reasoning) = context
            .generate_file_list("Add multiply function", "Add a multiply function to lib.rs")
            .await?;

        assert!(!files.is_empty());
        assert!(files.contains(&"src/lib.rs".to_string()));
        assert!(!files.contains(&"src/main.rs".to_string()));
        assert!(!reasoning.is_empty());

        Ok(())
    }
}
