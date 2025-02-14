use crate::repo::{cleanup_temp_dir, clone_repository};
use crate::repomap::generate_repo_map;
use anyhow::Result;
use sqlx::PgPool;
use std::path::PathBuf;
use tracing::{info, warn};

mod cache;
use cache::RepomapCache;

pub struct RepomapService {
    pool: Option<PgPool>,
    temp_dir: PathBuf,
    github_token: Option<String>,
}

impl RepomapService {
    pub fn new(temp_dir: PathBuf, github_token: Option<String>) -> Self {
        Self {
            pool: None,
            temp_dir,
            github_token,
        }
    }

    pub fn with_pool(pool: PgPool, temp_dir: PathBuf, github_token: Option<String>) -> Self {
        Self {
            pool: Some(pool),
            temp_dir,
            github_token,
        }
    }

    pub async fn generate_repomap(&self, owner: &str, repo: &str) -> Result<(String, PathBuf)> {
        let map = self
            .generate_repository_map(&format!("{}/{}", owner, repo))
            .await?;
        Ok((serde_json::to_string(&map)?, self.temp_dir.clone()))
    }

    pub async fn get_repository_map(
        &self,
        repo_name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<serde_json::Value> {
        // Try to get from cache first if pool is available
        if let Some(pool) = &self.pool {
            if let Some(cached) = RepomapCache::get(pool, repo_name, branch, commit_sha).await? {
                info!(
                    "Found cached repomap for {}/{} at {}",
                    repo_name, branch, commit_sha
                );
                return Ok(cached.map_data);
            }
        }

        // If not in cache or no pool available, generate the map
        info!(
            "Generating repomap for {}/{} at {}",
            repo_name, branch, commit_sha
        );
        let map_data = self.generate_repository_map(repo_name).await?;

        // Cache the result if pool is available
        if let Some(pool) = &self.pool {
            let cache = RepomapCache::new(
                repo_name.to_string(),
                branch.to_string(),
                commit_sha.to_string(),
                map_data.clone(),
            );
            if let Err(e) = cache.save(pool).await {
                warn!("Failed to cache repomap: {}", e);
            }
        }

        Ok(map_data)
    }

    async fn generate_repository_map(&self, repo_name: &str) -> Result<serde_json::Value> {
        // Clean up any existing temp directory first
        cleanup_temp_dir(&self.temp_dir);

        // Clone the repository
        let repo_url = format!("https://github.com/{}", repo_name);
        let _repo = clone_repository(&repo_url, &self.temp_dir, self.github_token.as_deref())?;

        // Generate the repository map
        let map = generate_repo_map(&self.temp_dir);

        // Convert to serde_json::Value
        Ok(serde_json::Value::String(map))
    }

    pub async fn invalidate_cache(
        &self,
        repo_name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<()> {
        if let Some(pool) = &self.pool {
            RepomapCache::delete(pool, repo_name, branch, commit_sha).await?;
        }
        Ok(())
    }

    pub fn cleanup(&self) {
        cleanup_temp_dir(&self.temp_dir);
    }
}
