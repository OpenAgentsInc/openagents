//! Storage trait for event persistence.
//!
//! This module defines the storage interface that relay implementations
//! must provide. The trait is async-agnostic and can be implemented for:
//!
//! - SQLite (native or Cloudflare Durable Object)
//! - PostgreSQL
//! - In-memory (for testing)
//! - Any other storage backend

use crate::Filter;
use nostr::Event;
use thiserror::Error;

/// Errors that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("event not found: {0}")]
    NotFound(String),

    #[error("duplicate event: {0}")]
    Duplicate(String),

    #[error("storage error: {0}")]
    Internal(String),

    #[error("query error: {0}")]
    Query(String),
}

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Query parameters for event retrieval.
#[derive(Debug, Clone, Default)]
pub struct EventQuery {
    /// Filter to match events
    pub filter: Filter,
    /// Offset for pagination
    pub offset: Option<u64>,
}

impl EventQuery {
    /// Create a query from a filter.
    pub fn from_filter(filter: Filter) -> Self {
        Self {
            filter,
            offset: None,
        }
    }

    /// Set pagination offset.
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Storage trait for event persistence.
///
/// Implementations should be thread-safe and handle concurrent access.
/// The trait uses associated types to allow different async runtimes.
pub trait Storage {
    /// Store an event.
    ///
    /// Returns `Ok(true)` if the event was stored (new event).
    /// Returns `Ok(false)` if the event already existed.
    /// Returns `Err` on storage failure.
    fn store_event(&self, event: &Event) -> StorageResult<bool>;

    /// Query events matching a filter.
    ///
    /// Returns events in reverse chronological order (newest first).
    /// Respects the filter's `limit` field.
    fn query_events(&self, query: &EventQuery) -> StorageResult<Vec<Event>>;

    /// Get a single event by ID.
    fn get_event(&self, id: &str) -> StorageResult<Option<Event>>;

    /// Delete an event by ID.
    ///
    /// Returns `Ok(true)` if the event was deleted.
    /// Returns `Ok(false)` if the event didn't exist.
    fn delete_event(&self, id: &str) -> StorageResult<bool>;

    /// Count events matching a filter.
    fn count_events(&self, filter: &Filter) -> StorageResult<u64>;

    /// Check if an event exists.
    fn has_event(&self, id: &str) -> StorageResult<bool> {
        Ok(self.get_event(id)?.is_some())
    }
}

/// SQL query builder for filters.
///
/// This is a helper for SQL-based storage implementations.
pub struct SqlQueryBuilder;

impl SqlQueryBuilder {
    /// Build a WHERE clause from a filter.
    ///
    /// Returns (where_clause, parameters).
    /// Parameters are strings that should be bound to the query.
    pub fn build_where(filter: &Filter) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut params = Vec::new();

        // IDs (prefix match)
        if let Some(ref ids) = filter.ids {
            if !ids.is_empty() {
                let placeholders: Vec<&str> = ids.iter().map(|_| "id LIKE ?").collect();
                conditions.push(format!("({})", placeholders.join(" OR ")));
                for id in ids {
                    params.push(format!("{}%", id));
                }
            }
        }

        // Authors (prefix match)
        if let Some(ref authors) = filter.authors {
            if !authors.is_empty() {
                let placeholders: Vec<&str> = authors.iter().map(|_| "pubkey LIKE ?").collect();
                conditions.push(format!("({})", placeholders.join(" OR ")));
                for author in authors {
                    params.push(format!("{}%", author));
                }
            }
        }

        // Kinds
        if let Some(ref kinds) = filter.kinds {
            if !kinds.is_empty() {
                let placeholders: Vec<String> = kinds.iter().map(|_| "?".to_string()).collect();
                conditions.push(format!("kind IN ({})", placeholders.join(", ")));
                for kind in kinds {
                    params.push(kind.to_string());
                }
            }
        }

        // Since (exclusive)
        if let Some(since) = filter.since {
            conditions.push("created_at > ?".to_string());
            params.push(since.to_string());
        }

        // Until (inclusive)
        if let Some(until) = filter.until {
            conditions.push("created_at <= ?".to_string());
            params.push(until.to_string());
        }

        let where_clause = if conditions.is_empty() {
            "1=1".to_string()
        } else {
            conditions.join(" AND ")
        };

        (where_clause, params)
    }

    /// Build a SELECT query from a filter.
    ///
    /// Returns (sql, parameters).
    pub fn build_select(filter: &Filter) -> (String, Vec<String>) {
        let (where_clause, params) = Self::build_where(filter);
        let limit = filter.limit.unwrap_or(100).min(1000);

        let sql = format!(
            "SELECT id, pubkey, created_at, kind, tags, content, sig \
             FROM events \
             WHERE {} \
             ORDER BY created_at DESC \
             LIMIT {}",
            where_clause, limit
        );

        (sql, params)
    }

    /// Build a COUNT query from a filter.
    pub fn build_count(filter: &Filter) -> (String, Vec<String>) {
        let (where_clause, params) = Self::build_where(filter);

        let sql = format!("SELECT COUNT(*) FROM events WHERE {}", where_clause);

        (sql, params)
    }

    /// SQL to create the events table.
    pub fn create_table_sql() -> &'static str {
        r#"
        CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            kind INTEGER NOT NULL,
            tags TEXT NOT NULL,
            content TEXT NOT NULL,
            sig TEXT NOT NULL
        )
        "#
    }

    /// SQL to create indexes.
    pub fn create_indexes_sql() -> &'static str {
        r#"
        CREATE INDEX IF NOT EXISTS idx_events_pubkey ON events(pubkey);
        CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);
        CREATE INDEX IF NOT EXISTS idx_events_created_at ON events(created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_events_pubkey_kind ON events(pubkey, kind)
        "#
    }

    /// SQL to insert an event.
    pub fn insert_sql() -> &'static str {
        "INSERT OR IGNORE INTO events (id, pubkey, created_at, kind, tags, content, sig) \
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    }

    /// Convert an event to SQL parameter values.
    pub fn event_to_params(event: &Event) -> Vec<String> {
        vec![
            event.id.clone(),
            event.pubkey.clone(),
            event.created_at.to_string(),
            event.kind.to_string(),
            serde_json::to_string(&event.tags).unwrap_or_else(|_| "[]".to_string()),
            event.content.clone(),
            event.sig.clone(),
        ]
    }

    /// Parse an event from SQL row values.
    ///
    /// Expected order: [id, pubkey, created_at, kind, tags, content, sig]
    pub fn event_from_row(row: &[String]) -> Option<Event> {
        if row.len() < 7 {
            return None;
        }

        Some(Event {
            id: row[0].clone(),
            pubkey: row[1].clone(),
            created_at: row[2].parse().ok()?,
            kind: row[3].parse().ok()?,
            tags: serde_json::from_str(&row[4]).ok()?,
            content: row[5].clone(),
            sig: row[6].clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_query_builder_kinds() {
        let filter = Filter::new().kinds([1, 7]);
        let (sql, params) = SqlQueryBuilder::build_select(&filter);

        assert!(sql.contains("kind IN (?, ?)"));
        assert_eq!(params, vec!["1", "7"]);
    }

    #[test]
    fn test_sql_query_builder_authors() {
        let filter = Filter::new().authors(["abc", "xyz"]);
        let (sql, params) = SqlQueryBuilder::build_select(&filter);

        assert!(sql.contains("pubkey LIKE ?"));
        assert_eq!(params, vec!["abc%", "xyz%"]);
    }

    #[test]
    fn test_sql_query_builder_combined() {
        let filter = Filter::new()
            .kinds([1])
            .authors(["abc"])
            .since(1000)
            .until(2000)
            .limit(50);

        let (sql, params) = SqlQueryBuilder::build_select(&filter);

        assert!(sql.contains("kind IN (?)"));
        assert!(sql.contains("pubkey LIKE ?"));
        assert!(sql.contains("created_at > ?"));
        assert!(sql.contains("created_at <= ?"));
        assert!(sql.contains("LIMIT 50"));
        assert_eq!(params, vec!["abc%", "1", "1000", "2000"]);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            id: "abc123".to_string(),
            pubkey: "pubkey".to_string(),
            created_at: 1234567890,
            kind: 1,
            tags: vec![vec!["e".to_string(), "eventid".to_string()]],
            content: "Hello".to_string(),
            sig: "signature".to_string(),
        };

        let params = SqlQueryBuilder::event_to_params(&event);
        assert_eq!(params.len(), 7);
        assert_eq!(params[0], "abc123");
        assert_eq!(params[2], "1234567890");
        assert!(params[4].contains("eventid"));

        let parsed = SqlQueryBuilder::event_from_row(&params).unwrap();
        assert_eq!(parsed.id, event.id);
        assert_eq!(parsed.kind, event.kind);
        assert_eq!(parsed.tags, event.tags);
    }
}
