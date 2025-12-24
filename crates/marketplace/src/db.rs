//! Database connection and migrations for marketplace

use rusqlite::{Connection, Result};
use std::path::Path;

/// Current schema version (used in tests)
#[cfg(test)]
const SCHEMA_VERSION: i32 = 2;

/// Initialize the database with migrations
pub fn init_db(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;

    // Configure SQLite for concurrent access (WAL mode) and enable foreign keys
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )?;

    // Check current version
    let version = get_schema_version(&conn)?;

    // Run migrations
    if version < 1 {
        migrate_v1(&conn)?;
    }
    if version < 2 {
        migrate_v2(&conn)?;
    }

    Ok(conn)
}

/// Initialize an in-memory database (for testing)
pub fn init_memory_db() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
    // Note: WAL mode not needed for in-memory, but busy_timeout and foreign_keys still useful
    conn.execute_batch(
        "PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )?;

    let version = get_schema_version(&conn)?;
    if version < 1 {
        migrate_v1(&conn)?;
    }
    if version < 2 {
        migrate_v2(&conn)?;
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
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?)", [version])?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Skills table
        CREATE TABLE IF NOT EXISTS skills (
            id TEXT PRIMARY KEY,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            description TEXT,
            author TEXT,
            version TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'available',
            icon_url TEXT,
            readme TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            installed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_skills_slug ON skills(slug);
        CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status);

        -- Skill versions table (version history)
        CREATE TABLE IF NOT EXISTS skill_versions (
            id TEXT PRIMARY KEY,
            skill_id TEXT NOT NULL,
            version TEXT NOT NULL,
            changelog TEXT,
            published_at TEXT NOT NULL,
            FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE,
            UNIQUE(skill_id, version)
        );

        CREATE INDEX IF NOT EXISTS idx_skill_versions_skill_id ON skill_versions(skill_id);
        CREATE INDEX IF NOT EXISTS idx_skill_versions_version ON skill_versions(version);

        -- Trajectory contributions table
        CREATE TABLE IF NOT EXISTS trajectory_contributions (
            contribution_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            source TEXT NOT NULL,
            trajectory_hash TEXT NOT NULL,
            nostr_event_id TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            quality_score REAL NOT NULL,
            estimated_reward_sats INTEGER NOT NULL,
            actual_reward_sats INTEGER,
            lightning_address TEXT,
            payment_preimage TEXT,
            submitted_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            paid_at TEXT,
            rejection_reason TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_traj_contrib_status ON trajectory_contributions(status);
        CREATE INDEX IF NOT EXISTS idx_traj_contrib_session_id ON trajectory_contributions(session_id);
        CREATE INDEX IF NOT EXISTS idx_traj_contrib_nostr_event_id ON trajectory_contributions(nostr_event_id);
        CREATE INDEX IF NOT EXISTS idx_traj_contrib_submitted_at ON trajectory_contributions(submitted_at);
        "#,
    )?;

    set_schema_version(conn, 1)?;
    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Payment tracking table
        CREATE TABLE IF NOT EXISTS payments (
            id TEXT PRIMARY KEY,
            payment_type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            amount_msats INTEGER NOT NULL,
            invoice TEXT NOT NULL,
            payment_hash TEXT,
            preimage TEXT,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            completed_at INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_payments_item_id ON payments(item_id);
        CREATE INDEX IF NOT EXISTS idx_payments_status ON payments(status);
        CREATE INDEX IF NOT EXISTS idx_payments_type ON payments(payment_type);
        CREATE INDEX IF NOT EXISTS idx_payments_hash ON payments(payment_hash);

        -- Earnings tracking table
        CREATE TABLE IF NOT EXISTS earnings (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            gross_sats INTEGER NOT NULL,
            platform_fee_sats INTEGER NOT NULL,
            net_sats INTEGER NOT NULL,
            period_start INTEGER NOT NULL,
            period_end INTEGER NOT NULL,
            paid_out INTEGER NOT NULL DEFAULT 0,
            payment_preimage TEXT,
            created_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_earnings_type ON earnings(type);
        CREATE INDEX IF NOT EXISTS idx_earnings_item_id ON earnings(item_id);
        CREATE INDEX IF NOT EXISTS idx_earnings_paid_out ON earnings(paid_out);

        -- Revenue buckets table (minute-level tracking)
        CREATE TABLE IF NOT EXISTS revenue_buckets (
            id TEXT PRIMARY KEY,
            bucket_minute INTEGER NOT NULL,
            type TEXT NOT NULL,
            item_id TEXT NOT NULL,
            gross_sats INTEGER NOT NULL,
            creator_sats INTEGER NOT NULL,
            compute_sats INTEGER NOT NULL,
            platform_sats INTEGER NOT NULL,
            referrer_sats INTEGER,
            split_version INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(bucket_minute, type, item_id)
        );

        CREATE INDEX IF NOT EXISTS idx_revenue_buckets_minute ON revenue_buckets(bucket_minute);
        CREATE INDEX IF NOT EXISTS idx_revenue_buckets_type ON revenue_buckets(type);
        CREATE INDEX IF NOT EXISTS idx_revenue_buckets_item_id ON revenue_buckets(item_id);
        "#,
    )?;

    set_schema_version(conn, 2)?;
    Ok(())
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
    fn test_schema_has_skills_table() {
        let conn = init_memory_db().unwrap();
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='skills'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_exists);
    }

    #[test]
    fn test_schema_has_skill_versions_table() {
        let conn = init_memory_db().unwrap();
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='skill_versions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_exists);
    }
}
