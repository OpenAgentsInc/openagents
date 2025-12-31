//! Database connection and migrations

use rusqlite::{Connection, Result};
use std::path::Path;
use tracing::{debug, error, info};

/// Current schema version
#[allow(dead_code)]
const SCHEMA_VERSION: i32 = 11;

/// Initialize the database with migrations
pub fn init_db(path: &Path) -> Result<Connection> {
    info!("Initializing database at {:?}", path);

    let conn = Connection::open(path).map_err(|e| {
        error!("Failed to open database at {:?}: {}", path, e);
        e
    })?;

    // Configure SQLite for concurrent access (WAL mode) and enable foreign keys
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| {
        error!("Failed to configure database pragmas: {}", e);
        e
    })?;

    // Check current version
    let version = get_schema_version(&conn)?;
    debug!("Current schema version: {}", version);

    // Run migrations
    if version < 1 {
        migrate_v1(&conn)?;
    }
    if version < 2 {
        migrate_v2(&conn)?;
    }
    if version < 3 {
        migrate_v3(&conn)?;
    }
    if version < 4 {
        migrate_v4(&conn)?;
    }
    if version < 5 {
        migrate_v5(&conn)?;
    }
    if version < 6 {
        migrate_v6(&conn)?;
    }
    if version < 7 {
        migrate_v7(&conn)?;
    }
    if version < 8 {
        migrate_v8(&conn)?;
    }
    if version < 9 {
        migrate_v9(&conn)?;
    }
    if version < 10 {
        migrate_v10(&conn)?;
    }
    if version < 11 {
        migrate_v11(&conn)?;
    }

    Ok(conn)
}

/// Initialize an in-memory database (for testing)
pub fn init_memory_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Ensure schema_version table exists before migration
    let version = get_schema_version(&conn)?;
    if version < 1 {
        migrate_v1(&conn)?;
    }
    if version < 2 {
        migrate_v2(&conn)?;
    }
    if version < 3 {
        migrate_v3(&conn)?;
    }
    if version < 4 {
        migrate_v4(&conn)?;
    }
    if version < 5 {
        migrate_v5(&conn)?;
    }
    if version < 6 {
        migrate_v6(&conn)?;
    }
    if version < 7 {
        migrate_v7(&conn)?;
    }
    if version < 8 {
        migrate_v8(&conn)?;
    }
    if version < 9 {
        migrate_v9(&conn)?;
    }
    if version < 10 {
        migrate_v10(&conn)?;
    }
    if version < 11 {
        migrate_v11(&conn)?;
    }

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
    debug!("Setting schema version to {}", version);
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?)", [version])?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    info!("Running migration v1");
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

        -- Issue events table (audit log)
        CREATE TABLE IF NOT EXISTS issue_events (
            id TEXT PRIMARY KEY,
            issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
            actor TEXT,
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

    set_schema_version(conn, 1).map_err(|e| {
        error!("Failed to set schema version to 1: {}", e);
        e
    })?;
    info!("Migration v1 completed successfully");
    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<()> {
    info!("Running migration v2");
    // Clean up any NULL ids (from manual inserts) and delete those rows
    let deleted_issues = conn
        .execute("DELETE FROM issues WHERE id IS NULL OR id = ''", [])
        .map_err(|e| {
            error!("Failed to clean up NULL issue ids: {}", e);
            e
        })?;
    if deleted_issues > 0 {
        info!(
            "Deleted {} issue(s) with NULL or empty IDs during migration v2",
            deleted_issues
        );
    }

    let deleted_events = conn
        .execute("DELETE FROM issue_events WHERE id IS NULL OR id = ''", [])
        .map_err(|e| {
            error!("Failed to clean up NULL event ids: {}", e);
            e
        })?;
    if deleted_events > 0 {
        info!(
            "Deleted {} issue_event(s) with NULL or empty IDs during migration v2",
            deleted_events
        );
    }

    // Recreate issues table with explicit NOT NULL constraint on id
    conn.execute_batch(
        r#"
        -- Create new table with proper constraints
        CREATE TABLE issues_new (
            id TEXT NOT NULL PRIMARY KEY,
            number INTEGER NOT NULL UNIQUE,
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
            completed_at TEXT,
            CHECK(id != '')
        );

        -- Copy data from old table
        INSERT INTO issues_new SELECT * FROM issues WHERE id IS NOT NULL AND id != '';

        -- Drop old table
        DROP TABLE issues;

        -- Rename new table
        ALTER TABLE issues_new RENAME TO issues;

        -- Recreate indexes
        CREATE INDEX idx_issues_status ON issues(status);
        CREATE INDEX idx_issues_number ON issues(number);
        "#,
    )?;

    set_schema_version(conn, 2)?;
    info!("Migration v2 completed successfully");
    Ok(())
}

fn migrate_v3(conn: &Connection) -> Result<()> {
    info!("Running migration v3");
    // Sync issue_counter to MAX(number) + 1 to fix any desync from manual inserts
    conn.execute_batch(
        r#"
        UPDATE issue_counter
        SET next_number = COALESCE((SELECT MAX(number) + 1 FROM issues), 1)
        WHERE id = 1;

        -- Create trigger to keep counter in sync when issues are inserted
        -- This handles manual sqlite3 inserts that bypass the API
        CREATE TRIGGER IF NOT EXISTS sync_issue_counter_on_insert
        AFTER INSERT ON issues
        BEGIN
            UPDATE issue_counter
            SET next_number = MAX(next_number, NEW.number + 1)
            WHERE id = 1;
        END;
        "#,
    )?;

    set_schema_version(conn, 3)?;
    info!("Migration v3 completed successfully");
    Ok(())
}

fn migrate_v4(conn: &Connection) -> Result<()> {
    info!("Running migration v4");
    // Add projects and sessions tables for multi-project Autopilot support
    conn.execute_batch(
        r#"
        -- Projects table
        CREATE TABLE IF NOT EXISTS projects (
            id TEXT NOT NULL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL,
            description TEXT,
            default_model TEXT,
            default_budget REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(id != ''),
            CHECK(name != '')
        );

        CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);

        -- Sessions table
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT NOT NULL PRIMARY KEY,
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            status TEXT NOT NULL DEFAULT 'running',
            prompt TEXT NOT NULL,
            model TEXT NOT NULL,
            pid INTEGER,
            trajectory_path TEXT,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            budget_spent REAL DEFAULT 0.0,
            issues_completed INTEGER DEFAULT 0,
            CHECK(id != ''),
            CHECK(status IN ('running', 'completed', 'failed', 'cancelled'))
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
        CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at);
        "#,
    )?;

    set_schema_version(conn, 4)?;
    info!("Migration v4 completed successfully");
    Ok(())
}

fn migrate_v5(conn: &Connection) -> Result<()> {
    info!("Running migration v5");
    // Add agent column to issues table for Codex integration
    conn.execute_batch(
        r#"
        -- Add agent column (default: 'claude')
        ALTER TABLE issues ADD COLUMN agent TEXT NOT NULL DEFAULT 'claude';
        "#,
    )?;

    set_schema_version(conn, 5)?;
    info!("Migration v5 completed successfully");
    Ok(())
}

fn migrate_v6(conn: &Connection) -> Result<()> {
    info!("Running migration v6");
    // Add directive_id column to issues table for linking issues to directives
    conn.execute_batch(
        r#"
        -- Add directive_id column for linking issues to directives
        ALTER TABLE issues ADD COLUMN directive_id TEXT;

        -- Index for efficient directive lookups
        CREATE INDEX IF NOT EXISTS idx_issues_directive ON issues(directive_id);
        "#,
    )?;

    set_schema_version(conn, 6)?;
    info!("Migration v6 completed successfully");
    Ok(())
}

fn migrate_v7(conn: &Connection) -> Result<()> {
    info!("Running migration v7");
    // Add project_id column to issues table for project-scoped issues
    conn.execute_batch(
        r#"
        -- Add project_id column for linking issues to projects
        ALTER TABLE issues ADD COLUMN project_id TEXT;

        -- Index for efficient project lookups
        CREATE INDEX IF NOT EXISTS idx_issues_project ON issues(project_id);
        "#,
    )?;

    set_schema_version(conn, 7)?;
    info!("Migration v7 completed successfully");
    Ok(())
}

fn migrate_v8(conn: &Connection) -> Result<()> {
    info!("Running migration v8");
    // Add ON DELETE CASCADE to issue_events foreign key
    conn.execute_batch(
        r#"
        -- Create new table with CASCADE delete
        CREATE TABLE issue_events_new (
            id TEXT PRIMARY KEY,
            issue_id TEXT NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
            actor TEXT,
            event_type TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            created_at TEXT NOT NULL
        );

        -- Copy data from old table
        INSERT INTO issue_events_new SELECT * FROM issue_events;

        -- Drop old table
        DROP TABLE issue_events;

        -- Rename new table
        ALTER TABLE issue_events_new RENAME TO issue_events;

        -- Recreate index
        CREATE INDEX idx_issue_events_issue ON issue_events(issue_id);
        "#,
    )?;

    set_schema_version(conn, 8)?;
    info!("Migration v8 completed successfully");
    Ok(())
}

fn migrate_v9(conn: &Connection) -> Result<()> {
    info!("Running migration v9");
    // Add index on agent column for efficient agent filtering
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_issues_agent ON issues(agent);
        "#,
    )?;

    set_schema_version(conn, 9)?;
    info!("Migration v9 completed successfully");
    Ok(())
}

fn migrate_v10(conn: &Connection) -> Result<()> {
    info!("Running migration v10");
    // Add index on claimed_by column for efficient claim expiry queries
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_issues_claimed_by ON issues(claimed_by);
        "#,
    )?;

    set_schema_version(conn, 10)?;
    info!("Migration v10 completed successfully");
    Ok(())
}

fn migrate_v11(conn: &Connection) -> Result<()> {
    info!("Running migration v11");
    // Add auto_created field to track issues created by automated detection
    conn.execute_batch(
        r#"
        ALTER TABLE issues ADD COLUMN auto_created INTEGER DEFAULT 0;
        "#,
    )?;

    set_schema_version(conn, 11)?;
    info!("Migration v11 completed successfully");
    info!(
        "Database initialized successfully at schema version {}",
        SCHEMA_VERSION
    );
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

    #[test]
    fn test_counter_auto_resync_on_manual_insert() {
        let conn = init_memory_db().unwrap();

        // Get first number normally
        assert_eq!(next_issue_number(&conn).unwrap(), 1);

        // Manually insert an issue with a high number (simulating manual sqlite3 insert)
        conn.execute(
            "INSERT INTO issues (id, number, title, description, status, created_at, updated_at)
             VALUES ('manual-1', 100, 'Manual Issue', 'Test', 'open', datetime('now'), datetime('now'))",
            []
        ).unwrap();

        // The trigger should have updated the counter to 101
        assert_eq!(next_issue_number(&conn).unwrap(), 101);
        assert_eq!(next_issue_number(&conn).unwrap(), 102);
    }

    #[test]
    fn test_unique_constraint_on_number() {
        let conn = init_memory_db().unwrap();

        // Create first issue
        conn.execute(
            "INSERT INTO issues (id, number, title, description, status, created_at, updated_at)
             VALUES ('test-1', 1, 'First', 'Test', 'open', datetime('now'), datetime('now'))",
            [],
        )
        .unwrap();

        // Try to create second issue with same number - should fail
        let result = conn.execute(
            "INSERT INTO issues (id, number, title, description, status, created_at, updated_at)
             VALUES ('test-2', 1, 'Second', 'Test', 'open', datetime('now'), datetime('now'))",
            [],
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_not_null_constraint_on_id() {
        let conn = init_memory_db().unwrap();

        // Try to insert issue with NULL id - should fail
        let result = conn.execute(
            "INSERT INTO issues (id, number, title, description, status, created_at, updated_at)
             VALUES (NULL, 1, 'Test', 'Test', 'open', datetime('now'), datetime('now'))",
            [],
        );

        assert!(result.is_err());

        // Try to insert issue with empty id - should also fail (CHECK constraint)
        let result = conn.execute(
            "INSERT INTO issues (id, number, title, description, status, created_at, updated_at)
             VALUES ('', 2, 'Test', 'Test', 'open', datetime('now'), datetime('now'))",
            [],
        );

        assert!(result.is_err());
    }
}
