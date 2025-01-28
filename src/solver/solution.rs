use anyhow::Result;
use std::path::PathBuf;
use crate::repo::{cleanup_temp_dir, clone_repository, RepoContext};
use crate::repomap::generate_repo_map;

pub struct SolutionContext {
    pub temp_dir: PathBuf,
    pub repo_context: RepoContext,
    pub modified_files: Vec<String>,
}

impl SolutionContext {
    pub fn new(issue_number: i32, openrouter_key: String, github_token: Option<String>) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("solver_{}", issue_number));
        
        // Clean up any existing temp directory first
        cleanup_temp_dir(&temp_dir);

        // Create the temporary directory
        std::fs::create_dir_all(&temp_dir)?;
        println!("Temporary directory created at: {:?}", temp_dir);

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key, github_token);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
        })
    }

    pub fn clone_repository(&self, repo_url: &str) -> Result<()> {
        clone_repository(repo_url, &self.repo_context.temp_dir)?;
        Ok(())
    }

    pub fn generate_repo_map(&self) -> String {
        generate_repo_map(&self.repo_context.temp_dir)
    }

    pub fn cleanup(&self) {
        cleanup_temp_dir(&self.temp_dir);
        println!("Temporary directory removed.");
    }
}