use crate::repo::{cleanup_temp_dir, clone_repository};
use crate::repomap::generate_repo_map;
use anyhow::Result;
use sqlx::PgPool;
use std::path::PathBuf;
use tracing::{info, warn};
use std::sync::Arc;
use serde_json::Value;
use tokio::sync::RwLock;

mod cache;
use cache::{RepoMapCache, RepomapCacheEntry};

pub struct RepomapService {
    pool: PgPool,
    cache: Arc<RwLock<RepoMapCache>>,
    temp_dir: PathBuf,
    github_token: Option<String>,
}

impl RepomapService {
    pub fn new(pool: PgPool, temp_dir: PathBuf, github_token: Option<String>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(RepoMapCache::new(pool.clone()))),
            pool,
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

    pub async fn get_map(
        &self,
        repo_name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<Option<Value>> {
        let cache = self.cache.read().await;
        if let Some(entry) = cache.get(repo_name, branch, commit_sha).await? {
            return Ok(Some(entry.map_data));
        }
        Ok(None)
    }

    pub async fn set_map(
        &self,
        repo_name: String,
        branch: String,
        commit_sha: String,
        map_data: Value,
    ) -> Result<()> {
        let cache = self.cache.write().await;
        let entry = RepomapCacheEntry {
            repo_name,
            branch,
            commit_sha,
            map_data,
            created_at: crate::server::models::timestamp::Timestamp::now(),
        };
        cache.set(entry).await?;
        Ok(())
    }

    pub async fn delete_map(
        &self,
        repo_name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<()> {
        let cache = self.cache.write().await;
        cache.delete(repo_name, branch, commit_sha).await?;
        Ok(())
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

    pub fn cleanup(&self) {
        cleanup_temp_dir(&self.temp_dir);
    }
}
