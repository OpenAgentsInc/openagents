//! Database connection and migrations

use rusqlite::{Connection, Result};
use std::path::Path;

/// Current schema version
const SCHEMA_VERSION: i32 = 1;

/// Initialize the database with migrations
pub fn init_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Enable foreign keys
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Check current version
    let version = get_schema_version(&conn)?;

    // Run migrations
    if version < 1 {
        migrate_v1(&conn)?;
    }

    Ok(conn)
}

/// Initialize an in-memory database (for testing)
pub fn init_memory_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate_v1(&conn)?;
    Ok(conn)
}

fn get_schema_version(conn: &Connection) -> Result<i32> {
    // Create version table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
        [],
    )?;

    let version: Option<i32> = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .ok();

    Ok(version.unwrap_or(0))
}

fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?)", [version])?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Issues table
        CREATE TABLE IF NOT EXISTS issues (
            id TEXT PRIMARY KEY,
            number INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            priority TEXT DEFAULT 'medium',
            issue_type TEXT DEFAULT 'task',
            is_blocked INTEGER DEFAULT 0,
            blocked_reason TEXT,
            claimed_by TEXT,
            claimed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_issues_status ON issues(status);
        CREATE INDEX IF NOT EXISTS idx_issues_number ON issues(number);

        -- Runs table
        CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            prompt TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'running',
            model TEXT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            duration_ms INTEGER,
            tokens_in INTEGER DEFAULT 0,
            tokens_out INTEGER DEFAULT 0,
            cost_usd REAL DEFAULT 0,
            issues_created INTEGER DEFAULT 0,
            issues_completed INTEGER DEFAULT 0,
            error_message TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);

        -- Issue events table
        CREATE TABLE IF NOT EXISTS issue_events (
            id TEXT PRIMARY KEY,
            issue_id TEXT NOT NULL REFERENCES issues(id),
            run_id TEXT REFERENCES runs(id),
            event_type TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_issue_events_issue ON issue_events(issue_id);

        -- Issue counter for sequential numbering
        CREATE TABLE IF NOT EXISTS issue_counter (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            next_number INTEGER NOT NULL DEFAULT 1
        );

        INSERT OR IGNORE INTO issue_counter (id, next_number) VALUES (1, 1);
        "#,
    )?;

    set_schema_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

/// Get the next issue number atomically
pub fn next_issue_number(conn: &Connection) -> Result<i32> {
    let number: i32 = conn.query_row(
        "UPDATE issue_counter SET next_number = next_number + 1 WHERE id = 1 RETURNING next_number - 1",
        [],
        |row| row.get(0),
    )?;
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_memory_db() {
        let conn = init_memory_db().unwrap();
        let version = get_schema_version(&conn).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn test_next_issue_number() {
        let conn = init_memory_db().unwrap();
        assert_eq!(next_issue_number(&conn).unwrap(), 1);
        assert_eq!(next_issue_number(&conn).unwrap(), 2);
        assert_eq!(next_issue_number(&conn).unwrap(), 3);
    }
}
