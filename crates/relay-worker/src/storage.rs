//! Hybrid D1 + DO storage layer
//!
//! - D1 Database: Persistent event storage
//! - DO Transactional Storage: Hot cache for recent events

use serde::{Deserialize, Serialize};
use worker::*;

use crate::subscription::Filter;

/// Maximum events to cache in DO storage
const MAX_CACHED_EVENTS: usize = 1000;

/// Cache TTL in seconds (5 minutes)
const CACHE_TTL_SECONDS: u64 = 300;

/// Storage abstraction for the relay
pub struct Storage<'a> {
    state: &'a State,
    env: &'a Env,
}

impl<'a> Storage<'a> {
    pub fn new(state: &'a State, env: &'a Env) -> Self {
        Self { state, env }
    }

    /// Get D1 database binding
    fn db(&self) -> Result<D1Database> {
        self.env.d1("DB")
    }

    /// Store an event in both D1 and DO cache
    pub async fn store_event(&self, event: &nostr::Event) -> Result<()> {
        // Store in D1 (persistent)
        self.store_event_d1(event).await?;

        // Store in DO cache (fast access)
        self.cache_event(event).await?;

        Ok(())
    }

    /// Store event in D1 database
    async fn store_event_d1(&self, event: &nostr::Event) -> Result<()> {
        let db = self.db()?;

        let tags_json = serde_json::to_string(&event.tags)?;

        let statement = db.prepare(
            "INSERT OR REPLACE INTO events (id, pubkey, kind, created_at, content, tags, sig) VALUES (?, ?, ?, ?, ?, ?, ?)"
        );

        statement
            .bind(&[
                event.id.clone().into(),
                event.pubkey.clone().into(),
                // Use f64 instead of i64 for D1 compatibility (BigInt not supported)
                (event.kind as f64).into(),
                (event.created_at as f64).into(),
                event.content.clone().into(),
                tags_json.into(),
                event.sig.clone().into(),
            ])?
            .run()
            .await?;

        Ok(())
    }

    /// Cache event in DO storage
    async fn cache_event(&self, event: &nostr::Event) -> Result<()> {
        let key = format!("event:{}", event.id);
        let cache_entry = CachedEvent {
            event: event.clone(),
            cached_at: js_sys::Date::now() as u64 / 1000,
        };

        self.state.storage().put(&key, &cache_entry).await?;

        // Add to recent events list (for quick broadcast matching)
        self.add_to_recent_events(&event.id).await?;

        Ok(())
    }

    /// Add event ID to recent events list
    async fn add_to_recent_events(&self, event_id: &str) -> Result<()> {
        let mut recent: Vec<String> = self
            .state
            .storage()
            .get("recent_events")
            .await?
            .unwrap_or_default();

        recent.insert(0, event_id.to_string());

        // Limit cache size
        if recent.len() > MAX_CACHED_EVENTS {
            let removed = recent.split_off(MAX_CACHED_EVENTS);
            // Clean up removed events from cache
            for id in removed {
                let _ = self.state.storage().delete(&format!("event:{}", id)).await;
            }
        }

        self.state.storage().put("recent_events", &recent).await?;

        Ok(())
    }

    /// Query events matching a filter
    pub async fn query_events(&self, filter: &Filter) -> Result<Vec<nostr::Event>> {
        // Try DO cache first for simple queries
        if self.can_use_cache(filter) {
            if let Ok(events) = self.query_cache(filter).await {
                if !events.is_empty() || filter.limit.map_or(false, |l| l == 0) {
                    return Ok(events);
                }
            }
        }

        // Fall back to D1
        self.query_d1(filter).await
    }

    /// Check if cache can satisfy this query
    fn can_use_cache(&self, filter: &Filter) -> bool {
        // Cache is useful for recent events without complex filters
        filter.since.is_none() || filter.since.unwrap() > (js_sys::Date::now() as u64 / 1000 - CACHE_TTL_SECONDS)
    }

    /// Query events from DO cache
    async fn query_cache(&self, filter: &Filter) -> Result<Vec<nostr::Event>> {
        let recent: Vec<String> = self
            .state
            .storage()
            .get("recent_events")
            .await?
            .unwrap_or_default();

        let mut events = Vec::new();
        let limit = filter.limit.unwrap_or(100);

        for event_id in recent {
            if events.len() >= limit {
                break;
            }

            if let Some(cached) = self
                .state
                .storage()
                .get::<CachedEvent>(&format!("event:{}", event_id))
                .await?
            {
                // Check if still fresh
                let now = js_sys::Date::now() as u64 / 1000;
                if now - cached.cached_at < CACHE_TTL_SECONDS {
                    if filter.matches(&cached.event) {
                        events.push(cached.event);
                    }
                }
            }
        }

        Ok(events)
    }

    /// Query events from D1 database
    async fn query_d1(&self, filter: &Filter) -> Result<Vec<nostr::Event>> {
        let db = self.db()?;

        let (where_clause, _params) = filter.to_sql_conditions();
        let limit = filter.limit.unwrap_or(100).min(500);

        // Build query
        let query = format!(
            "SELECT id, pubkey, kind, created_at, content, tags, sig FROM events WHERE {} ORDER BY created_at DESC LIMIT {}",
            where_clause, limit
        );

        let statement = db.prepare(&query);

        // Note: D1 binding is limited, we need to handle params carefully
        // For now, we'll use a simpler approach without parameterized queries for complex filters
        let result = statement.all().await?;

        let mut events = Vec::new();
        for row in result.results::<EventRow>()? {
            let tags: Vec<Vec<String>> = serde_json::from_str(&row.tags).unwrap_or_default();
            let event = nostr::Event {
                id: row.id,
                pubkey: row.pubkey,
                created_at: row.created_at as u64,
                kind: row.kind as u16,
                tags,
                content: row.content,
                sig: row.sig,
            };

            // Double-check filter match (for tag filters that aren't in SQL)
            if filter.matches(&event) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Get a specific event by ID
    pub async fn get_event(&self, event_id: &str) -> Result<Option<nostr::Event>> {
        // Check cache first
        if let Some(cached) = self
            .state
            .storage()
            .get::<CachedEvent>(&format!("event:{}", event_id))
            .await?
        {
            return Ok(Some(cached.event));
        }

        // Query D1
        let db = self.db()?;
        let statement = db.prepare("SELECT id, pubkey, kind, created_at, content, tags, sig FROM events WHERE id = ?");
        let result = statement.bind(&[event_id.into()])?.first::<EventRow>(None).await?;

        if let Some(row) = result {
            let tags: Vec<Vec<String>> = serde_json::from_str(&row.tags).unwrap_or_default();
            Ok(Some(nostr::Event {
                id: row.id,
                pubkey: row.pubkey,
                created_at: row.created_at as u64,
                kind: row.kind as u16,
                tags,
                content: row.content,
                sig: row.sig,
            }))
        } else {
            Ok(None)
        }
    }
}

/// Cached event with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEvent {
    event: nostr::Event,
    cached_at: u64,
}

/// D1 row structure
#[derive(Debug, Deserialize)]
struct EventRow {
    id: String,
    pubkey: String,
    kind: i64,
    created_at: i64,
    content: String,
    tags: String,
    sig: String,
}
