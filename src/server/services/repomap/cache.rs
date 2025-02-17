use crate::server::models::timestamp::Timestamp;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomapCacheEntry {
    pub repo_name: String,
    pub branch: String,
    pub commit_sha: String,
    pub map_data: Value,
    pub created_at: Timestamp,
}

impl RepomapCacheEntry {
    pub fn new(
        repo_name: String,
        branch: String,
        commit_sha: String,
        map_data: serde_json::Value,
    ) -> Self {
        Self {
            repo_name,
            branch,
            commit_sha,
            map_data,
            created_at: Timestamp::now(),
        }
    }
}

pub struct RepoMapCache {
    pool: PgPool,
}

impl RepoMapCache {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(
        &self,
        repo_name: &str,
        branch: &str,
        commit_sha: &str,
    ) -> Result<Option<RepomapCacheEntry>> {
        let record = sqlx::query_as!(
            RepomapCacheEntry,
            r#"
            SELECT repo_name, branch, commit_sha, map_data, created_at as "created_at: Timestamp"
            FROM repomap_cache
            WHERE repo_name = $1 AND branch = $2 AND commit_sha = $3
            "#,
            repo_name,
            branch,
            commit_sha
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(record)
    }

    pub async fn set(&self, entry: RepomapCacheEntry) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO repomap_cache (repo_name, branch, commit_sha, map_data, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (repo_name, branch, commit_sha) DO UPDATE
            SET map_data = $4, created_at = NOW()
            "#,
            entry.repo_name,
            entry.branch,
            entry.commit_sha,
            entry.map_data
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete(&self, repo_name: &str, branch: &str, commit_sha: &str) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM repomap_cache
            WHERE repo_name = $1 AND branch = $2 AND commit_sha = $3
            "#,
            repo_name,
            branch,
            commit_sha
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
