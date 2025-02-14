use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct RepomapCache {
    pub repo_name: String,
    pub branch: String,
    pub commit_sha: String,
    pub map_data: serde_json::Value,
    pub created_at: OffsetDateTime,
}

impl RepomapCache {
    pub fn new(repo_name: String, branch: String, commit_sha: String, map_data: serde_json::Value) -> Self {
        Self {
            repo_name,
            branch,
            commit_sha,
            map_data,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    pub async fn save(&self, pool: &PgPool) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO repomap_cache (repo_name, branch, commit_sha, map_data, created_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (repo_name, branch, commit_sha)
            DO UPDATE SET map_data = $4, created_at = $5
            "#,
            self.repo_name,
            self.branch,
            self.commit_sha,
            self.map_data,
            self.created_at,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get(pool: &PgPool, repo_name: &str, branch: &str, commit_sha: &str) -> Result<Option<Self>> {
        let record = sqlx::query!(
            r#"
            SELECT repo_name, branch, commit_sha, map_data, created_at
            FROM repomap_cache
            WHERE repo_name = $1 AND branch = $2 AND commit_sha = $3
            "#,
            repo_name,
            branch,
            commit_sha
        )
        .fetch_optional(pool)
        .await?;

        Ok(record.map(|r| Self {
            repo_name: r.repo_name,
            branch: r.branch,
            commit_sha: r.commit_sha,
            map_data: r.map_data,
            created_at: r.created_at,
        }))
    }

    pub async fn delete(pool: &PgPool, repo_name: &str, branch: &str, commit_sha: &str) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM repomap_cache
            WHERE repo_name = $1 AND branch = $2 AND commit_sha = $3
            "#,
            repo_name,
            branch,
            commit_sha
        )
        .execute(pool)
        .await?;

        Ok(())
    }
}
