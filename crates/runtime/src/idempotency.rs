//! Idempotency journal implementations.

use crate::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(feature = "cloudflare")]
use worker::sql::{SqlStorage, SqlStorageValue};

/// Journal result type.
pub type JournalResult<T> = std::result::Result<T, JournalError>;

/// Errors for the idempotency journal.
#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    /// Underlying storage error.
    #[error("storage error: {0}")]
    Storage(String),

    /// SQLite error.
    #[cfg(feature = "local")]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Idempotency journal entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Entry key.
    pub key: String,
    /// Expiration timestamp (millis).
    pub expires_at: Timestamp,
}

/// Trait for idempotency journals.
pub trait IdempotencyJournal: Send + Sync {
    /// Fetch a cached value by key.
    fn get(&self, key: &str) -> JournalResult<Option<Vec<u8>>>;

    /// Store a value with TTL.
    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> JournalResult<()>;

    /// Check if key exists and is not expired.
    fn contains(&self, key: &str) -> JournalResult<bool> {
        Ok(self.get(key)?.is_some())
    }

    /// Record key with TTL. Returns true if recorded, false if already exists.
    fn check_or_record(&self, key: &str, ttl: Duration) -> JournalResult<bool> {
        if self.contains(key)? {
            return Ok(false);
        }
        self.put_with_ttl(key, &[], ttl)?;
        Ok(true)
    }

    /// Remove expired entries and return count removed.
    fn cleanup(&self) -> JournalResult<usize>;
}

#[derive(Clone)]
struct MemoryEntry {
    expires_at: Timestamp,
    value: Vec<u8>,
}

/// In-memory idempotency journal.
#[derive(Clone, Default)]
pub struct MemoryJournal {
    entries: Arc<Mutex<HashMap<String, MemoryEntry>>>,
}

impl MemoryJournal {
    /// Create a new memory journal.
    pub fn new() -> Self {
        Self::default()
    }

    fn purge_expired(&self, now: Timestamp) -> usize {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let before = entries.len();
        entries.retain(|_, entry| entry.expires_at.as_millis() > now.as_millis());
        before.saturating_sub(entries.len())
    }
}

impl IdempotencyJournal for MemoryJournal {
    fn get(&self, key: &str) -> JournalResult<Option<Vec<u8>>> {
        let now = Timestamp::now();
        self.purge_expired(now);
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        Ok(entries.get(key).map(|entry| entry.value.clone()))
    }

    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> JournalResult<()> {
        let now = Timestamp::now();
        self.purge_expired(now);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let expires_at = Timestamp::from_millis(now.as_millis() + ttl.as_millis() as u64);
        entries.insert(
            key.to_string(),
            MemoryEntry {
                expires_at,
                value: value.to_vec(),
            },
        );
        Ok(())
    }

    fn check_or_record(&self, key: &str, ttl: Duration) -> JournalResult<bool> {
        let now = Timestamp::now();
        self.purge_expired(now);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.contains_key(key) {
            return Ok(false);
        }
        let expires_at = Timestamp::from_millis(now.as_millis() + ttl.as_millis() as u64);
        entries.insert(
            key.to_string(),
            MemoryEntry {
                expires_at,
                value: Vec::new(),
            },
        );
        Ok(true)
    }

    fn cleanup(&self) -> JournalResult<usize> {
        let now = Timestamp::now();
        Ok(self.purge_expired(now))
    }
}

/// Cloudflare Durable Object SQL-backed idempotency journal.
#[cfg(feature = "cloudflare")]
#[derive(Clone)]
pub struct DoJournal {
    sql: SqlStorage,
    prefix: String,
}

#[cfg(feature = "cloudflare")]
impl DoJournal {
    /// Create a journal scoped by prefix in the DO SQL database.
    pub fn new(sql: SqlStorage, prefix: impl Into<String>) -> JournalResult<Self> {
        let journal = Self {
            sql,
            prefix: prefix.into(),
        };
        journal.init()?;
        Ok(journal)
    }

    fn init(&self) -> JournalResult<()> {
        self.sql.exec(
            "CREATE TABLE IF NOT EXISTS idempotency_journal (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                expires_at INTEGER NOT NULL
            );",
            None,
        )?;
        Ok(())
    }

    fn scoped_key(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    fn exec(
        &self,
        query: &str,
        bindings: Option<Vec<SqlStorageValue>>,
    ) -> JournalResult<worker::sql::SqlCursor> {
        self.sql.exec(query, bindings).map_err(JournalError::from)
    }
}

#[cfg(feature = "cloudflare")]
#[derive(Deserialize)]
struct DoValueRow {
    value: Vec<u8>,
}

#[cfg(feature = "cloudflare")]
impl IdempotencyJournal for DoJournal {
    fn get(&self, key: &str) -> JournalResult<Option<Vec<u8>>> {
        let now_ms = Timestamp::now().as_millis() as i64;
        let scoped = self.scoped_key(key);
        let cursor = self.exec(
            "SELECT value FROM idempotency_journal WHERE key = ?1 AND expires_at > ?2",
            Some(vec![
                SqlStorageValue::from(scoped.as_str()),
                SqlStorageValue::from(now_ms),
            ]),
        )?;
        let rows: Vec<DoValueRow> = cursor.to_array().map_err(JournalError::from)?;
        Ok(rows.into_iter().next().map(|row| row.value))
    }

    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> JournalResult<()> {
        let now_ms = Timestamp::now().as_millis() as i64;
        let expires_at = now_ms + ttl.as_millis() as i64;
        let scoped = self.scoped_key(key);
        self.exec(
            "INSERT OR REPLACE INTO idempotency_journal (key, value, expires_at) VALUES (?1, ?2, ?3)",
            Some(vec![
                SqlStorageValue::from(scoped.as_str()),
                SqlStorageValue::from(value.to_vec()),
                SqlStorageValue::from(expires_at),
            ]),
        )?;
        Ok(())
    }

    fn cleanup(&self) -> JournalResult<usize> {
        let now_ms = Timestamp::now().as_millis() as i64;
        let cursor = self.exec(
            "DELETE FROM idempotency_journal WHERE expires_at <= ?1",
            Some(vec![SqlStorageValue::from(now_ms)]),
        )?;
        Ok(cursor.rows_written())
    }
}

/// SQLite-backed idempotency journal.
#[cfg(feature = "local")]
#[derive(Clone)]
pub struct SqliteJournal {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

#[cfg(feature = "local")]
impl SqliteJournal {
    /// Open or create a journal at the given path.
    pub fn new(path: impl AsRef<std::path::Path>) -> JournalResult<Self> {
        let conn = rusqlite::Connection::open(path)?;
        let journal = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        journal.init()?;
        Ok(journal)
    }

    fn init(&self) -> JournalResult<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS idempotency_journal (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL DEFAULT x'',
                expires_at INTEGER NOT NULL
            );
            ",
        )?;
        let mut stmt = conn.prepare("PRAGMA table_info(idempotency_journal)")?;
        let mut rows = stmt.query([])?;
        let mut has_value = false;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "value" {
                has_value = true;
                break;
            }
        }
        if !has_value {
            conn.execute(
                "ALTER TABLE idempotency_journal ADD COLUMN value BLOB NOT NULL DEFAULT x''",
                [],
            )?;
        }
        Ok(())
    }

    fn purge_expired(&self, now_ms: u64) -> JournalResult<usize> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let count = conn.execute(
            "DELETE FROM idempotency_journal WHERE expires_at <= ?1",
            rusqlite::params![now_ms as i64],
        )?;
        Ok(count)
    }
}

#[cfg(feature = "local")]
impl IdempotencyJournal for SqliteJournal {
    fn get(&self, key: &str) -> JournalResult<Option<Vec<u8>>> {
        let now_ms = Timestamp::now().as_millis();
        self.purge_expired(now_ms)?;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(
            "SELECT value FROM idempotency_journal WHERE key = ?1 AND expires_at > ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![key, now_ms as i64])?;
        if let Some(row) = rows.next()? {
            let value: Vec<u8> = row.get(0)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    fn put_with_ttl(&self, key: &str, value: &[u8], ttl: Duration) -> JournalResult<()> {
        let now_ms = Timestamp::now().as_millis();
        let expires_at = now_ms + ttl.as_millis() as u64;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO idempotency_journal (key, value, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, value, expires_at as i64],
        )?;
        Ok(())
    }

    fn check_or_record(&self, key: &str, ttl: Duration) -> JournalResult<bool> {
        let now_ms = Timestamp::now().as_millis();
        let expires_at = now_ms + ttl.as_millis() as u64;
        let mut conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM idempotency_journal WHERE expires_at <= ?1",
            rusqlite::params![now_ms as i64],
        )?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO idempotency_journal (key, value, expires_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, Vec::<u8>::new(), expires_at as i64],
        )?;
        tx.commit()?;
        Ok(inserted > 0)
    }

    fn cleanup(&self) -> JournalResult<usize> {
        let now_ms = Timestamp::now().as_millis();
        self.purge_expired(now_ms)
    }
}

#[cfg(feature = "cloudflare")]
impl From<worker::Error> for JournalError {
    fn from(err: worker::Error) -> Self {
        JournalError::Storage(err.to_string())
    }
}
