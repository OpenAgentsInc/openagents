use crate::repo::{clone_repository, RepoContext};
use crate::repomap::generate_repo_map;
use crate::server::services::gateway::Gateway;
use crate::server::services::openrouter::OpenRouterService;
use crate::solver::Change;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct SolutionContext {
    pub temp_dir: PathBuf,
    pub repo_context: RepoContext,
    pub modified_files: Vec<String>,
    gateway: Box<dyn Gateway + Send + Sync>,
}

impl SolutionContext {
    pub fn new(
        issue_number: i32,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("solver_{}", issue_number));

        // Clean up any existing temp directory first
        if temp_dir.exists() {
            println!("Cleaning up existing temp directory: {:?}", temp_dir);
            fs::remove_dir_all(&temp_dir)?;
        }

        // Create the temporary directory
        println!("Creating temporary directory: {:?}", temp_dir);
        fs::create_dir_all(&temp_dir)?;

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key.clone(), github_token);
        let gateway = Box::new(OpenRouterService::new(openrouter_key)?);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            gateway,
        })
    }

    pub fn new_with_dir(
        temp_dir: PathBuf,
        openrouter_key: String,
        github_token: Option<String>,
    ) -> Result<Self> {
        // Create the temporary directory if it doesn't exist
        if !temp_dir.exists() {
            println!("Creating temporary directory: {:?}", temp_dir);
            fs::create_dir_all(&temp_dir)?;
        }

        let repo_context = RepoContext::new(temp_dir.clone(), openrouter_key.clone(), github_token);
        let gateway = Box::new(OpenRouterService::new(openrouter_key)?);

        Ok(Self {
            temp_dir,
            repo_context,
            modified_files: Vec::new(),
            gateway,
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
        if self.temp_dir.exists() {
            println!("Cleaning up temp directory: {:?}", self.temp_dir);
            let _ = fs::remove_dir_all(&self.temp_dir);
        }
    }

    pub async fn generate_file_list(
        &self,
        issue_title: &str,
        issue_body: &str,
        repo_map: &str,
    ) -> Result<Vec<String>> {
        crate::solver::file_list::generate_file_list(
            &*self.gateway,
            issue_title,
            issue_body,
            repo_map,
            &self.temp_dir,
        )
        .await
    }

    pub async fn generate_changes(
        &self,
        file_path: &str,
        file_content: &str,
        issue_title: &str,
        issue_body: &str,
    ) -> Result<Vec<Change>> {
        crate::solver::changes::generate_changes(
            &*self.gateway,
            file_path,
            file_content,
            issue_title,
            issue_body,
        )
        .await
    }

    pub fn apply_changes(&mut self, changes: &[Change]) -> Result<()> {
        crate::solver::changes::apply_changes(self, changes)
    }
}

impl Drop for SolutionContext {
    fn drop(&mut self) {
        self.cleanup();
    }
}