use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::server::models::timestamp::Timestamp;

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomapCacheEntry {
    pub repo_name: String,
    pub branch: String,
    pub commit_sha: String,
    pub map_data: Value,
    pub created_at: Timestamp,
}

pub struct RepoMapCache {
    pool: PgPool,
}

impl RepoMapCache {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, repo_name: &str, branch: &str, commit_sha: &str) -> Result<Option<RepomapCacheEntry>> {
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
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (repo_name, branch, commit_sha) DO UPDATE
            SET map_data = $4, created_at = $5
            "#,
            entry.repo_name,
            entry.branch,
            entry.commit_sha,
            entry.map_data,
            entry.created_at.into_inner()
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
