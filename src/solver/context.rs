use crate::repo::{
    checkout_branch, cleanup_temp_dir, clone_repository, commit_changes, push_changes_with_token,
    RepoContext,
};
use crate::repomap::generate_repo_map;
use crate::solver::changes::{generate_changes, parse_search_replace};
use crate::solver::types::{Change, ChangeError, ChangeResult};
use anyhow::{Context as _, Result};
use git2::Repository;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

/// Context for generating and applying solutions to GitHub issues
pub struct SolutionContext {
    /// Temporary directory for working with repository files
    pub temp_dir: PathBuf,
    /// Repository context for Git operations
    pub repo_context: RepoContext,
    /// List of files that have been modified
    pub modified_files: Vec<String>,
    /// Git repository instance
    repo: Option<Repository>,
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
        debug!("Temporary directory created at: {:?}", temp_dir);

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key, github_token);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            repo: None,
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
            repo: None,
        })
    }

    /// Clones a repository into the temporary directory
    pub fn clone_repository(&mut self, repo_url: &str) -> Result<()> {
        let repo = clone_repository(repo_url, &self.repo_context.temp_dir)?;
        self.repo = Some(repo);
        Ok(())
    }

    /// Checks out a branch in the repository
    pub fn checkout_branch(&self, branch_name: &str) -> Result<()> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Repository not initialized"))?;
        checkout_branch(repo, branch_name)
            .with_context(|| format!("Failed to checkout branch: {}", branch_name))
    }

    /// Commits changes to the repository
    pub fn commit_changes(&self, message: &str) -> Result<()> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Repository not initialized"))?;

        if self.modified_files.is_empty() {
            debug!("No files to commit");
            return Ok(());
        }

        info!("Committing changes to {} files", self.modified_files.len());
        debug!("Modified files: {:?}", self.modified_files);

        // Commit changes
        commit_changes(repo, &self.modified_files, message)
            .with_context(|| format!("Failed to commit changes: {}", message))?;

        // Push changes if we have a token
        if let Some(token) = &self.repo_context.github_token {
            debug!("Pushing changes with auth token");
            push_changes_with_token(repo, token).with_context(|| "Failed to push changes")?;
        } else {
            debug!("No GitHub token available - skipping push");
        }

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
        crate::solver::file_list::generate_file_list(
            title,
            description,
            &repo_map,
            &self.repo_context.api_key,
        )
        .await
    }

    /// Generates changes for a specific file
    pub async fn generate_changes(
        &mut self,
        path: &str,
        title: &str,
        description: &str,
    ) -> Result<(Vec<Change>, String)> {
        let file_path = self.temp_dir.join(path);
        let content = fs::read_to_string(&file_path)?;

        let (changes, reasoning) = generate_changes(
            path,
            &content,
            title,
            description,
            &self.repo_context.api_key,
        )
        .await?;

        // Track modified files
        if !changes.is_empty() && !self.modified_files.contains(&path.to_string()) {
            self.modified_files.push(path.to_string());
        }

        Ok((changes, reasoning))
    }

    /// Generates changes from SEARCH/REPLACE blocks
    pub fn parse_changes(&self, content: &str) -> ChangeResult<Vec<Change>> {
        parse_search_replace(content)
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
        debug!("Temporary directory removed.");
    }
}
