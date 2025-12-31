//! SQLite storage layer with connection pooling
//!
//! The database layer uses three separate connection pools:
//! - Writer pool: Single connection for writes (SQLite write lock)
//! - Reader pool: Multiple connections for reads
//! - Metadata pool: Separate connections for metadata queries
//!
//! This architecture maximizes read throughput while ensuring write consistency.

use crate::error::{RelayError, Result};
use nostr::Event;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, params};
use std::path::PathBuf;
use tracing::{debug, info};

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Path to the SQLite database file
    pub path: PathBuf,
    /// Maximum number of reader connections
    pub max_reader_connections: u32,
    /// Maximum number of metadata connections
    pub max_metadata_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("relay.db"),
            max_reader_connections: 10,
            max_metadata_connections: 5,
        }
    }
}

/// Connection pool wrapper managing writer, reader, and metadata pools
pub struct ConnectionPool {
    writer: Pool<SqliteConnectionManager>,
    reader: Pool<SqliteConnectionManager>,
    metadata: Pool<SqliteConnectionManager>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(config: &DatabaseConfig) -> Result<Self> {
        // Writer pool (single connection for writes)
        let writer_manager = SqliteConnectionManager::file(&config.path);
        let writer = Pool::builder()
            .max_size(1)
            .build(writer_manager)
            .map_err(RelayError::Pool)?;

        // Reader pool (multiple connections for reads)
        let reader_manager = SqliteConnectionManager::file(&config.path);
        let reader = Pool::builder()
            .max_size(config.max_reader_connections)
            .build(reader_manager)
            .map_err(RelayError::Pool)?;

        // Metadata pool (separate connections for metadata queries)
        let metadata_manager = SqliteConnectionManager::file(&config.path);
        let metadata = Pool::builder()
            .max_size(config.max_metadata_connections)
            .build(metadata_manager)
            .map_err(RelayError::Pool)?;

        Ok(Self {
            writer,
            reader,
            metadata,
        })
    }

    /// Get a writer connection
    pub fn writer(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.writer.get().map_err(RelayError::Pool)
    }

    /// Get a reader connection
    pub fn reader(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.reader.get().map_err(RelayError::Pool)
    }

    /// Get a metadata connection
    pub fn metadata(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.metadata.get().map_err(RelayError::Pool)
    }
}

/// Main database interface
pub struct Database {
    pool: ConnectionPool,
}

impl Database {
    /// Create a new database instance
    pub fn new(config: DatabaseConfig) -> Result<Self> {
        let pool = ConnectionPool::new(&config)?;

        // Initialize schema using writer connection
        let conn = pool.writer()?;
        Self::init_schema(&conn)?;

        info!("Database initialized at {:?}", config.path);

        Ok(Self { pool })
    }

    /// Initialize the database schema
    fn init_schema(conn: &Connection) -> Result<()> {
        // Events table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                pubkey TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                kind INTEGER NOT NULL,
                content TEXT NOT NULL,
                sig TEXT NOT NULL,
                tags TEXT NOT NULL,
                raw_event TEXT NOT NULL,
                first_seen INTEGER NOT NULL
            )",
            [],
        )?;

        // Indexes for common queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_pubkey ON events(pubkey)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at)",
            [],
        )?;

        // Tags table for efficient tag queries
        conn.execute(
            "CREATE TABLE IF NOT EXISTS event_tags (
                event_id TEXT NOT NULL,
                tag_name TEXT NOT NULL,
                tag_value TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_event_tags_event_id ON event_tags(event_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_event_tags_name_value ON event_tags(tag_name, tag_value)",
            [],
        )?;

        debug!("Database schema initialized");
        Ok(())
    }

    /// Store an event in the database
    pub fn store_event(&self, event: &Event) -> Result<()> {
        let conn = self.pool.writer()?;

        // Serialize the full event as JSON for storage
        let raw_event = serde_json::to_string(event)?;
        let tags_json = serde_json::to_string(&event.tags)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert event
        conn.execute(
            "INSERT OR REPLACE INTO events (id, pubkey, created_at, kind, content, sig, tags, raw_event, first_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &event.id,
                &event.pubkey,
                event.created_at,
                event.kind,
                &event.content,
                &event.sig,
                tags_json,
                raw_event,
                now,
            ],
        )?;

        // Insert tags for efficient querying
        for tag in &event.tags {
            if !tag.is_empty() {
                let tag_name = &tag[0];
                let tag_value = tag.get(1).map(|s| s.as_str());

                conn.execute(
                    "INSERT INTO event_tags (event_id, tag_name, tag_value) VALUES (?1, ?2, ?3)",
                    params![&event.id, tag_name, tag_value],
                )?;
            }
        }

        debug!("Stored event {}", event.id);
        Ok(())
    }

    /// Get an event by ID
    pub fn get_event(&self, event_id: &str) -> Result<Option<Event>> {
        let conn = self.pool.reader()?;

        let mut stmt = conn.prepare("SELECT raw_event FROM events WHERE id = ?1")?;

        let result = stmt.query_row(params![event_id], |row| {
            let raw_event: String = row.get(0)?;
            Ok(raw_event)
        });

        match result {
            Ok(raw_event) => {
                let event: Event = serde_json::from_str(&raw_event)?;
                Ok(Some(event))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(RelayError::Database(e)),
        }
    }

    /// Get events by pubkey
    pub fn get_events_by_pubkey(&self, pubkey: &str, limit: usize) -> Result<Vec<Event>> {
        let conn = self.pool.reader()?;

        let mut stmt = conn.prepare(
            "SELECT raw_event FROM events WHERE pubkey = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pubkey, limit as i64], |row| {
            let raw_event: String = row.get(0)?;
            Ok(raw_event)
        })?;

        let mut events = Vec::new();
        for row in rows {
            let raw_event = row?;
            let event: Event = serde_json::from_str(&raw_event)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get events by kind
    pub fn get_events_by_kind(&self, kind: u16, limit: usize) -> Result<Vec<Event>> {
        let conn = self.pool.reader()?;

        let mut stmt = conn.prepare(
            "SELECT raw_event FROM events WHERE kind = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![kind, limit as i64], |row| {
            let raw_event: String = row.get(0)?;
            Ok(raw_event)
        })?;

        let mut events = Vec::new();
        for row in rows {
            let raw_event = row?;
            let event: Event = serde_json::from_str(&raw_event)?;
            events.push(event);
        }

        Ok(events)
    }

    /// Count total events in the database
    pub fn count_events(&self) -> Result<i64> {
        let conn = self.pool.metadata()?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;

        Ok(count)
    }

    /// Delete an event by ID
    pub fn delete_event(&self, event_id: &str) -> Result<bool> {
        let conn = self.pool.writer()?;

        let rows_affected = conn.execute("DELETE FROM events WHERE id = ?1", params![event_id])?;

        Ok(rows_affected > 0)
    }

    /// Query events matching a filter
    pub fn query_events(&self, filter: &crate::subscription::Filter) -> Result<Vec<Event>> {
        let conn = self.pool.reader()?;

        // Build the SQL query based on filter criteria
        let mut sql = String::from("SELECT raw_event FROM events WHERE 1=1");
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // Filter by IDs (exact match for full IDs, prefix matching for partial)
        if let Some(ref ids) = filter.ids
            && !ids.is_empty()
        {
            let placeholders = ids
                .iter()
                .map(|id| {
                    // Use exact match for full 64-char hex IDs, prefix match for partial
                    if id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                        "id = ?"
                    } else {
                        "id LIKE ?"
                    }
                })
                .collect::<Vec<_>>()
                .join(" OR ");
            sql.push_str(&format!(" AND ({})", placeholders));
            for id in ids {
                if id.len() == 64 && id.chars().all(|c| c.is_ascii_hexdigit()) {
                    // Exact match
                    params_vec.push(Box::new(id.to_string()));
                } else {
                    // Prefix match
                    params_vec.push(Box::new(format!("{}%", id)));
                }
            }
        }

        // Filter by authors (using prefix matching)
        if let Some(ref authors) = filter.authors
            && !authors.is_empty()
        {
            let placeholders = authors
                .iter()
                .map(|_| "pubkey LIKE ?")
                .collect::<Vec<_>>()
                .join(" OR ");
            sql.push_str(&format!(" AND ({})", placeholders));
            for author in authors {
                params_vec.push(Box::new(format!("{}%", author)));
            }
        }

        // Filter by kinds
        if let Some(ref kinds) = filter.kinds
            && !kinds.is_empty()
        {
            let placeholders = kinds.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            sql.push_str(&format!(" AND kind IN ({})", placeholders));
            for kind in kinds {
                params_vec.push(Box::new(*kind));
            }
        }

        // Filter by since timestamp
        if let Some(since) = filter.since {
            sql.push_str(" AND created_at >= ?");
            params_vec.push(Box::new(since as i64));
        }

        // Filter by until timestamp
        if let Some(until) = filter.until {
            sql.push_str(" AND created_at <= ?");
            params_vec.push(Box::new(until as i64));
        }

        // Order by created_at descending
        sql.push_str(" ORDER BY created_at DESC");

        // Apply limit
        let limit = filter.limit.unwrap_or(500).min(5000);
        sql.push_str(" LIMIT ?");
        params_vec.push(Box::new(limit as i64));

        // Execute query
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let raw_event: String = row.get(0)?;
            Ok(raw_event)
        })?;

        let mut events = Vec::new();
        for row in rows {
            let raw_event = row?;
            let event: Event = serde_json::from_str(&raw_event)?;

            // Additional filtering for tag-based queries (not easily done in SQL)
            if filter.matches(&event) {
                events.push(event);
            }
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::{Event, EventTemplate, finalize_event, generate_secret_key};

    fn create_test_event(content: &str) -> Event {
        let secret_key = generate_secret_key();
        let template = EventTemplate {
            kind: 1,
            tags: vec![],
            content: content.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        finalize_event(&template, &secret_key).unwrap()
    }

    fn create_test_db() -> (Database, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let config = DatabaseConfig {
            path: db_path,
            ..Default::default()
        };
        let db = Database::new(config).unwrap();
        (db, temp_dir)
    }

    #[test]
    fn test_database_creation() {
        let (db, _temp) = create_test_db();
        assert_eq!(db.count_events().unwrap(), 0);
    }

    #[test]
    fn test_store_and_retrieve_event() {
        let (db, _temp) = create_test_db();

        let event = create_test_event("Hello, Nostr!");
        let event_id = event.id.clone();

        db.store_event(&event).unwrap();

        let retrieved = db.get_event(&event_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content, "Hello, Nostr!");
    }

    #[test]
    fn test_count_events() {
        let (db, _temp) = create_test_db();

        assert_eq!(db.count_events().unwrap(), 0);

        db.store_event(&create_test_event("Event 1")).unwrap();
        db.store_event(&create_test_event("Event 2")).unwrap();

        assert_eq!(db.count_events().unwrap(), 2);
    }

    #[test]
    fn test_get_events_by_pubkey() {
        let (db, _temp) = create_test_db();

        let event = create_test_event("Test event");
        let pubkey = event.pubkey.clone();

        db.store_event(&event).unwrap();

        let events = db.get_events_by_pubkey(&pubkey, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].content, "Test event");
    }

    #[test]
    fn test_delete_event() {
        let (db, _temp) = create_test_db();

        let event = create_test_event("To be deleted");
        let event_id = event.id.clone();

        db.store_event(&event).unwrap();
        assert!(db.get_event(&event_id).unwrap().is_some());

        let deleted = db.delete_event(&event_id).unwrap();
        assert!(deleted);
        assert!(db.get_event(&event_id).unwrap().is_none());
    }
}
