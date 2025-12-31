//! Storage abstractions and implementations.

use crate::agent::AgentState;
use crate::error::{AgentError, Result, StorageError, StorageResult};
use crate::types::AgentId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[cfg(feature = "cloudflare")]
use worker::sql::{SqlStorage, SqlStorageValue};

/// Storage operation for transactions.
#[derive(Debug, Clone)]
pub enum StorageOp {
    /// Set key-value item.
    Set {
        /// Key name.
        key: String,
        /// Value bytes.
        value: Vec<u8>,
    },
    /// Delete key-value item.
    Delete {
        /// Key name.
        key: String,
    },
    /// Replace agent state blob.
    SetState {
        /// State bytes.
        state: Vec<u8>,
    },
    /// Delete agent state.
    DeleteState,
}

/// Abstraction over agent state storage.
#[async_trait]
pub trait AgentStorage: Send + Sync {
    /// Load raw state bytes.
    async fn load_state(&self, agent_id: &AgentId) -> StorageResult<Option<Vec<u8>>>;

    /// Save raw state bytes.
    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> StorageResult<()>;

    /// Delete agent state.
    async fn delete_state(&self, agent_id: &AgentId) -> StorageResult<()>;

    /// Get key-value item.
    async fn get(&self, agent_id: &AgentId, key: &str) -> StorageResult<Option<Vec<u8>>>;

    /// Set key-value item.
    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> StorageResult<()>;

    /// Delete key-value item.
    async fn delete(&self, agent_id: &AgentId, key: &str) -> StorageResult<()>;

    /// List keys with prefix.
    async fn list(&self, agent_id: &AgentId, prefix: &str) -> StorageResult<Vec<String>>;

    /// Execute transactional operations.
    async fn transaction(&self, agent_id: &AgentId, ops: Vec<StorageOp>) -> StorageResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredState {
    version: u32,
    data: Vec<u8>,
}

impl StoredState {
    pub fn encode<S: AgentState>(state: &S) -> Result<Vec<u8>> {
        let data = serde_json::to_vec(state)?;
        let stored = StoredState {
            version: S::version(),
            data,
        };
        Ok(serde_json::to_vec(&stored)?)
    }

    pub fn decode<S: AgentState>(bytes: &[u8]) -> Result<S> {
        let stored: StoredState = serde_json::from_slice(bytes)?;
        if stored.version == S::version() {
            Ok(serde_json::from_slice(&stored.data)?)
        } else {
            S::migrate(stored.version, &stored.data)
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AgentRecord {
    state: Option<Vec<u8>>,
    kv: HashMap<String, Vec<u8>>,
}

/// In-memory storage for tests.
#[derive(Clone, Default)]
pub struct InMemoryStorage {
    inner: Arc<Mutex<HashMap<AgentId, AgentRecord>>>,
}

impl InMemoryStorage {
    /// Create an empty in-memory storage.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AgentStorage for InMemoryStorage {
    async fn load_state(&self, agent_id: &AgentId) -> StorageResult<Option<Vec<u8>>> {
        let map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        Ok(map.get(agent_id).and_then(|record| record.state.clone()))
    }

    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> StorageResult<()> {
        let mut map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let record = map.entry(agent_id.clone()).or_default();
        record.state = Some(state.to_vec());
        Ok(())
    }

    async fn delete_state(&self, agent_id: &AgentId) -> StorageResult<()> {
        let mut map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        if let Some(record) = map.get_mut(agent_id) {
            record.state = None;
        }
        Ok(())
    }

    async fn get(&self, agent_id: &AgentId, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        Ok(map
            .get(agent_id)
            .and_then(|record| record.kv.get(key).cloned()))
    }

    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> StorageResult<()> {
        let mut map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let record = map.entry(agent_id.clone()).or_default();
        record.kv.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, agent_id: &AgentId, key: &str) -> StorageResult<()> {
        let mut map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        if let Some(record) = map.get_mut(agent_id) {
            record.kv.remove(key);
        }
        Ok(())
    }

    async fn list(&self, agent_id: &AgentId, prefix: &str) -> StorageResult<Vec<String>> {
        let map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let mut keys = Vec::new();
        if let Some(record) = map.get(agent_id) {
            for key in record.kv.keys() {
                if key.starts_with(prefix) {
                    keys.push(key.clone());
                }
            }
        }
        Ok(keys)
    }

    async fn transaction(&self, agent_id: &AgentId, ops: Vec<StorageOp>) -> StorageResult<()> {
        let mut map = self.inner.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let record = map.entry(agent_id.clone()).or_default();
        let mut new_record = record.clone();

        for op in ops {
            match op {
                StorageOp::Set { key, value } => {
                    new_record.kv.insert(key, value);
                }
                StorageOp::Delete { key } => {
                    new_record.kv.remove(&key);
                }
                StorageOp::SetState { state } => {
                    new_record.state = Some(state);
                }
                StorageOp::DeleteState => {
                    new_record.state = None;
                }
            }
        }

        *record = new_record;
        Ok(())
    }
}

/// SQLite-backed storage for local runtime.
#[cfg(feature = "local")]
#[derive(Clone)]
pub struct SqliteStorage {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

#[cfg(feature = "local")]
impl SqliteStorage {
    /// Open or create a SQLite storage at the given path.
    pub fn new(path: impl AsRef<std::path::Path>) -> StorageResult<Self> {
        let conn = rusqlite::Connection::open(path)?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        storage.init()?;
        Ok(storage)
    }

    fn init(&self) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agent_state (
                agent_id TEXT PRIMARY KEY,
                state BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_kv (
                agent_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                PRIMARY KEY (agent_id, key)
            );
            ",
        )?;
        Ok(())
    }
}

#[cfg(feature = "local")]
#[async_trait]
impl AgentStorage for SqliteStorage {
    async fn load_state(&self, agent_id: &AgentId) -> StorageResult<Option<Vec<u8>>> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let mut stmt = conn.prepare("SELECT state FROM agent_state WHERE agent_id = ?1")?;
        let mut rows = stmt.query([agent_id.as_str()])?;
        if let Some(row) = rows.next()? {
            let data: Vec<u8> = row.get(0)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        conn.execute(
            "INSERT OR REPLACE INTO agent_state (agent_id, state) VALUES (?1, ?2)",
            rusqlite::params![agent_id.as_str(), state],
        )?;
        Ok(())
    }

    async fn delete_state(&self, agent_id: &AgentId) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        conn.execute(
            "DELETE FROM agent_state WHERE agent_id = ?1",
            rusqlite::params![agent_id.as_str()],
        )?;
        Ok(())
    }

    async fn get(&self, agent_id: &AgentId, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let mut stmt = conn.prepare(
            "SELECT value FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
        )?;
        let mut rows = stmt.query(rusqlite::params![agent_id.as_str(), key])?;
        if let Some(row) = rows.next()? {
            let data: Vec<u8> = row.get(0)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        conn.execute(
            "INSERT OR REPLACE INTO agent_kv (agent_id, key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![agent_id.as_str(), key, value],
        )?;
        Ok(())
    }

    async fn delete(&self, agent_id: &AgentId, key: &str) -> StorageResult<()> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        conn.execute(
            "DELETE FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
            rusqlite::params![agent_id.as_str(), key],
        )?;
        Ok(())
    }

    async fn list(&self, agent_id: &AgentId, prefix: &str) -> StorageResult<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let pattern = format!("{}%", prefix);
        let mut stmt = conn.prepare(
            "SELECT key FROM agent_kv WHERE agent_id = ?1 AND key LIKE ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![agent_id.as_str(), pattern], |row| {
            row.get(0)
        })?;
        let mut keys = Vec::new();
        for row in rows {
            keys.push(row?);
        }
        Ok(keys)
    }

    async fn transaction(&self, agent_id: &AgentId, ops: Vec<StorageOp>) -> StorageResult<()> {
        let mut conn =
            self.conn.lock().map_err(|_| StorageError::Other("lock poisoned".into()))?;
        let tx = conn.transaction()?;

        for op in ops {
            match op {
                StorageOp::Set { key, value } => {
                    tx.execute(
                        "INSERT OR REPLACE INTO agent_kv (agent_id, key, value) VALUES (?1, ?2, ?3)",
                        rusqlite::params![agent_id.as_str(), key, value],
                    )?;
                }
                StorageOp::Delete { key } => {
                    tx.execute(
                        "DELETE FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
                        rusqlite::params![agent_id.as_str(), key],
                    )?;
                }
                StorageOp::SetState { state } => {
                    tx.execute(
                        "INSERT OR REPLACE INTO agent_state (agent_id, state) VALUES (?1, ?2)",
                        rusqlite::params![agent_id.as_str(), state],
                    )?;
                }
                StorageOp::DeleteState => {
                    tx.execute(
                        "DELETE FROM agent_state WHERE agent_id = ?1",
                        rusqlite::params![agent_id.as_str()],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }
}

impl From<AgentError> for StorageError {
    fn from(err: AgentError) -> Self {
        StorageError::Other(err.to_string())
    }
}

/// Durable Object SQLite-backed storage for Cloudflare runtime.
#[cfg(feature = "cloudflare")]
#[derive(Clone)]
pub struct CloudflareStorage {
    sql: SqlStorage,
}

#[cfg(feature = "cloudflare")]
impl CloudflareStorage {
    /// Create a storage wrapper around a DO SQL handle.
    pub fn new(sql: SqlStorage) -> StorageResult<Self> {
        let storage = Self { sql };
        storage.init()?;
        Ok(storage)
    }

    fn init(&self) -> StorageResult<()> {
        self.sql.exec(
            "CREATE TABLE IF NOT EXISTS agent_state (
                agent_id TEXT PRIMARY KEY,
                state BLOB NOT NULL
            );",
            None,
        )?;
        self.sql.exec(
            "CREATE TABLE IF NOT EXISTS agent_kv (
                agent_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value BLOB NOT NULL,
                PRIMARY KEY (agent_id, key)
            );",
            None,
        )?;
        Ok(())
    }

    fn exec(
        &self,
        query: &str,
        bindings: Option<Vec<SqlStorageValue>>,
    ) -> StorageResult<worker::sql::SqlCursor> {
        self.sql.exec(query, bindings).map_err(StorageError::from)
    }
}

#[cfg(feature = "cloudflare")]
#[derive(Deserialize)]
struct ValueRow {
    value: Vec<u8>,
}

#[cfg(feature = "cloudflare")]
#[derive(Deserialize)]
struct StateRow {
    state: Vec<u8>,
}

#[cfg(feature = "cloudflare")]
#[derive(Deserialize)]
struct KeyRow {
    key: String,
}

#[cfg(feature = "cloudflare")]
#[async_trait]
impl AgentStorage for CloudflareStorage {
    async fn load_state(&self, agent_id: &AgentId) -> StorageResult<Option<Vec<u8>>> {
        let cursor = self.exec(
            "SELECT state FROM agent_state WHERE agent_id = ?1",
            Some(vec![SqlStorageValue::from(agent_id.as_str())]),
        )?;
        let rows: Vec<StateRow> = cursor.to_array().map_err(StorageError::from)?;
        Ok(rows.into_iter().next().map(|row| row.state))
    }

    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> StorageResult<()> {
        self.exec(
            "INSERT OR REPLACE INTO agent_state (agent_id, state) VALUES (?1, ?2)",
            Some(vec![
                SqlStorageValue::from(agent_id.as_str()),
                SqlStorageValue::from(state.to_vec()),
            ]),
        )?;
        Ok(())
    }

    async fn delete_state(&self, agent_id: &AgentId) -> StorageResult<()> {
        self.exec(
            "DELETE FROM agent_state WHERE agent_id = ?1",
            Some(vec![SqlStorageValue::from(agent_id.as_str())]),
        )?;
        Ok(())
    }

    async fn get(&self, agent_id: &AgentId, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let cursor = self.exec(
            "SELECT value FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
            Some(vec![
                SqlStorageValue::from(agent_id.as_str()),
                SqlStorageValue::from(key),
            ]),
        )?;
        let rows: Vec<ValueRow> = cursor.to_array().map_err(StorageError::from)?;
        Ok(rows.into_iter().next().map(|row| row.value))
    }

    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> StorageResult<()> {
        self.exec(
            "INSERT OR REPLACE INTO agent_kv (agent_id, key, value) VALUES (?1, ?2, ?3)",
            Some(vec![
                SqlStorageValue::from(agent_id.as_str()),
                SqlStorageValue::from(key),
                SqlStorageValue::from(value.to_vec()),
            ]),
        )?;
        Ok(())
    }

    async fn delete(&self, agent_id: &AgentId, key: &str) -> StorageResult<()> {
        self.exec(
            "DELETE FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
            Some(vec![
                SqlStorageValue::from(agent_id.as_str()),
                SqlStorageValue::from(key),
            ]),
        )?;
        Ok(())
    }

    async fn list(&self, agent_id: &AgentId, prefix: &str) -> StorageResult<Vec<String>> {
        let pattern = format!("{}%", prefix);
        let cursor = self.exec(
            "SELECT key FROM agent_kv WHERE agent_id = ?1 AND key LIKE ?2 ORDER BY key",
            Some(vec![
                SqlStorageValue::from(agent_id.as_str()),
                SqlStorageValue::from(pattern.as_str()),
            ]),
        )?;
        let rows: Vec<KeyRow> = cursor.to_array().map_err(StorageError::from)?;
        Ok(rows.into_iter().map(|row| row.key).collect())
    }

    async fn transaction(&self, agent_id: &AgentId, ops: Vec<StorageOp>) -> StorageResult<()> {
        self.exec("BEGIN", None)?;
        let mut result = Ok(());

        for op in ops {
            let op_result = match op {
                StorageOp::Set { key, value } => self.exec(
                    "INSERT OR REPLACE INTO agent_kv (agent_id, key, value) VALUES (?1, ?2, ?3)",
                    Some(vec![
                        SqlStorageValue::from(agent_id.as_str()),
                        SqlStorageValue::from(key.as_str()),
                        SqlStorageValue::from(value),
                    ]),
                ),
                StorageOp::Delete { key } => self.exec(
                    "DELETE FROM agent_kv WHERE agent_id = ?1 AND key = ?2",
                    Some(vec![
                        SqlStorageValue::from(agent_id.as_str()),
                        SqlStorageValue::from(key.as_str()),
                    ]),
                ),
                StorageOp::SetState { state } => self.exec(
                    "INSERT OR REPLACE INTO agent_state (agent_id, state) VALUES (?1, ?2)",
                    Some(vec![
                        SqlStorageValue::from(agent_id.as_str()),
                        SqlStorageValue::from(state),
                    ]),
                ),
                StorageOp::DeleteState => self.exec(
                    "DELETE FROM agent_state WHERE agent_id = ?1",
                    Some(vec![SqlStorageValue::from(agent_id.as_str())]),
                ),
            };

            if let Err(err) = op_result {
                result = Err(err);
                break;
            }
        }

        if result.is_ok() {
            self.exec("COMMIT", None)?;
        } else {
            let _ = self.exec("ROLLBACK", None);
        }

        result.map(|_| ())
    }
}

#[cfg(feature = "cloudflare")]
impl From<worker::Error> for StorageError {
    fn from(err: worker::Error) -> Self {
        StorageError::Other(err.to_string())
    }
}
