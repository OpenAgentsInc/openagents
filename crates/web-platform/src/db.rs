// Database layer for web platform

use rusqlite::Connection;
use anyhow::Result;
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[allow(dead_code)]
impl Database {
    pub fn new(database_path: &str) -> Result<Self> {
        let conn = Connection::open(database_path)?;

        // Run migrations
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                github_id INTEGER UNIQUE NOT NULL,
                github_login TEXT NOT NULL,
                email TEXT,
                credits INTEGER NOT NULL DEFAULT 10000,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            [],
        )?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                repo_url TEXT NOT NULL,
                task TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                pr_url TEXT,
                files_modified INTEGER,
                tests_passed INTEGER,
                credits_used INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id)
            )
            "#,
            [],
        )?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                type TEXT NOT NULL,
                amount INTEGER NOT NULL,
                stripe_session_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id)
            )
            "#,
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_init() {
        let db = Database::new(":memory:").unwrap();
        assert!(db.conn().lock().is_ok());
    }
}
