//! Persistent storage for Coder sessions and messages.
//!
//! This crate provides SQLite-based storage for:
//! - Sessions (conversations)
//! - Messages (within sessions)
//! - Key-value metadata
//!
//! # Example
//!
//! ```no_run
//! use coder_storage::Storage;
//!
//! let storage = Storage::open("~/.coder/data.db").unwrap();
//!
//! // Store a value
//! storage.set(&["sessions", "abc123", "title"], &"My Session").unwrap();
//!
//! // Retrieve a value
//! let title: Option<String> = storage.get(&["sessions", "abc123", "title"]).unwrap();
//! ```

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

/// Storage errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite error.
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Key not found.
    #[error("Key not found: {0}")]
    NotFound(String),

    /// Invalid key.
    #[error("Invalid key: {0}")]
    InvalidKey(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// SQLite-based storage for sessions and messages.
#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    /// Open or create a storage database at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.init_schema()?;
        Ok(storage)
    }

    /// Create an in-memory storage (for testing).
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.init_schema()?;
        Ok(storage)
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            r#"
            -- Key-value store for arbitrary data
            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            -- Sessions table
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                directory TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                archived_at TEXT,
                metadata TEXT
            );

            -- Messages table
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                metadata TEXT,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
            CREATE INDEX IF NOT EXISTS idx_sessions_directory ON sessions(directory);
            CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
            "#,
        )?;

        Ok(())
    }

    // ========================================================================
    // Key-Value Operations
    // ========================================================================

    /// Get a value by key path.
    ///
    /// Key path is joined with `/` separator.
    pub fn get<T: DeserializeOwned>(&self, key: &[&str]) -> Result<Option<T>> {
        let key_str = key.join("/");
        let conn = self.conn.lock().unwrap();

        let result: Option<String> = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key_str], |row| {
                row.get(0)
            })
            .optional()?;

        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Set a value by key path.
    pub fn set<T: Serialize>(&self, key: &[&str], value: &T) -> Result<()> {
        let key_str = key.join("/");
        let json = serde_json::to_string(value)?;
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO kv (key, value, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3
            "#,
            params![key_str, json, now],
        )?;

        Ok(())
    }

    /// Update a value by key path with a function.
    pub fn update<T: DeserializeOwned + Serialize>(
        &self,
        key: &[&str],
        f: impl FnOnce(&mut T),
    ) -> Result<T> {
        let mut value: T = self.get(key)?.ok_or_else(|| {
            StorageError::NotFound(key.join("/"))
        })?;

        f(&mut value);
        self.set(key, &value)?;
        Ok(value)
    }

    /// Delete a value by key path.
    pub fn delete(&self, key: &[&str]) -> Result<bool> {
        let key_str = key.join("/");
        let conn = self.conn.lock().unwrap();

        let count = conn.execute("DELETE FROM kv WHERE key = ?1", params![key_str])?;
        Ok(count > 0)
    }

    /// List keys with a given prefix.
    pub fn list(&self, prefix: &[&str]) -> Result<Vec<String>> {
        let prefix_str = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix.join("/"))
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key FROM kv WHERE key LIKE ?1")?;

        let rows = stmt.query_map(params![format!("{}%", prefix_str)], |row| row.get(0))?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row?);
        }

        Ok(keys)
    }

    // ========================================================================
    // Session Operations
    // ========================================================================

    /// Create a new session.
    pub fn create_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let metadata = serde_json::to_string(&session.metadata)?;

        conn.execute(
            r#"
            INSERT INTO sessions (id, project_id, directory, title, created_at, updated_at, archived_at, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                session.id.to_string(),
                session.project_id.as_ref().map(|id| id.to_string()),
                session.directory,
                session.title,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                session.archived_at.map(|t| t.to_rfc3339()),
                metadata,
            ],
        )?;

        Ok(())
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &Uuid) -> Result<Option<Session>> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT id, project_id, directory, title, created_at, updated_at, archived_at, metadata FROM sessions WHERE id = ?1",
            params![id.to_string()],
            |row| {
                Ok(Session {
                    id: row.get::<_, String>(0)?.parse().unwrap(),
                    project_id: row.get::<_, Option<String>>(1)?.map(|s| s.parse().unwrap()),
                    directory: row.get(2)?,
                    title: row.get(3)?,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).unwrap().into(),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).unwrap().into(),
                    archived_at: row.get::<_, Option<String>>(6)?.map(|s| DateTime::parse_from_rfc3339(&s).unwrap().into()),
                    metadata: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
                })
            },
        ).optional().map_err(Into::into)
    }

    /// Update a session.
    pub fn update_session(&self, session: &Session) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let metadata = serde_json::to_string(&session.metadata)?;

        conn.execute(
            r#"
            UPDATE sessions
            SET project_id = ?2, directory = ?3, title = ?4, updated_at = ?5, archived_at = ?6, metadata = ?7
            WHERE id = ?1
            "#,
            params![
                session.id.to_string(),
                session.project_id.as_ref().map(|id| id.to_string()),
                session.directory,
                session.title,
                session.updated_at.to_rfc3339(),
                session.archived_at.map(|t| t.to_rfc3339()),
                metadata,
            ],
        )?;

        Ok(())
    }

    /// List sessions for a directory.
    pub fn list_sessions(&self, directory: &str) -> Result<Vec<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_id, directory, title, created_at, updated_at, archived_at, metadata FROM sessions WHERE directory = ?1 ORDER BY updated_at DESC"
        )?;

        let rows = stmt.query_map(params![directory], |row| {
            Ok(Session {
                id: row.get::<_, String>(0)?.parse().unwrap(),
                project_id: row.get::<_, Option<String>>(1)?.map(|s| s.parse().unwrap()),
                directory: row.get(2)?,
                title: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).unwrap().into(),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).unwrap().into(),
                archived_at: row.get::<_, Option<String>>(6)?.map(|s| DateTime::parse_from_rfc3339(&s).unwrap().into()),
                metadata: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }

        Ok(sessions)
    }

    /// Delete a session and its messages.
    pub fn delete_session(&self, id: &Uuid) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        // Messages are deleted via CASCADE
        let count = conn.execute("DELETE FROM sessions WHERE id = ?1", params![id.to_string()])?;
        Ok(count > 0)
    }

    // ========================================================================
    // Message Operations
    // ========================================================================

    /// Add a message to a session.
    pub fn add_message(&self, message: &StoredMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let content = serde_json::to_string(&message.content)?;
        let metadata = serde_json::to_string(&message.metadata)?;

        conn.execute(
            r#"
            INSERT INTO messages (id, session_id, role, content, created_at, updated_at, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                message.id.to_string(),
                message.session_id.to_string(),
                message.role,
                content,
                message.created_at.to_rfc3339(),
                message.updated_at.to_rfc3339(),
                metadata,
            ],
        )?;

        // Update session's updated_at
        conn.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
            params![message.session_id.to_string(), Utc::now().to_rfc3339()],
        )?;

        Ok(())
    }

    /// Get messages for a session.
    pub fn get_messages(&self, session_id: &Uuid) -> Result<Vec<StoredMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at, updated_at, metadata FROM messages WHERE session_id = ?1 ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map(params![session_id.to_string()], |row| {
            Ok(StoredMessage {
                id: row.get::<_, String>(0)?.parse().unwrap(),
                session_id: row.get::<_, String>(1)?.parse().unwrap(),
                role: row.get(2)?,
                content: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?).unwrap().into(),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?).unwrap().into(),
                metadata: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }

        Ok(messages)
    }

    /// Update a message.
    pub fn update_message(&self, message: &StoredMessage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let content = serde_json::to_string(&message.content)?;
        let metadata = serde_json::to_string(&message.metadata)?;

        conn.execute(
            r#"
            UPDATE messages
            SET role = ?2, content = ?3, updated_at = ?4, metadata = ?5
            WHERE id = ?1
            "#,
            params![
                message.id.to_string(),
                message.role,
                content,
                message.updated_at.to_rfc3339(),
                metadata,
            ],
        )?;

        Ok(())
    }

    /// Delete a message.
    pub fn delete_message(&self, id: &Uuid) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM messages WHERE id = ?1", params![id.to_string()])?;
        Ok(count > 0)
    }
}

// ============================================================================
// Data Types
// ============================================================================

/// A stored session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    /// Session ID.
    pub id: Uuid,
    /// Associated project ID.
    pub project_id: Option<Uuid>,
    /// Working directory.
    pub directory: String,
    /// Session title.
    pub title: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Archive timestamp.
    pub archived_at: Option<DateTime<Utc>>,
    /// Additional metadata.
    pub metadata: serde_json::Value,
}

impl Session {
    /// Create a new session.
    pub fn new(directory: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            project_id: None,
            directory: directory.into(),
            title: None,
            created_at: now,
            updated_at: now,
            archived_at: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the project ID.
    pub fn with_project(mut self, project_id: Uuid) -> Self {
        self.project_id = Some(project_id);
        self
    }
}

/// A stored message.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredMessage {
    /// Message ID.
    pub id: Uuid,
    /// Session this message belongs to.
    pub session_id: Uuid,
    /// Message role (user, assistant, system).
    pub role: String,
    /// Message content (JSON array of parts).
    pub content: serde_json::Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Additional metadata (tokens, cost, etc.).
    pub metadata: serde_json::Value,
}

impl StoredMessage {
    /// Create a new user message.
    pub fn user(session_id: Uuid, content: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            session_id,
            role: "user".to_string(),
            content: serde_json::json!([{ "type": "text", "text": content.into() }]),
            created_at: now,
            updated_at: now,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(session_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            session_id,
            role: "assistant".to_string(),
            content: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            metadata: serde_json::Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_operations() {
        let storage = Storage::in_memory().unwrap();

        // Set and get
        storage.set(&["foo", "bar"], &"hello").unwrap();
        let value: Option<String> = storage.get(&["foo", "bar"]).unwrap();
        assert_eq!(value, Some("hello".to_string()));

        // Update
        storage.set(&["foo", "bar"], &"world").unwrap();
        let value: Option<String> = storage.get(&["foo", "bar"]).unwrap();
        assert_eq!(value, Some("world".to_string()));

        // Delete
        let deleted = storage.delete(&["foo", "bar"]).unwrap();
        assert!(deleted);
        let value: Option<String> = storage.get(&["foo", "bar"]).unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_session_operations() {
        let storage = Storage::in_memory().unwrap();

        // Create session
        let session = Session::new("/home/user/project").with_title("Test Session");
        storage.create_session(&session).unwrap();

        // Get session
        let retrieved = storage.get_session(&session.id).unwrap().unwrap();
        assert_eq!(retrieved.id, session.id);
        assert_eq!(retrieved.title, Some("Test Session".to_string()));

        // List sessions
        let sessions = storage.list_sessions("/home/user/project").unwrap();
        assert_eq!(sessions.len(), 1);

        // Delete session
        let deleted = storage.delete_session(&session.id).unwrap();
        assert!(deleted);
        let retrieved = storage.get_session(&session.id).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_message_operations() {
        let storage = Storage::in_memory().unwrap();

        // Create session first
        let session = Session::new("/home/user/project");
        storage.create_session(&session).unwrap();

        // Add message
        let msg = StoredMessage::user(session.id, "Hello, world!");
        storage.add_message(&msg).unwrap();

        // Get messages
        let messages = storage.get_messages(&session.id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");

        // Delete message
        let deleted = storage.delete_message(&msg.id).unwrap();
        assert!(deleted);
        let messages = storage.get_messages(&session.id).unwrap();
        assert_eq!(messages.len(), 0);
    }
}
