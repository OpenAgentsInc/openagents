use crate::repo::{cleanup_temp_dir, clone_repository, RepoContext};
use crate::repomap::generate_repo_map;
use anyhow::Result;
use std::path::PathBuf;

pub struct RepomapService {
    temp_dir: PathBuf,
    github_token: Option<String>,
}

impl RepomapService {
    pub fn new(temp_dir: PathBuf, github_token: Option<String>) -> Self {
        Self {
            temp_dir,
            github_token,
        }
    }

    pub async fn generate_repomap(&self, owner: &str, repo: &str) -> Result<String> {
        // Clean up any existing temp directory first
        cleanup_temp_dir(&self.temp_dir);

        // Create the temporary directory
        std::fs::create_dir_all(&self.temp_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create temporary directory: {}", e))?;

        // Create context
        let ctx = RepoContext::new(
            self.temp_dir.clone(),
            String::new(), // API key not needed for repomap generation
            self.github_token.clone(),
        );

        // Clone the repository
        let repo_url = format!("https://github.com/{}/{}", owner, repo);
        let _repo = clone_repository(&repo_url, &ctx.temp_dir)?;

        // Generate the repository map
        let map = generate_repo_map(&ctx.temp_dir);

        // Clean up
        cleanup_temp_dir(&self.temp_dir);

        Ok(map)
    }
}
