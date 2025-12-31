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

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use futures::channel::oneshot;
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use js_sys::{Array, Promise, Uint8Array};
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use wasm_bindgen_futures::{spawn_local, JsFuture};
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
use web_sys::{
    DomException, IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransaction,
    IdbTransactionMode, WorkerGlobalScope,
};

/// IndexedDB-backed storage for browser runtime.
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
#[derive(Clone)]
pub struct IndexedDbStorage {
    db_name: String,
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
impl IndexedDbStorage {
    /// Create a new IndexedDB storage with the provided database name.
    pub fn new(db_name: impl Into<String>) -> Self {
        Self {
            db_name: db_name.into(),
        }
    }

    fn kv_key(agent_id: &AgentId, key: &str) -> String {
        format!("{}::{}", agent_id.as_str(), key)
    }

    fn kv_prefix(agent_id: &AgentId) -> String {
        format!("{}::", agent_id.as_str())
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
#[async_trait]
impl AgentStorage for IndexedDbStorage {
    async fn load_state(&self, agent_id: &AgentId) -> StorageResult<Option<Vec<u8>>> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readonly)?;
            let store = tx
                .object_store(STATE_STORE)
                .map_err(js_error)?;
            let result = store_get(&store, agent_id.as_str()).await?;
            await_transaction(tx).await?;
            Ok(result)
        })
        .await
    }

    async fn save_state(&self, agent_id: &AgentId, state: &[u8]) -> StorageResult<()> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        let state = state.to_vec();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readwrite)?;
            let store = tx
                .object_store(STATE_STORE)
                .map_err(js_error)?;
            store_put(&store, agent_id.as_str(), &state).await?;
            await_transaction(tx).await?;
            Ok(())
        })
        .await
    }

    async fn delete_state(&self, agent_id: &AgentId) -> StorageResult<()> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readwrite)?;
            let store = tx
                .object_store(STATE_STORE)
                .map_err(js_error)?;
            store_delete(&store, agent_id.as_str()).await?;
            await_transaction(tx).await?;
            Ok(())
        })
        .await
    }

    async fn get(&self, agent_id: &AgentId, key: &str) -> StorageResult<Option<Vec<u8>>> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        let key = key.to_string();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readonly)?;
            let store = tx
                .object_store(KV_STORE)
                .map_err(js_error)?;
            let storage_key = IndexedDbStorage::kv_key(&agent_id, &key);
            let result = store_get(&store, &storage_key).await?;
            await_transaction(tx).await?;
            Ok(result)
        })
        .await
    }

    async fn set(&self, agent_id: &AgentId, key: &str, value: &[u8]) -> StorageResult<()> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        let key = key.to_string();
        let value = value.to_vec();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readwrite)?;
            let store = tx
                .object_store(KV_STORE)
                .map_err(js_error)?;
            let storage_key = IndexedDbStorage::kv_key(&agent_id, &key);
            store_put(&store, &storage_key, &value).await?;
            await_transaction(tx).await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, agent_id: &AgentId, key: &str) -> StorageResult<()> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        let key = key.to_string();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readwrite)?;
            let store = tx
                .object_store(KV_STORE)
                .map_err(js_error)?;
            let storage_key = IndexedDbStorage::kv_key(&agent_id, &key);
            store_delete(&store, &storage_key).await?;
            await_transaction(tx).await?;
            Ok(())
        })
        .await
    }

    async fn list(&self, agent_id: &AgentId, prefix: &str) -> StorageResult<Vec<String>> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        let prefix = prefix.to_string();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readonly)?;
            let store = tx
                .object_store(KV_STORE)
                .map_err(js_error)?;
            let keys = store_list_keys(&store).await?;
            await_transaction(tx).await?;
            let prefix_root = IndexedDbStorage::kv_prefix(&agent_id);
            let prefix_match = format!("{}{}", prefix_root, prefix);
            let mut results = Vec::new();
            for key in keys {
                if key.starts_with(&prefix_match) {
                    let trimmed = key.trim_start_matches(&prefix_root);
                    results.push(trimmed.to_string());
                }
            }
            Ok(results)
        })
        .await
    }

    async fn transaction(&self, agent_id: &AgentId, ops: Vec<StorageOp>) -> StorageResult<()> {
        let db_name = self.db_name.clone();
        let agent_id = agent_id.clone();
        run_indexeddb_task(async move {
            let db = open_db(&db_name).await?;
            let tx = open_transaction(&db, IdbTransactionMode::Readwrite)?;
            let state_store = tx
                .object_store(STATE_STORE)
                .map_err(js_error)?;
            let kv_store = tx
                .object_store(KV_STORE)
                .map_err(js_error)?;

            for op in ops {
                match op {
                    StorageOp::Set { key, value } => {
                        let storage_key = IndexedDbStorage::kv_key(&agent_id, &key);
                        store_put(&kv_store, &storage_key, &value).await?;
                    }
                    StorageOp::Delete { key } => {
                        let storage_key = IndexedDbStorage::kv_key(&agent_id, &key);
                        store_delete(&kv_store, &storage_key).await?;
                    }
                    StorageOp::SetState { state } => {
                        store_put(&state_store, agent_id.as_str(), &state).await?;
                    }
                    StorageOp::DeleteState => {
                        store_delete(&state_store, agent_id.as_str()).await?;
                    }
                }
            }

            await_transaction(tx).await?;
            Ok(())
        })
        .await
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
const DB_VERSION: u32 = 1;
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
const STATE_STORE: &str = "state";
#[cfg(all(target_arch = "wasm32", feature = "browser"))]
const KV_STORE: &str = "kv";

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn run_indexeddb_task<T>(
    task: impl std::future::Future<Output = StorageResult<T>> + 'static,
) -> impl std::future::Future<Output = StorageResult<T>> + Send
where
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    spawn_local(async move {
        let _ = tx.send(task.await);
    });
    async move {
        rx.await
            .map_err(|_| StorageError::Other("indexeddb task canceled".to_string()))?
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn open_db(db_name: &str) -> StorageResult<IdbDatabase> {
    let factory = idb_factory()?;
    let open_request = factory
        .open_with_u32(db_name, DB_VERSION)
        .map_err(js_error)?;
    let upgrade_request = open_request.clone();
    let on_upgrade = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let db = upgrade_request
            .result()
            .expect("upgrade result")
            .dyn_into::<IdbDatabase>()
            .expect("db cast");
        let names = db.object_store_names();
        if !names.contains(STATE_STORE) {
            let _ = db.create_object_store(STATE_STORE);
        }
        if !names.contains(KV_STORE) {
            let _ = db.create_object_store(KV_STORE);
        }
    }) as Box<dyn FnMut(_)>);
    open_request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    on_upgrade.forget();

    let request: IdbRequest = open_request
        .dyn_into()
        .map_err(|err| js_error(err.into()))?;
    let value = request_result(request).await?;
    value
        .dyn_into::<IdbDatabase>()
        .map_err(|err| StorageError::Other(js_error_message(err)))
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn open_transaction(db: &IdbDatabase, mode: IdbTransactionMode) -> StorageResult<IdbTransaction> {
    let stores = Array::new();
    stores.push(&JsValue::from_str(STATE_STORE));
    stores.push(&JsValue::from_str(KV_STORE));
    db.transaction_with_str_sequence_and_mode(&stores, mode)
        .map_err(js_error)
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn store_get(store: &IdbObjectStore, key: &str) -> StorageResult<Option<Vec<u8>>> {
    let request = store.get(&JsValue::from_str(key)).map_err(js_error)?;
    let value = request_result(request).await?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    let array = Uint8Array::new(&value);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    Ok(Some(bytes))
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn store_put(store: &IdbObjectStore, key: &str, value: &[u8]) -> StorageResult<()> {
    let data = Uint8Array::from(value);
    let request = store
        .put_with_key(&JsValue::from(data), &JsValue::from_str(key))
        .map_err(js_error)?;
    let _ = request_result(request).await?;
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn store_delete(store: &IdbObjectStore, key: &str) -> StorageResult<()> {
    let request = store
        .delete(&JsValue::from_str(key))
        .map_err(js_error)?;
    let _ = request_result(request).await?;
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn store_list_keys(store: &IdbObjectStore) -> StorageResult<Vec<String>> {
    let request = store.get_all_keys().map_err(js_error)?;
    let value = request_result(request).await?;
    let array = Array::from(&value);
    let mut keys = Vec::new();
    for entry in array.iter() {
        if let Some(key) = entry.as_string() {
            keys.push(key);
        }
    }
    Ok(keys)
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
async fn await_transaction(tx: IdbTransaction) -> StorageResult<()> {
    let promise = transaction_promise(&tx);
    JsFuture::from(promise)
        .await
        .map_err(|err| StorageError::Other(js_error_message(err)))?;
    Ok(())
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn request_result(request: IdbRequest) -> impl std::future::Future<Output = StorageResult<JsValue>> {
    async move {
        let promise = request_promise(&request);
        JsFuture::from(promise)
            .await
            .map_err(|err| StorageError::Other(js_error_message(err)))
    }
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn request_promise(request: &IdbRequest) -> Promise {
    let request = request.clone();
    Promise::new(&mut |resolve, reject| {
        let success_request = request.clone();
        let success = Closure::once(move |_event: web_sys::Event| {
            let result = success_request.result().unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::NULL, &result);
        });
        let error_request = request.clone();
        let error = Closure::once(move |_event: web_sys::Event| {
            let message = match error_request.error() {
                Ok(Some(err)) => err.message(),
                Ok(None) => "indexeddb request error".to_string(),
                Err(err) => js_error_message(err),
            };
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&message));
        });
        request.set_onsuccess(Some(success.as_ref().unchecked_ref()));
        request.set_onerror(Some(error.as_ref().unchecked_ref()));
        success.forget();
        error.forget();
    })
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn transaction_promise(tx: &IdbTransaction) -> Promise {
    let tx = tx.clone();
    Promise::new(&mut |resolve, reject| {
        let on_complete = Closure::once(move |_event: web_sys::Event| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let reject_error = reject.clone();
        let on_error = Closure::once(move |_event: web_sys::Event| {
            let _ = reject_error.call1(&JsValue::NULL, &JsValue::from_str("indexeddb tx error"));
        });
        let reject_abort = reject.clone();
        let on_abort = Closure::once(move |_event: web_sys::Event| {
            let _ = reject_abort.call1(&JsValue::NULL, &JsValue::from_str("indexeddb tx aborted"));
        });
        tx.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
        tx.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        tx.set_onabort(Some(on_abort.as_ref().unchecked_ref()));
        on_complete.forget();
        on_error.forget();
        on_abort.forget();
    })
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn idb_factory() -> StorageResult<IdbFactory> {
    if let Some(window) = web_sys::window() {
        return window
            .indexed_db()
            .map_err(js_error)?
            .ok_or_else(|| StorageError::Other("indexeddb unavailable".to_string()));
    }
    let global = js_sys::global();
    let scope: WorkerGlobalScope = global
        .dyn_into()
        .map_err(|err| StorageError::Other(js_error_message(err.into())))?;
    scope
        .indexed_db()
        .map_err(js_error)?
        .ok_or_else(|| StorageError::Other("indexeddb unavailable".to_string()))
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn js_error(err: JsValue) -> StorageError {
    StorageError::Other(js_error_message(err))
}

#[cfg(all(target_arch = "wasm32", feature = "browser"))]
fn js_error_message(err: JsValue) -> String {
    if let Ok(dom) = err.clone().dyn_into::<DomException>() {
        return dom.message();
    }
    if let Some(message) = err.as_string() {
        return message;
    }
    format!("{err:?}")
}
