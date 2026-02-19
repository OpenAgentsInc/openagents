//! Message queue for offline event handling
//!
//! Provides reliable event delivery with:
//! - SQLite persistence for durability
//! - Automatic retry with exponential backoff
//! - Dead letter queue for permanently failed events
//! - Integration with connection state changes

use crate::error::{ClientError, Result};
use nostr::Event;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Status of a queued message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Pending delivery
    Pending,
    /// Successfully sent
    Sent,
    /// Failed after max retries
    Failed,
}

impl MessageStatus {
    fn to_str(&self) -> &str {
        match self {
            MessageStatus::Pending => "pending",
            MessageStatus::Sent => "sent",
            MessageStatus::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "sent" => MessageStatus::Sent,
            "failed" => MessageStatus::Failed,
            _ => MessageStatus::Pending,
        }
    }
}

/// A queued message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    /// Database ID
    pub id: i64,
    /// Event ID
    pub event_id: String,
    /// Event JSON
    pub event_json: String,
    /// Target relay URL
    pub relay_url: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Last retry timestamp
    pub last_retry: Option<u64>,
    /// Message status
    pub status: MessageStatus,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Configuration for message queue
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Database file path
    pub db_path: PathBuf,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial retry delay (doubles each retry)
    pub initial_retry_delay: Duration,
    /// Enable automatic retries
    pub auto_retry: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("nostr_queue.db"),
            max_retries: 5,
            initial_retry_delay: Duration::from_secs(1),
            auto_retry: true,
        }
    }
}

/// Message queue for offline event handling
pub struct MessageQueue {
    config: QueueConfig,
    db: Arc<Mutex<rusqlite::Connection>>,
}

impl MessageQueue {
    /// Create a new message queue with default config
    pub fn new() -> Result<Self> {
        Self::with_config(QueueConfig::default())
    }

    /// Create a new message queue with custom config
    pub fn with_config(config: QueueConfig) -> Result<Self> {
        let db = rusqlite::Connection::open(&config.db_path)
            .map_err(|e| ClientError::Internal(format!("Failed to open queue database: {}", e)))?;

        // Create table if it doesn't exist
        db.execute(
            "CREATE TABLE IF NOT EXISTS message_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL,
                event_json TEXT NOT NULL,
                relay_url TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                retry_count INTEGER DEFAULT 0,
                last_retry INTEGER,
                status TEXT DEFAULT 'pending',
                error TEXT
            )",
            [],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to create queue table: {}", e)))?;

        // Create index on status for faster queries
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_status ON message_queue(status)",
            [],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to create index: {}", e)))?;

        Ok(Self {
            config,
            db: Arc::new(Mutex::new(db)),
        })
    }

    /// Enqueue an event for delivery
    pub fn enqueue(&self, event: &Event, relay_url: &str) -> Result<i64> {
        let event_json = serde_json::to_string(event).map_err(ClientError::Serialization)?;

        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let now = i64::try_from(now_millis)
            .map_err(|_| ClientError::Internal("Timestamp overflow".to_string()))?;

        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        db.execute(
            "INSERT INTO message_queue (event_id, event_json, relay_url, created_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &event.id,
                &event_json,
                relay_url,
                now,
                MessageStatus::Pending.to_str()
            ],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to enqueue message: {}", e)))?;

        Ok(db.last_insert_rowid())
    }

    /// Get the next pending message to send
    pub fn dequeue(&self) -> Result<Option<QueuedMessage>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        let mut stmt = db
            .prepare(
                "SELECT id, event_id, event_json, relay_url, created_at, retry_count, last_retry, status, error
                 FROM message_queue
                 WHERE status = 'pending'
                 ORDER BY created_at ASC
                 LIMIT 1",
            )
            .map_err(|e| ClientError::Internal(format!("Failed to prepare query: {}", e)))?;

        let result = stmt
            .query_row([], |row| {
                Ok(QueuedMessage {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    event_json: row.get(2)?,
                    relay_url: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    retry_count: row.get::<_, i64>(5)? as u32,
                    last_retry: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
                    status: MessageStatus::from_str(&row.get::<_, String>(7)?),
                    error: row.get(8)?,
                })
            })
            .optional()
            .map_err(|e| ClientError::Internal(format!("Failed to dequeue message: {}", e)))?;

        // Check if message is ready for retry (based on backoff)
        if let Some(msg) = result {
            if msg.retry_count > 0
                && let Some(last_retry) = msg.last_retry
            {
                let delay = self.calculate_backoff(msg.retry_count);
                let next_retry_millis = last_retry + delay.as_millis() as u64;
                if now < next_retry_millis {
                    // Not ready for retry yet
                    return Ok(None);
                }
            }
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }

    /// Mark a message as successfully sent
    pub fn mark_sent(&self, id: i64) -> Result<()> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        db.execute(
            "UPDATE message_queue SET status = ?1 WHERE id = ?2",
            rusqlite::params![MessageStatus::Sent.to_str(), id],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to mark message as sent: {}", e)))?;

        Ok(())
    }

    /// Mark a message as failed and optionally retry
    pub fn mark_failed(&self, id: i64, error: &str) -> Result<()> {
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let now = i64::try_from(now_millis)
            .map_err(|_| ClientError::Internal("Timestamp overflow".to_string()))?;

        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        // Get current retry count
        let retry_count: u32 = db
            .query_row(
                "SELECT retry_count FROM message_queue WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get::<_, i64>(0).map(|c| c as u32),
            )
            .map_err(|e| ClientError::Internal(format!("Failed to get retry count: {}", e)))?;

        let new_retry_count = retry_count + 1;
        let status = if new_retry_count >= self.config.max_retries {
            MessageStatus::Failed
        } else {
            MessageStatus::Pending
        };

        db.execute(
            "UPDATE message_queue
             SET retry_count = ?1, last_retry = ?2, status = ?3, error = ?4
             WHERE id = ?5",
            rusqlite::params![new_retry_count as i64, now, status.to_str(), error, id],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to mark message as failed: {}", e)))?;

        Ok(())
    }

    /// Get all pending messages
    pub fn get_pending(&self) -> Result<Vec<QueuedMessage>> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        let mut stmt = db
            .prepare(
                "SELECT id, event_id, event_json, relay_url, created_at, retry_count, last_retry, status, error
                 FROM message_queue
                 WHERE status = 'pending'
                 ORDER BY created_at ASC",
            )
            .map_err(|e| ClientError::Internal(format!("Failed to prepare query: {}", e)))?;

        let messages = stmt
            .query_map([], |row| {
                Ok(QueuedMessage {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    event_json: row.get(2)?,
                    relay_url: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    retry_count: row.get::<_, i64>(5)? as u32,
                    last_retry: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
                    status: MessageStatus::from_str(&row.get::<_, String>(7)?),
                    error: row.get(8)?,
                })
            })
            .map_err(|e| ClientError::Internal(format!("Failed to query pending messages: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ClientError::Internal(format!("Failed to collect messages: {}", e)))?;

        Ok(messages)
    }

    /// Get all failed messages (dead letter queue)
    pub fn get_failed(&self) -> Result<Vec<QueuedMessage>> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        let mut stmt = db
            .prepare(
                "SELECT id, event_id, event_json, relay_url, created_at, retry_count, last_retry, status, error
                 FROM message_queue
                 WHERE status = 'failed'
                 ORDER BY created_at DESC",
            )
            .map_err(|e| ClientError::Internal(format!("Failed to prepare query: {}", e)))?;

        let messages = stmt
            .query_map([], |row| {
                Ok(QueuedMessage {
                    id: row.get(0)?,
                    event_id: row.get(1)?,
                    event_json: row.get(2)?,
                    relay_url: row.get(3)?,
                    created_at: row.get::<_, i64>(4)? as u64,
                    retry_count: row.get::<_, i64>(5)? as u32,
                    last_retry: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
                    status: MessageStatus::from_str(&row.get::<_, String>(7)?),
                    error: row.get(8)?,
                })
            })
            .map_err(|e| ClientError::Internal(format!("Failed to query failed messages: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ClientError::Internal(format!("Failed to collect messages: {}", e)))?;

        Ok(messages)
    }

    /// Get queue size (pending messages)
    pub fn size(&self) -> Result<usize> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM message_queue WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| ClientError::Internal(format!("Failed to get queue size: {}", e)))?;

        Ok(count as usize)
    }

    /// Clear all messages from queue
    pub fn clear(&self) -> Result<()> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        db.execute("DELETE FROM message_queue", [])
            .map_err(|e| ClientError::Internal(format!("Failed to clear queue: {}", e)))?;

        Ok(())
    }

    /// Clear only sent messages (for cleanup)
    pub fn clear_sent(&self) -> Result<usize> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        let count = db
            .execute("DELETE FROM message_queue WHERE status = 'sent'", [])
            .map_err(|e| ClientError::Internal(format!("Failed to clear sent messages: {}", e)))?;

        Ok(count)
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff(&self, retry_count: u32) -> Duration {
        let multiplier = 2_u32.saturating_pow(retry_count.saturating_sub(1));
        self.config.initial_retry_delay * multiplier
    }

    /// Retry a specific message immediately (resets backoff timer)
    pub fn retry_now(&self, id: i64) -> Result<()> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        db.execute(
            "UPDATE message_queue SET last_retry = NULL WHERE id = ?1 AND status = 'pending'",
            rusqlite::params![id],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to reset retry timer: {}", e)))?;

        Ok(())
    }

    /// Retry all pending messages immediately
    pub fn retry_all(&self) -> Result<()> {
        let db = self.db.lock().map_err(|e| {
            tracing::error!("Database lock poisoned: {}", e);
            ClientError::Internal("Failed to acquire database lock (poisoned)".to_string())
        })?;

        db.execute(
            "UPDATE message_queue SET last_retry = NULL WHERE status = 'pending'",
            [],
        )
        .map_err(|e| ClientError::Internal(format!("Failed to reset all retry timers: {}", e)))?;

        Ok(())
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new().expect("Failed to create default message queue")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn create_test_event(id: &str) -> Event {
        Event {
            id: id.to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![],
            content: "test message".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    fn create_test_queue() -> MessageQueue {
        let config = QueueConfig {
            db_path: PathBuf::from(format!("test_queue_{}.db", rand::random::<u64>())),
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            auto_retry: true,
        };
        MessageQueue::with_config(config).unwrap()
    }

    #[test]
    fn test_enqueue_and_dequeue() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.example.com").unwrap();
        assert!(id > 0);

        let msg = queue.dequeue().unwrap().unwrap();
        assert_eq!(msg.event_id, "event1");
        assert_eq!(msg.relay_url, "wss://relay.example.com");
        assert_eq!(msg.status, MessageStatus::Pending);
        assert_eq!(msg.retry_count, 0);
    }

    #[test]
    fn test_mark_sent() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.example.com").unwrap();
        queue.mark_sent(id).unwrap();

        // Should not be in pending queue
        assert!(queue.dequeue().unwrap().is_none());
        assert_eq!(queue.size().unwrap(), 0);
    }

    #[test]
    fn test_mark_failed_with_retry() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.example.com").unwrap();
        queue.mark_failed(id, "Connection error").unwrap();

        let pending = queue.get_pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].retry_count, 1);
        assert_eq!(pending[0].status, MessageStatus::Pending);
        assert_eq!(pending[0].error, Some("Connection error".to_string()));
    }

    #[test]
    fn test_mark_failed_max_retries() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.example.com").unwrap();

        // Fail 3 times (max_retries = 3)
        queue.mark_failed(id, "Error 1").unwrap();
        queue.mark_failed(id, "Error 2").unwrap();
        queue.mark_failed(id, "Error 3").unwrap();

        // Should be moved to failed
        assert_eq!(queue.size().unwrap(), 0);
        let failed = queue.get_failed().unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].status, MessageStatus::Failed);
        assert_eq!(failed[0].retry_count, 3);
    }

    #[test]
    fn test_get_pending() {
        let queue = create_test_queue();

        queue
            .enqueue(&create_test_event("event1"), "wss://relay1.com")
            .unwrap();
        queue
            .enqueue(&create_test_event("event2"), "wss://relay2.com")
            .unwrap();

        let pending = queue.get_pending().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].event_id, "event1");
        assert_eq!(pending[1].event_id, "event2");
    }

    #[test]
    fn test_size() {
        let queue = create_test_queue();

        assert_eq!(queue.size().unwrap(), 0);

        queue
            .enqueue(&create_test_event("event1"), "wss://relay.com")
            .unwrap();
        assert_eq!(queue.size().unwrap(), 1);

        queue
            .enqueue(&create_test_event("event2"), "wss://relay.com")
            .unwrap();
        assert_eq!(queue.size().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let queue = create_test_queue();

        queue
            .enqueue(&create_test_event("event1"), "wss://relay.com")
            .unwrap();
        queue
            .enqueue(&create_test_event("event2"), "wss://relay.com")
            .unwrap();

        assert_eq!(queue.size().unwrap(), 2);
        queue.clear().unwrap();
        assert_eq!(queue.size().unwrap(), 0);
    }

    #[test]
    fn test_clear_sent() {
        let queue = create_test_queue();

        let id1 = queue
            .enqueue(&create_test_event("event1"), "wss://relay.com")
            .unwrap();
        queue
            .enqueue(&create_test_event("event2"), "wss://relay.com")
            .unwrap();

        queue.mark_sent(id1).unwrap();

        let cleared = queue.clear_sent().unwrap();
        assert_eq!(cleared, 1);
        assert_eq!(queue.size().unwrap(), 1); // event2 still pending
    }

    #[test]
    fn test_exponential_backoff() {
        let queue = create_test_queue();

        assert_eq!(queue.calculate_backoff(1), Duration::from_millis(100));
        assert_eq!(queue.calculate_backoff(2), Duration::from_millis(200));
        assert_eq!(queue.calculate_backoff(3), Duration::from_millis(400));
        assert_eq!(queue.calculate_backoff(4), Duration::from_millis(800));
    }

    #[test]
    fn test_backoff_delay() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.com").unwrap();
        queue.mark_failed(id, "Error").unwrap();

        // Immediately try to dequeue - should be blocked by backoff
        let msg = queue.dequeue().unwrap();
        assert!(msg.is_none());

        // Wait for backoff to expire
        thread::sleep(Duration::from_millis(150));

        // Now should be available
        let msg = queue.dequeue().unwrap();
        assert!(msg.is_some());
    }

    #[test]
    fn test_retry_now() {
        let queue = create_test_queue();
        let event = create_test_event("event1");

        let id = queue.enqueue(&event, "wss://relay.com").unwrap();
        queue.mark_failed(id, "Error").unwrap();

        // Should be blocked by backoff
        assert!(queue.dequeue().unwrap().is_none());

        // Reset backoff
        queue.retry_now(id).unwrap();

        // Now should be available
        let msg = queue.dequeue().unwrap();
        assert!(msg.is_some());
    }

    #[test]
    fn test_retry_all() {
        let queue = create_test_queue();

        let id1 = queue
            .enqueue(&create_test_event("event1"), "wss://relay.com")
            .unwrap();
        let id2 = queue
            .enqueue(&create_test_event("event2"), "wss://relay.com")
            .unwrap();

        queue.mark_failed(id1, "Error 1").unwrap();
        queue.mark_failed(id2, "Error 2").unwrap();

        // Should be blocked by backoff
        assert!(queue.dequeue().unwrap().is_none());

        // Reset all backoffs
        queue.retry_all().unwrap();

        // Now should be available
        assert!(queue.dequeue().unwrap().is_some());
        assert!(queue.dequeue().unwrap().is_some());
    }
}
