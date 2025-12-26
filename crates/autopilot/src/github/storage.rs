//! Storage operations for connected GitHub repositories

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::models::{ConnectedRepo, TokenInfo};

/// Store a newly connected repository
pub fn store_connected_repo(
    conn: &Connection,
    owner: &str,
    repo: &str,
    token: &TokenInfo,
    default_branch: &str,
    languages: &[String],
) -> Result<i64> {
    let full_name = format!("{}/{}", owner, repo);
    let languages_json = serde_json::to_string(languages)?;
    let expires_at = token.expires_at.map(|t| t.to_rfc3339());

    conn.execute(
        r#"
        INSERT INTO connected_repos (owner, repo, full_name, access_token, refresh_token,
                                     token_expires_at, default_branch, languages)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(full_name) DO UPDATE SET
            access_token = excluded.access_token,
            refresh_token = excluded.refresh_token,
            token_expires_at = excluded.token_expires_at,
            default_branch = excluded.default_branch,
            languages = excluded.languages,
            last_sync_at = datetime('now')
        "#,
        params![
            owner,
            repo,
            full_name,
            token.access_token,
            token.refresh_token,
            expires_at,
            default_branch,
            languages_json,
        ],
    )
    .context("Failed to store connected repo")?;

    let id = conn.last_insert_rowid();
    Ok(id)
}

/// Get a connected repository by full name (owner/repo)
pub fn get_connected_repo(conn: &Connection, full_name: &str) -> Result<Option<ConnectedRepo>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, owner, repo, full_name, default_branch, languages, connected_at
        FROM connected_repos
        WHERE full_name = ?1
        "#,
    )?;

    let result = stmt.query_row([full_name], |row| {
        let languages_json: String = row.get(5)?;
        let languages: Vec<String> =
            serde_json::from_str(&languages_json).unwrap_or_default();
        let connected_at_str: String = row.get(6)?;
        let connected_at = DateTime::parse_from_rfc3339(&connected_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(ConnectedRepo {
            id: row.get(0)?,
            owner: row.get(1)?,
            repo: row.get(2)?,
            full_name: row.get(3)?,
            default_branch: row.get(4)?,
            languages,
            connected_at,
        })
    });

    match result {
        Ok(repo) => Ok(Some(repo)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get access token for a connected repository
pub fn get_repo_token(conn: &Connection, full_name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT access_token FROM connected_repos WHERE full_name = ?1",
    )?;

    let result = stmt.query_row([full_name], |row| row.get(0));

    match result {
        Ok(token) => Ok(Some(token)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List all connected repositories
pub fn list_connected_repos(conn: &Connection) -> Result<Vec<ConnectedRepo>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT id, owner, repo, full_name, default_branch, languages, connected_at
        FROM connected_repos
        ORDER BY connected_at DESC
        "#,
    )?;

    let repos = stmt
        .query_map([], |row| {
            let languages_json: String = row.get(5)?;
            let languages: Vec<String> =
                serde_json::from_str(&languages_json).unwrap_or_default();
            let connected_at_str: String = row.get(6)?;
            let connected_at = DateTime::parse_from_rfc3339(&connected_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(ConnectedRepo {
                id: row.get(0)?,
                owner: row.get(1)?,
                repo: row.get(2)?,
                full_name: row.get(3)?,
                default_branch: row.get(4)?,
                languages,
                connected_at,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(repos)
}

/// Update token for a connected repository
pub fn update_repo_token(conn: &Connection, full_name: &str, token: &TokenInfo) -> Result<()> {
    let expires_at = token.expires_at.map(|t| t.to_rfc3339());

    conn.execute(
        r#"
        UPDATE connected_repos
        SET access_token = ?1, refresh_token = ?2, token_expires_at = ?3
        WHERE full_name = ?4
        "#,
        params![
            token.access_token,
            token.refresh_token,
            expires_at,
            full_name,
        ],
    )
    .context("Failed to update repo token")?;

    Ok(())
}

/// Delete a connected repository
pub fn delete_connected_repo(conn: &Connection, full_name: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM connected_repos WHERE full_name = ?1",
        [full_name],
    )?;

    Ok(rows > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsDb;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, Connection) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize the database with migrations
        let _metrics_db = MetricsDb::open(&db_path).unwrap();
        let conn = Connection::open(&db_path).unwrap();

        (temp_dir, conn)
    }

    #[test]
    fn test_store_and_get_repo() {
        let (_temp, conn) = setup_test_db();

        let token = TokenInfo {
            access_token: "test_token".to_string(),
            token_type: "bearer".to_string(),
            scope: "repo".to_string(),
            refresh_token: None,
            expires_at: None,
        };

        let id = store_connected_repo(
            &conn,
            "openagents",
            "openagents",
            &token,
            "main",
            &["Rust".to_string(), "TypeScript".to_string()],
        )
        .unwrap();

        assert!(id > 0);

        let repo = get_connected_repo(&conn, "openagents/openagents")
            .unwrap()
            .unwrap();

        assert_eq!(repo.owner, "openagents");
        assert_eq!(repo.repo, "openagents");
        assert_eq!(repo.default_branch, "main");
        assert_eq!(repo.languages, vec!["Rust", "TypeScript"]);
    }

    #[test]
    fn test_list_repos() {
        let (_temp, conn) = setup_test_db();

        let token = TokenInfo {
            access_token: "test_token".to_string(),
            token_type: "bearer".to_string(),
            scope: "repo".to_string(),
            refresh_token: None,
            expires_at: None,
        };

        store_connected_repo(&conn, "owner1", "repo1", &token, "main", &[]).unwrap();
        store_connected_repo(&conn, "owner2", "repo2", &token, "master", &[]).unwrap();

        let repos = list_connected_repos(&conn).unwrap();
        assert_eq!(repos.len(), 2);
    }
}
