use crate::repo::{cleanup_temp_dir, clone_repository, commit_changes, checkout_branch, RepoContext};
use crate::repomap::generate_repo_map;
use anyhow::{Result, Context};
use git2::Repository;
use std::path::PathBuf;
use tracing::{debug, info};

pub struct SolutionContext {
    pub temp_dir: PathBuf,
    pub repo_context: RepoContext,
    pub modified_files: Vec<String>,
    repo: Option<Repository>,
}

impl SolutionContext {
    pub fn new(
        issue_number: i32,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("solver_{}", issue_number));

        // Clean up any existing temp directory first
        cleanup_temp_dir(&temp_dir);

        // Create the temporary directory
        std::fs::create_dir_all(&temp_dir)?;
        debug!("Temporary directory created at: {:?}", temp_dir);

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key, github_token);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            repo: None,
        })
    }

    pub fn clone_repository(&mut self, repo_url: &str) -> Result<()> {
        let repo = clone_repository(repo_url, &self.repo_context.temp_dir)?;
        self.repo = Some(repo);
        Ok(())
    }

    pub fn checkout_branch(&self, branch_name: &str) -> Result<()> {
        let repo = self.repo.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Repository not initialized"))?;
        checkout_branch(repo, branch_name)
            .with_context(|| format!("Failed to checkout branch: {}", branch_name))
    }

    pub fn commit_changes(&self, message: &str) -> Result<()> {
        let repo = self.repo.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Repository not initialized"))?;
        
        if self.modified_files.is_empty() {
            debug!("No files to commit");
            return Ok(());
        }

        info!("Committing changes to {} files", self.modified_files.len());
        debug!("Modified files: {:?}", self.modified_files);
        
        commit_changes(repo, &self.modified_files, message)
            .with_context(|| format!("Failed to commit changes: {}", message))
    }

    pub fn generate_repo_map(&self) -> String {
        generate_repo_map(&self.repo_context.temp_dir)
    }

    pub fn cleanup(&self) {
        cleanup_temp_dir(&self.temp_dir);
        debug!("Temporary directory removed.");
    }
}