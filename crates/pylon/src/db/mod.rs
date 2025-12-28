//! SQLite persistence for Pylon
//!
//! Provides persistent storage for:
//! - Job history (provider mode)
//! - Earnings tracking
//! - Agent state (host mode)

pub mod agents;
pub mod earnings;
pub mod jobs;

use rusqlite::Connection;
use std::path::Path;

/// Database wrapper for Pylon
pub struct PylonDb {
    conn: Connection,
}

impl PylonDb {
    /// Open or create the database at the given path
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrency
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;

        let db = Self { conn };
        db.migrate()?;

        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Run all migrations
    fn migrate(&self) -> anyhow::Result<()> {
        // Create migrations table if it doesn't exist
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch())
            )",
            [],
        )?;

        // Run migrations in order
        self.run_migration("001_initial_schema", MIGRATION_001)?;
        self.run_migration("002_invoices", MIGRATION_002)?;

        Ok(())
    }

    /// Run a single migration if not already applied
    fn run_migration(&self, name: &str, sql: &str) -> anyhow::Result<()> {
        // Check if already applied
        let applied: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM migrations WHERE name = ?)",
                [name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if applied {
            return Ok(());
        }

        // Run migration
        self.conn.execute_batch(sql)?;

        // Mark as applied
        self.conn.execute(
            "INSERT INTO migrations (name) VALUES (?)",
            [name],
        )?;

        tracing::info!("Applied migration: {}", name);
        Ok(())
    }

    /// Get a reference to the connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

/// Initial schema migration
const MIGRATION_001: &str = r#"
-- Jobs table (provider mode)
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    kind INTEGER NOT NULL,
    customer_pubkey TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'cancelled')),
    price_msats INTEGER NOT NULL DEFAULT 0,
    input_hash TEXT,
    output_hash TEXT,
    error_message TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_customer ON jobs(customer_pubkey);
CREATE INDEX IF NOT EXISTS idx_jobs_created ON jobs(created_at);

-- Earnings table
CREATE TABLE IF NOT EXISTS earnings (
    id TEXT PRIMARY KEY,
    job_id TEXT REFERENCES jobs(id),
    amount_msats INTEGER NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('job', 'tip', 'other')),
    payment_hash TEXT,
    preimage TEXT,
    earned_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_earnings_job ON earnings(job_id);
CREATE INDEX IF NOT EXISTS idx_earnings_date ON earnings(earned_at);

-- Agent state table (host mode)
CREATE TABLE IF NOT EXISTS agents (
    npub TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL CHECK(lifecycle_state IN ('embryonic', 'active', 'dormant', 'terminated')),
    balance_sats INTEGER NOT NULL DEFAULT 0,
    tick_count INTEGER NOT NULL DEFAULT 0,
    last_tick_at INTEGER,
    memory_json TEXT,
    goals_json TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_agents_state ON agents(lifecycle_state);

-- Tick history for agents
CREATE TABLE IF NOT EXISTS tick_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_npub TEXT NOT NULL REFERENCES agents(npub),
    tick_number INTEGER NOT NULL,
    prompt_tokens INTEGER,
    completion_tokens INTEGER,
    actions_json TEXT,
    cost_sats INTEGER,
    duration_ms INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_tick_history_agent ON tick_history(agent_npub);
CREATE INDEX IF NOT EXISTS idx_tick_history_date ON tick_history(created_at);
"#;

/// Invoices table migration
const MIGRATION_002: &str = r#"
-- Invoices table for payment tracking
CREATE TABLE IF NOT EXISTS invoices (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL REFERENCES jobs(id),
    bolt11 TEXT NOT NULL,
    amount_msats INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'paid', 'expired', 'cancelled')) DEFAULT 'pending',
    paid_amount_msats INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    paid_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_invoices_job ON invoices(job_id);
CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices(status);
CREATE INDEX IF NOT EXISTS idx_invoices_created ON invoices(created_at);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory() {
        let db = PylonDb::open_in_memory().expect("Failed to open in-memory db");

        // Verify tables exist
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='jobs'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrations_idempotent() {
        let db = PylonDb::open_in_memory().expect("Failed to open db");

        // Run migrate again - should be idempotent
        db.migrate().expect("Second migration should not fail");
    }
}
