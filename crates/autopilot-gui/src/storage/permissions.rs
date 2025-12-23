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

            // Get all rules and check for matches
            let mut stmt = conn
                .prepare("SELECT pattern, allowed FROM permission_rules ORDER BY created_at DESC")
                .context("Failed to prepare query")?;

            let rules = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .context("Failed to query permission rules")?;

            // Check each rule pattern against the input pattern
            for rule in rules {
                let (rule_pattern, allowed) = rule.context("Failed to read rule")?;

                if Self::matches_pattern(&rule_pattern, &pattern) {
                    return Ok(Some(allowed == 1));
                }
            }

            Ok(None)
        })
        .await
        .context("Task join error")?
    }

    /// Check if a stored pattern matches an input pattern
    /// Supports wildcards: * matches any sequence of characters
    /// Examples:
    ///   "Bash:npm" matches "Bash:npm" (exact)
    ///   "Bash:*" matches "Bash:npm" (wildcard)
    ///   "Edit:*.rs" matches "Edit:src/main.rs" (wildcard)
    fn matches_pattern(stored: &str, input: &str) -> bool {
        // Exact match
        if stored == input {
            return true;
        }

        // No wildcards in stored pattern
        if !stored.contains('*') {
            return false;
        }

        // Simple glob matching
        let pattern_parts: Vec<&str> = stored.split('*').collect();

        // Pattern must start with first part (unless starts with *)
        if !stored.starts_with('*') && !input.starts_with(pattern_parts[0]) {
            return false;
        }

        // Pattern must end with last part (unless ends with *)
        if !stored.ends_with('*') && !input.ends_with(pattern_parts[pattern_parts.len() - 1]) {
            return false;
        }

        // Check all parts appear in order
        let mut pos = 0;
        for part in pattern_parts.iter() {
            if part.is_empty() {
                continue;
            }

            if let Some(found_pos) = input[pos..].find(part) {
                pos += found_pos + part.len();
            } else {
                // Required part not found
                return false;
            }
        }

        true
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
    async fn test_wildcard_file_patterns() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Save file extension wildcard rule
        storage.save_rule("Edit:*.rs", true, true).await.unwrap();

        // Should match Rust files
        assert_eq!(storage.check_pattern("Edit:src/main.rs").await.unwrap(), Some(true));
        assert_eq!(storage.check_pattern("Edit:lib.rs").await.unwrap(), Some(true));
        assert_eq!(storage.check_pattern("Edit:tests/integration.rs").await.unwrap(), Some(true));

        // Should not match non-Rust files
        assert_eq!(storage.check_pattern("Edit:src/main.js").await.unwrap(), None);
        assert_eq!(storage.check_pattern("Edit:README.md").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_multiple_wildcards() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Pattern with multiple wildcards
        storage.save_rule("Edit:src/*.rs", true, true).await.unwrap();

        // Should match files in src directory
        assert_eq!(storage.check_pattern("Edit:src/main.rs").await.unwrap(), Some(true));
        assert_eq!(storage.check_pattern("Edit:src/lib.rs").await.unwrap(), Some(true));

        // Should not match files outside src
        assert_eq!(storage.check_pattern("Edit:tests/test.rs").await.unwrap(), None);
        assert_eq!(storage.check_pattern("Edit:main.rs").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_pattern_priority() {
        let temp = NamedTempFile::new().unwrap();
        let storage = PermissionStorage::new(temp.path()).unwrap();

        // Add general rule first
        storage.save_rule("Bash:*", false, true).await.unwrap();

        // Add more specific rule later
        storage.save_rule("Bash:npm", true, true).await.unwrap();

        // More specific (newer) rule should take priority
        assert_eq!(storage.check_pattern("Bash:npm").await.unwrap(), Some(true));

        // General rule still applies to other commands
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

    #[test]
    fn test_matches_pattern() {
        // Exact matches
        assert!(PermissionStorage::matches_pattern("Bash:npm", "Bash:npm"));
        assert!(!PermissionStorage::matches_pattern("Bash:npm", "Bash:cargo"));

        // Simple wildcards
        assert!(PermissionStorage::matches_pattern("Bash:*", "Bash:npm"));
        assert!(PermissionStorage::matches_pattern("Bash:*", "Bash:cargo"));
        assert!(!PermissionStorage::matches_pattern("Bash:*", "Edit:file.rs"));

        // File extension wildcards
        assert!(PermissionStorage::matches_pattern("Edit:*.rs", "Edit:main.rs"));
        assert!(PermissionStorage::matches_pattern("Edit:*.rs", "Edit:src/lib.rs"));
        assert!(!PermissionStorage::matches_pattern("Edit:*.rs", "Edit:main.js"));

        // Path wildcards
        assert!(PermissionStorage::matches_pattern("Edit:src/*.rs", "Edit:src/main.rs"));
        assert!(!PermissionStorage::matches_pattern("Edit:src/*.rs", "Edit:tests/test.rs"));

        // Wildcard at start
        assert!(PermissionStorage::matches_pattern("*.rs", "main.rs"));
        assert!(PermissionStorage::matches_pattern("*.rs", "src/lib.rs"));
        assert!(!PermissionStorage::matches_pattern("*.rs", "main.js"));

        // Multiple wildcards
        assert!(PermissionStorage::matches_pattern("*:*.rs", "Edit:main.rs"));
        assert!(PermissionStorage::matches_pattern("*:*.rs", "Read:lib.rs"));
        assert!(!PermissionStorage::matches_pattern("*:*.rs", "Edit:main.js"));
    }
}
