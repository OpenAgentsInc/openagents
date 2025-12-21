//! Local SQLite cache for storing Nostr events offline

use anyhow::{Context, Result};
use nostr::Event;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use tracing::{debug, info};

/// SQLite-based event cache for offline storage
pub struct EventCache {
    conn: Connection,
}

impl EventCache {
    /// Create a new event cache with the given database path
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create cache directory")?;
        }

        let conn = Connection::open(&db_path)
            .context("Failed to open cache database")?;

        let cache = Self { conn };
        cache.init_schema()?;

        info!("Event cache initialized at: {:?}", db_path);
        Ok(cache)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                kind INTEGER NOT NULL,
                pubkey TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                sig TEXT NOT NULL,
                cached_at INTEGER NOT NULL,
                INDEX idx_kind ON events(kind),
                INDEX idx_pubkey ON events(pubkey),
                INDEX idx_created_at ON events(created_at)
            );

            CREATE TABLE IF NOT EXISTS repositories (
                event_id TEXT PRIMARY KEY,
                name TEXT,
                description TEXT,
                identifier TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id)
            );

            CREATE TABLE IF NOT EXISTS issues (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                status TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id)
            );

            CREATE TABLE IF NOT EXISTS patches (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id)
            );

            CREATE TABLE IF NOT EXISTS pull_requests (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                status TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id)
            );
            "#,
        )
        .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Insert or update an event in the cache
    pub fn insert_event(&self, event: &Event) -> Result<()> {
        let tags_json = serde_json::to_string(&event.tags)
            .context("Failed to serialize tags")?;

        let cached_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO events
            (id, kind, pubkey, created_at, content, tags, sig, cached_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                event.id,
                event.kind,
                event.pubkey,
                event.created_at,
                event.content,
                tags_json,
                event.sig,
                cached_at,
            ],
        )
        .context("Failed to insert event")?;

        // Insert into specialized tables based on event kind
        match event.kind {
            30617 => self.insert_repository(event)?,
            1621 => self.insert_issue(event)?,
            1617 => self.insert_patch(event)?,
            1618 => self.insert_pull_request(event)?,
            _ => {}
        }

        debug!("Cached event: id={} kind={}", event.id, event.kind);
        Ok(())
    }

    /// Insert repository metadata
    fn insert_repository(&self, event: &Event) -> Result<()> {
        let name = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "name")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let description = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "description")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let identifier = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "d")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO repositories
            (event_id, name, description, identifier)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![event.id, name, description, identifier],
        )?;

        Ok(())
    }

    /// Insert issue metadata
    fn insert_issue(&self, event: &Event) -> Result<()> {
        let repo_address = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "a")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let title = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "subject")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let status = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "status")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO issues
            (event_id, repo_address, title, status)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![event.id, repo_address, title, status],
        )?;

        Ok(())
    }

    /// Insert patch metadata
    fn insert_patch(&self, event: &Event) -> Result<()> {
        let repo_address = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "a")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let title = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "subject")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO patches
            (event_id, repo_address, title)
            VALUES (?1, ?2, ?3)
            "#,
            params![event.id, repo_address, title],
        )?;

        Ok(())
    }

    /// Insert pull request metadata
    fn insert_pull_request(&self, event: &Event) -> Result<()> {
        let repo_address = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "a")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let title = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "subject")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        let status = event
            .tags
            .iter()
            .find(|t| t.len() >= 2 && t[0] == "status")
            .and_then(|t| t.get(1))
            .map(|s| s.as_str());

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO pull_requests
            (event_id, repo_address, title, status)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![event.id, repo_address, title, status],
        )?;

        Ok(())
    }

    /// Get event by ID
    #[allow(dead_code)]
    pub fn get_event(&self, event_id: &str) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events WHERE id = ?1",
        )?;

        match stmt.query_row(params![event_id], |row| {
            let tags_json: String = row.get(5)?;
            let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?;

            Ok(Event {
                id: row.get(0)?,
                kind: row.get(1)?,
                pubkey: row.get(2)?,
                created_at: row.get(3)?,
                content: row.get(4)?,
                tags,
                sig: row.get(6)?,
            })
        }) {
            Ok(event) => Ok(Some(event)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get events by kind
    #[allow(dead_code)]
    pub fn get_events_by_kind(&self, kind: u16, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events WHERE kind = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![kind, limit], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get all repositories from cache
    #[allow(dead_code)]
    pub fn get_repositories(&self, limit: usize) -> Result<Vec<Event>> {
        self.get_events_by_kind(30617, limit)
    }

    /// Get all issues from cache
    #[allow(dead_code)]
    pub fn get_issues(&self, limit: usize) -> Result<Vec<Event>> {
        self.get_events_by_kind(1621, limit)
    }

    /// Get all patches from cache
    #[allow(dead_code)]
    pub fn get_patches(&self, limit: usize) -> Result<Vec<Event>> {
        self.get_events_by_kind(1617, limit)
    }

    /// Get all pull requests from cache
    #[allow(dead_code)]
    pub fn get_pull_requests(&self, limit: usize) -> Result<Vec<Event>> {
        self.get_events_by_kind(1618, limit)
    }

    /// Get a repository by its identifier (d tag)
    pub fn get_repository_by_identifier(&self, identifier: &str) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN repositories r ON e.id = r.event_id
             WHERE r.identifier = ?1",
        )?;

        match stmt.query_row(params![identifier], |row| {
            let tags_json: String = row.get(5)?;
            let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                ))?;

            Ok(Event {
                id: row.get(0)?,
                kind: row.get(1)?,
                pubkey: row.get(2)?,
                created_at: row.get(3)?,
                content: row.get(4)?,
                tags,
                sig: row.get(6)?,
            })
        }) {
            Ok(event) => Ok(Some(event)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get issues for a specific repository by its address tag
    pub fn get_issues_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN issues i ON e.id = i.event_id
             WHERE i.repo_address = ?1
             ORDER BY e.created_at DESC
             LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![repo_address, limit], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get patches for a specific repository by its address tag
    pub fn get_patches_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN patches p ON e.id = p.event_id
             WHERE p.repo_address = ?1
             ORDER BY e.created_at DESC
             LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![repo_address, limit], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Get pull requests for a specific repository by its address tag
    pub fn get_pull_requests_by_repo(&self, repo_address: &str, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN pull_requests pr ON e.id = pr.event_id
             WHERE pr.repo_address = ?1
             ORDER BY e.created_at DESC
             LIMIT ?2",
        )?;

        let events = stmt
            .query_map(params![repo_address, limit], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Delete events older than the specified age (in seconds)
    #[allow(dead_code)]
    pub fn delete_old_events(&self, max_age_seconds: i64) -> Result<usize> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - max_age_seconds;

        let deleted = self.conn.execute(
            "DELETE FROM events WHERE cached_at < ?1",
            params![cutoff],
        )?;

        info!("Deleted {} old events from cache", deleted);
        Ok(deleted)
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub fn get_stats(&self) -> Result<CacheStats> {
        let total_events: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))?;

        let repositories: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE kind = 30617",
            [],
            |row| row.get(0),
        )?;

        let issues: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE kind = 1621",
            [],
            |row| row.get(0),
        )?;

        let patches: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE kind = 1617",
            [],
            |row| row.get(0),
        )?;

        let pull_requests: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE kind = 1618",
            [],
            |row| row.get(0),
        )?;

        Ok(CacheStats {
            total_events: total_events as usize,
            repositories: repositories as usize,
            issues: issues as usize,
            patches: patches as usize,
            pull_requests: pull_requests as usize,
        })
    }

    /// Get claims for a specific issue
    /// Claims are kind:1634 events that reference the issue via an "e" tag
    pub fn get_claims_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        // Query for kind:1634 events where tags contains ["e", "<issue_event_id>", ...]
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1634
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Filter events that reference this issue
        let filtered: Vec<Event> = events.into_iter()
            .filter(|event| {
                event.tags.iter().any(|tag| {
                    tag.len() >= 2 && tag[0] == "e" && tag[1] == issue_event_id
                })
            })
            .collect();

        Ok(filtered)
    }

    /// Get bounty offers for a specific issue
    /// Bounty offers are kind:1636 events that reference the issue via an "e" tag
    pub fn get_bounties_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        // Query for kind:1636 events
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1636
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json)
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Filter events that reference this issue
        let filtered: Vec<Event> = events.into_iter()
            .filter(|event| {
                event.tags.iter().any(|tag| {
                    tag.len() >= 2 && tag[0] == "e" && tag[1] == issue_event_id
                })
            })
            .collect();

        Ok(filtered)
    }

    /// Clear all cached events
    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM events", [])?;
        info!("Cache cleared");
        Ok(())
    }
}

/// Cache statistics
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_events: usize,
    pub repositories: usize,
    pub issues: usize,
    pub patches: usize,
    pub pull_requests: usize,
}
