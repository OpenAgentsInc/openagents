//! Permission rule storage using SQLite
//!
//! Stores user-defined permission rules for auto-approving or auto-rejecting
//! tool executions based on patterns.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A saved permission rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Unique rule ID
    pub id: i64,

    /// Pattern to match (e.g., "Bash:npm", "Edit:*.rs")
    pub pattern: String,

    /// Whether this pattern is allowed (true) or denied (false)
    pub allowed: bool,

    /// Whether this is a persistent rule (true) or session-only (false)
    pub persistent: bool,

    /// When the rule was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Permission rule storage
pub struct PermissionStorage {
    conn: Arc<Mutex<Connection>>,
}

impl PermissionStorage {
    /// Create or open permission storage database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)
            .context("Failed to open permission database")?;

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS permission_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL UNIQUE,
                allowed INTEGER NOT NULL,
                persistent INTEGER NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )
        .context("Failed to create permission_rules table")?;

        // Create index on pattern for fast lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pattern ON permission_rules(pattern)",
            [],
        )
        .context("Failed to create pattern index")?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Save a new permission rule
    pub async fn save_rule(&self, pattern: &str, allowed: bool, persistent: bool) -> Result<i64> {
        let pattern = pattern.to_string();
        let allowed_int = if allowed { 1 } else { 0 };
        let persistent_int = if persistent { 1 } else { 0 };
        let created_at = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            // Use INSERT OR REPLACE to update if pattern already exists
            conn.execute(
                "INSERT OR REPLACE INTO permission_rules (pattern, allowed, persistent, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![pattern, allowed_int, persistent_int, created_at],
            )
            .context("Failed to insert permission rule")?;

            Ok(conn.last_insert_rowid())
        })
        .await
        .context("Task join error")?
    }

    /// Check if a pattern matches any stored rules
    pub async fn check_pattern(&self, pattern: &str) -> Result<Option<bool>> {
        let pattern = pattern.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            let mut stmt = conn
                .prepare("SELECT allowed FROM permission_rules WHERE pattern = ?1")
                .context("Failed to prepare query")?;

            let mut rows = stmt
                .query(params![pattern.as_str()])
                .context("Failed to query permission rule")?;

            if let Some(row) = rows.next().context("Failed to get row")? {
                let allowed: i64 = row.get(0).context("Failed to get allowed column")?;
                return Ok(Some(allowed == 1));
            }

            // Try wildcard patterns
            // For pattern "Bash:npm", try "Bash:*"
            if let Some((tool, _)) = pattern.split_once(':') {
                let wildcard = format!("{}:*", tool);

                let mut stmt = conn
                    .prepare("SELECT allowed FROM permission_rules WHERE pattern = ?1")
                    .context("Failed to prepare query")?;

                let mut rows = stmt
                    .query(params![wildcard.as_str()])
                    .context("Failed to query permission rule")?;

                if let Some(row) = rows.next().context("Failed to get row")? {
                    let allowed: i64 = row.get(0).context("Failed to get allowed column")?;
                    return Ok(Some(allowed == 1));
                }
            }

            Ok(None)
        })
        .await
        .context("Task join error")?
    }

    /// Get all permission rules
    pub async fn get_all_rules(&self) -> Result<Vec<PermissionRule>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            let mut stmt = conn
                .prepare("SELECT id, pattern, allowed, persistent, created_at FROM permission_rules ORDER BY created_at DESC")
                .context("Failed to prepare query")?;

            let rules = stmt
                .query_map([], |row| {
                    Ok(PermissionRule {
                        id: row.get(0)?,
                        pattern: row.get(1)?,
                        allowed: row.get::<_, i64>(2)? == 1,
                        persistent: row.get::<_, i64>(3)? == 1,
                        created_at: row
                            .get::<_, String>(4)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                    })
                })
                .context("Failed to query rules")?
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to collect rules")?;

            Ok(rules)
        })
        .await
        .context("Task join error")?
    }

    /// Delete a permission rule by ID
    pub async fn delete_rule(&self, id: i64) -> Result<()> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            conn.execute("DELETE FROM permission_rules WHERE id = ?1", params![id])
                .context("Failed to delete permission rule")?;

            Ok(())
        })
        .await
        .context("Task join error")?
    }

    /// Delete all session-only rules (called on startup)
    pub async fn clear_session_rules(&self) -> Result<()> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            conn.execute("DELETE FROM permission_rules WHERE persistent = 0", [])
                .context("Failed to clear session rules")?;

            Ok(())
        })
        .await
        .context("Task join error")?
    }

    /// Update a permission rule
    pub async fn update_rule(&self, id: i64, pattern: &str, allowed: bool, persistent: bool) -> Result<()> {
        let pattern = pattern.to_string();
        let allowed_int = if allowed { 1 } else { 0 };
        let persistent_int = if persistent { 1 } else { 0 };
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();

            conn.execute(
                "UPDATE permission_rules SET pattern = ?1, allowed = ?2, persistent = ?3 WHERE id = ?4",
                params![pattern, allowed_int, persistent_int, id],
            )
            .context("Failed to update permission rule")?;

            Ok(())
        })
        .await
        .context("Task join error")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_save_and_check_rule() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Save a rule
        storage.save_rule("Bash:npm", true, true).await.unwrap();

        // Check exact match
        assert_eq!(storage.check_pattern("Bash:npm").await.unwrap(), Some(true));

        // Check no match
        assert_eq!(storage.check_pattern("Bash:cargo").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_wildcard_patterns() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Save wildcard rule
        storage.save_rule("Bash:*", false, true).await.unwrap();

        // Should match any Bash command
        assert_eq!(storage.check_pattern("Bash:npm").await.unwrap(), Some(false));
        assert_eq!(storage.check_pattern("Bash:cargo").await.unwrap(), Some(false));
    }

    #[tokio::test]
    async fn test_session_rules() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Save persistent and session rules
        storage.save_rule("Bash:npm", true, true).await.unwrap();
        storage.save_rule("Bash:cargo", true, false).await.unwrap();

        // Both should be present
        let rules = storage.get_all_rules().await.unwrap();
        assert_eq!(rules.len(), 2);

        // Clear session rules
        storage.clear_session_rules().await.unwrap();

        // Only persistent rule should remain
        let rules = storage.get_all_rules().await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "Bash:npm");
    }
}
