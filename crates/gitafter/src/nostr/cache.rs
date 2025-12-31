//! Local SQLite cache for storing Nostr events offline

use anyhow::{Context, Result};
use nostr::Event;
use rusqlite::{Connection, OptionalExtension, params};
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
            std::fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let conn = Connection::open(&db_path).context("Failed to open cache database")?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON", [])
            .context("Failed to enable foreign keys")?;

        let cache = Self { conn };
        cache.init_schema()?;

        info!("Event cache initialized at: {:?}", db_path);
        Ok(cache)
    }

    /// Create a new in-memory event cache (for testing)
    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("Failed to open in-memory cache database")?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON", [])
            .context("Failed to enable foreign keys")?;

        let cache = Self { conn };
        cache.init_schema()?;

        debug!("In-memory event cache initialized");
        Ok(cache)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                kind INTEGER NOT NULL,
                pubkey TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                content TEXT NOT NULL,
                tags TEXT NOT NULL,
                sig TEXT NOT NULL,
                cached_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_kind ON events(kind);
            CREATE INDEX IF NOT EXISTS idx_pubkey ON events(pubkey);
            CREATE INDEX IF NOT EXISTS idx_created_at ON events(created_at);

            CREATE TABLE IF NOT EXISTS repositories (
                event_id TEXT PRIMARY KEY,
                name TEXT,
                description TEXT,
                identifier TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS issues (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                status TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS patches (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS pull_requests (
                event_id TEXT PRIMARY KEY,
                repo_address TEXT,
                title TEXT,
                status TEXT,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS watched_repos (
                repo_identifier TEXT PRIMARY KEY,
                repo_address TEXT NOT NULL,
                watched_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                user_pubkey TEXT NOT NULL,
                event_id TEXT NOT NULL,
                event_kind INTEGER NOT NULL,
                notification_type TEXT NOT NULL,
                title TEXT NOT NULL,
                preview TEXT,
                read INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_pubkey);
            CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(read);
            CREATE INDEX IF NOT EXISTS idx_notifications_created ON notifications(created_at);

            -- FTS5 full-text search index for event content
            CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
                id UNINDEXED,
                content,
                content='events',
                content_rowid='rowid'
            );

            -- Triggers to keep FTS5 index in sync
            CREATE TRIGGER IF NOT EXISTS events_fts_insert AFTER INSERT ON events BEGIN
                INSERT INTO events_fts(rowid, id, content)
                VALUES (new.rowid, new.id, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS events_fts_delete AFTER DELETE ON events BEGIN
                DELETE FROM events_fts WHERE rowid = old.rowid;
            END;

            CREATE TRIGGER IF NOT EXISTS events_fts_update AFTER UPDATE ON events BEGIN
                UPDATE events_fts SET content = new.content WHERE rowid = new.rowid;
            END;

            -- Tag value indexes for fast filtering
            CREATE TABLE IF NOT EXISTS event_tags (
                event_id TEXT NOT NULL,
                tag_name TEXT NOT NULL,
                tag_value TEXT NOT NULL,
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE,
                PRIMARY KEY (event_id, tag_name, tag_value)
            ) WITHOUT ROWID;

            CREATE INDEX IF NOT EXISTS idx_event_tags_name_value ON event_tags(tag_name, tag_value);
            CREATE INDEX IF NOT EXISTS idx_event_tags_value ON event_tags(tag_value);

            -- Common query indexes
            CREATE INDEX IF NOT EXISTS idx_events_kind_created ON events(kind, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_events_kind_pubkey ON events(kind, pubkey);
            "#,
            )
            .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Insert or update an event in the cache
    pub fn insert_event(&self, event: &Event) -> Result<()> {
        let tags_json = serde_json::to_string(&event.tags).context("Failed to serialize tags")?;

        let cached_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn
            .execute(
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

        // Index important tags for fast filtering
        self.index_event_tags(event)?;

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

    /// Insert multiple events in a single transaction (batch operation)
    pub fn insert_events_batch(&self, events: &[Event]) -> Result<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let tx = self.conn.unchecked_transaction()?;

        let cached_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Prepare statement once
        let mut event_stmt = tx.prepare_cached(
            r#"
            INSERT OR REPLACE INTO events
            (id, kind, pubkey, created_at, content, tags, sig, cached_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )?;

        let mut tag_delete_stmt =
            tx.prepare_cached("DELETE FROM event_tags WHERE event_id = ?1")?;

        let mut tag_insert_stmt = tx.prepare_cached(
            r#"
            INSERT OR IGNORE INTO event_tags (event_id, tag_name, tag_value)
            VALUES (?1, ?2, ?3)
            "#,
        )?;

        let mut inserted = 0;

        for event in events {
            let tags_json =
                serde_json::to_string(&event.tags).context("Failed to serialize tags")?;

            // Insert event
            event_stmt.execute(params![
                event.id,
                event.kind,
                event.pubkey,
                event.created_at,
                event.content,
                tags_json,
                event.sig,
                cached_at,
            ])?;

            // Delete existing tag indexes
            tag_delete_stmt.execute(params![event.id])?;

            // Index important tags
            for tag in &event.tags {
                if tag.len() >= 2 {
                    let tag_name = &tag[0];
                    if matches!(tag_name.as_str(), "a" | "e" | "p" | "d" | "t") {
                        let tag_value = &tag[1];
                        tag_insert_stmt.execute(params![event.id, tag_name, tag_value])?;
                    }
                }
            }

            // Insert into specialized tables
            match event.kind {
                30617 => self.insert_repository_in_tx(&tx, event)?,
                1621 => self.insert_issue_in_tx(&tx, event)?,
                1617 => self.insert_patch_in_tx(&tx, event)?,
                1618 => self.insert_pull_request_in_tx(&tx, event)?,
                _ => {}
            }

            inserted += 1;
        }

        // Drop prepared statements before committing
        drop(event_stmt);
        drop(tag_delete_stmt);
        drop(tag_insert_stmt);

        tx.commit()?;

        debug!("Batch cached {} events", inserted);
        Ok(inserted)
    }

    /// Index event tags for fast filtering
    fn index_event_tags(&self, event: &Event) -> Result<()> {
        // Delete existing tag indexes for this event
        self.conn.execute(
            "DELETE FROM event_tags WHERE event_id = ?1",
            params![event.id],
        )?;

        // Index important tag types (a, e, p, d)
        for tag in &event.tags {
            if tag.len() >= 2 {
                let tag_name = &tag[0];
                if matches!(tag_name.as_str(), "a" | "e" | "p" | "d" | "t") {
                    let tag_value = &tag[1];
                    self.conn.execute(
                        r#"
                        INSERT OR IGNORE INTO event_tags (event_id, tag_name, tag_value)
                        VALUES (?1, ?2, ?3)
                        "#,
                        params![event.id, tag_name, tag_value],
                    )?;
                }
            }
        }

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

    /// Transaction-aware repository insert
    fn insert_repository_in_tx(&self, tx: &rusqlite::Transaction, event: &Event) -> Result<()> {
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

        tx.execute(
            r#"
            INSERT OR REPLACE INTO repositories
            (event_id, name, description, identifier)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![event.id, name, description, identifier],
        )?;

        Ok(())
    }

    /// Transaction-aware issue insert
    fn insert_issue_in_tx(&self, tx: &rusqlite::Transaction, event: &Event) -> Result<()> {
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

        tx.execute(
            r#"
            INSERT OR REPLACE INTO issues
            (event_id, repo_address, title, status)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![event.id, repo_address, title, status],
        )?;

        Ok(())
    }

    /// Transaction-aware patch insert
    fn insert_patch_in_tx(&self, tx: &rusqlite::Transaction, event: &Event) -> Result<()> {
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

        tx.execute(
            r#"
            INSERT OR REPLACE INTO patches
            (event_id, repo_address, title)
            VALUES (?1, ?2, ?3)
            "#,
            params![event.id, repo_address, title],
        )?;

        Ok(())
    }

    /// Transaction-aware pull request insert
    fn insert_pull_request_in_tx(&self, tx: &rusqlite::Transaction, event: &Event) -> Result<()> {
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

        tx.execute(
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
            let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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
            let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

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

    /// Get repository state for a repository
    /// Repository state is kind:30618 events with 'd' tag matching repository identifier
    pub fn get_repository_state(&self, repo_identifier: &str) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 30618
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that match the repository identifier
        let state = events.into_iter().find(|event| {
            event
                .tags
                .iter()
                .any(|tag| tag.len() >= 2 && tag[0] == "d" && tag[1] == repo_identifier)
        });

        Ok(state)
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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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
    pub fn get_pull_requests_by_repo(
        &self,
        repo_address: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        let deleted = self
            .conn
            .execute("DELETE FROM events WHERE cached_at < ?1", params![cutoff])?;

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

        let issues: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM events WHERE kind = 1621", [], |row| {
                    row.get(0)
                })?;

        let patches: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM events WHERE kind = 1617", [], |row| {
                    row.get(0)
                })?;

        let pull_requests: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM events WHERE kind = 1618", [], |row| {
                    row.get(0)
                })?;

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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == issue_event_id)
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
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == issue_event_id)
            })
            .collect();

        Ok(filtered)
    }

    /// Get bounties for a specific stack layer
    ///
    /// Returns bounties (kind:1636) that have both a stack tag and layer tag matching
    /// the specified stack_id and layer.
    pub fn get_bounties_for_layer(&self, stack_id: &str, layer: u32) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1636
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter for stack_id and layer
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                let has_stack = event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "stack" && tag[1] == stack_id);
                let has_layer = event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "layer" && tag[1] == layer.to_string());
                has_stack && has_layer
            })
            .collect();

        Ok(filtered)
    }

    /// Get all bounties for a stack (all layers)
    ///
    /// Returns all bounties (kind:1636) that have a stack tag matching the stack_id.
    pub fn get_bounties_for_stack(&self, stack_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN event_tags et ON et.event_id = e.id
             WHERE e.kind = 1636 AND et.tag_name = 'stack' AND et.tag_value = ?1
             ORDER BY e.created_at DESC",
        )?;

        let events = stmt
            .query_map(params![stack_id], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    /// Get PR updates for a pull request
    /// PR updates are kind:1619 events that reference the PR via an "e" tag
    pub fn get_pr_updates(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN event_tags et ON et.event_id = e.id
             WHERE e.kind = 1619 AND et.tag_name = 'e' AND et.tag_value = ?1
             ORDER BY e.created_at ASC",
        )?;

        let events = stmt
            .query_map(params![pr_event_id], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    /// Get comments for a specific issue
    /// Comments are kind:1 (text note) events that reference the issue via an "e" tag (NIP-22)
    pub fn get_comments_for_issue(&self, issue_event_id: &str) -> Result<Vec<Event>> {
        // Query for kind:1 events
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1
             ORDER BY created_at ASC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that reference this issue as root
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == issue_event_id)
            })
            .collect();

        Ok(filtered)
    }

    /// Get trajectory session by ID
    /// Trajectory sessions are kind:39230 events (NIP-SA)
    pub fn get_trajectory_session(&self, session_id: &str) -> Result<Option<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 39230 AND id = ?1",
        )?;

        let event = stmt
            .query_row(params![session_id], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                Ok(Event {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    pubkey: row.get(2)?,
                    created_at: row.get(3)?,
                    content: row.get(4)?,
                    tags,
                    sig: row.get(6)?,
                })
            })
            .optional()?;

        Ok(event)
    }

    /// Get trajectory events for a session
    /// Trajectory events are kind:39231 events that reference a session via e tag (NIP-SA)
    pub fn get_trajectory_events(&self, session_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 39231
             ORDER BY created_at ASC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that reference this session
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == session_id)
            })
            .collect();

        Ok(filtered)
    }

    /// Get review comments for a PR or patch
    /// Reviews are kind:1 (text note) events that reference the PR via e tag
    /// These are NIP-22 comments
    pub fn get_reviews_for_pr(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1
             ORDER BY created_at ASC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that reference this PR (NIP-22 comments)
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == pr_event_id)
            })
            .collect();

        Ok(filtered)
    }

    /// Get status events for a PR or patch
    /// Status events are kinds 1630-1633 that reference the PR via e tag
    pub fn get_status_events_for_pr(&self, pr_event_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind IN (1630, 1631, 1632, 1633)
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that reference this PR
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "e" && tag[1] == pr_event_id)
            })
            .collect();

        Ok(filtered)
    }

    /// Get all pull requests by a specific agent (pubkey)
    pub fn get_pull_requests_by_agent(
        &self,
        agent_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1618 AND pubkey = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )?;

        let events = stmt
            .query_map(params![agent_pubkey, limit as i64], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    /// Get all issues claimed by a specific agent (pubkey)
    pub fn get_issue_claims_by_agent(
        &self,
        agent_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1634 AND pubkey = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )?;

        let events = stmt
            .query_map(params![agent_pubkey, limit as i64], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

    /// Get reputation labels for an agent (NIP-32, kind:1985)
    pub fn get_reputation_labels_for_agent(&self, agent_pubkey: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1985
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that reference this agent via p tag
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "p" && tag[1] == agent_pubkey)
            })
            .collect();

        Ok(filtered)
    }

    /// Search repositories by query string (searches name, description, identifier)
    pub fn search_repositories(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        let query_lower = query.to_lowercase();

        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN repositories r ON e.id = r.event_id
             WHERE e.kind = 30617
             ORDER BY e.created_at DESC
             LIMIT ?",
        )?;

        let events = stmt
            .query_map(params![limit as i64], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter by query string (search in name, description, tags)
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                // Search in content (description)
                if event.content.to_lowercase().contains(&query_lower) {
                    return true;
                }

                // Search in tags
                for tag in &event.tags {
                    if tag.len() >= 2 {
                        let tag_value = tag[1].to_lowercase();
                        if tag_value.contains(&query_lower) {
                            return true;
                        }
                    }
                }

                false
            })
            .collect();

        Ok(filtered)
    }

    /// Search issues by query string (searches title, content, labels)
    pub fn search_issues(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        let query_lower = query.to_lowercase();

        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
             FROM events e
             JOIN issues i ON e.id = i.event_id
             WHERE e.kind = 1621
             ORDER BY e.created_at DESC
             LIMIT ?",
        )?;

        let events = stmt
            .query_map(params![limit as i64], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter by query string
        let filtered: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                // Search in content
                if event.content.to_lowercase().contains(&query_lower) {
                    return true;
                }

                // Search in tags (title, labels, etc.)
                for tag in &event.tags {
                    if tag.len() >= 2 {
                        let tag_value = tag[1].to_lowercase();
                        if tag_value.contains(&query_lower) {
                            return true;
                        }
                    }
                }

                false
            })
            .collect();

        Ok(filtered)
    }

    /// Watch a repository
    pub fn watch_repository(&self, repo_identifier: &str, repo_address: &str) -> Result<()> {
        let watched_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO watched_repos (repo_identifier, repo_address, watched_at) VALUES (?, ?, ?)",
            params![repo_identifier, repo_address, watched_at],
        )?;

        debug!("Watching repository: {}", repo_identifier);
        Ok(())
    }

    /// Unwatch a repository
    pub fn unwatch_repository(&self, repo_identifier: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM watched_repos WHERE repo_identifier = ?",
            params![repo_identifier],
        )?;

        debug!("Unwatched repository: {}", repo_identifier);
        Ok(())
    }

    /// Check if a repository is watched
    pub fn is_repository_watched(&self, repo_identifier: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM watched_repos WHERE repo_identifier = ?",
            params![repo_identifier],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }

    /// Get all watched repositories
    pub fn get_watched_repositories(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT repo_identifier FROM watched_repos ORDER BY watched_at DESC")?;

        let identifiers = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(identifiers)
    }

    /// Create a notification
    pub fn create_notification(
        &self,
        user_pubkey: &str,
        event_id: &str,
        event_kind: u16,
        notification_type: &str,
        title: &str,
        preview: Option<&str>,
    ) -> Result<String> {
        let id = format!("notif-{}-{}", user_pubkey, event_id);
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO notifications (id, user_pubkey, event_id, event_kind, notification_type, title, preview, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![id, user_pubkey, event_id, event_kind, notification_type, title, preview, created_at],
        )?;

        debug!("Created notification: {} for user {}", id, user_pubkey);
        Ok(id)
    }

    /// Get unread notifications for a user
    pub fn get_unread_notifications(
        &self,
        user_pubkey: &str,
        limit: usize,
    ) -> Result<Vec<Notification>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_pubkey, event_id, event_kind, notification_type, title, preview, read, created_at
             FROM notifications
             WHERE user_pubkey = ? AND read = 0
             ORDER BY created_at DESC
             LIMIT ?",
        )?;

        let notifications = stmt
            .query_map(params![user_pubkey, limit as i64], |row| {
                Ok(Notification {
                    id: row.get(0)?,
                    user_pubkey: row.get(1)?,
                    event_id: row.get(2)?,
                    event_kind: row.get(3)?,
                    notification_type: row.get(4)?,
                    title: row.get(5)?,
                    preview: row.get(6)?,
                    read: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(notifications)
    }

    /// Get all notifications for a user
    pub fn get_notifications(&self, user_pubkey: &str, limit: usize) -> Result<Vec<Notification>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, user_pubkey, event_id, event_kind, notification_type, title, preview, read, created_at
             FROM notifications
             WHERE user_pubkey = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )?;

        let notifications = stmt
            .query_map(params![user_pubkey, limit as i64], |row| {
                Ok(Notification {
                    id: row.get(0)?,
                    user_pubkey: row.get(1)?,
                    event_id: row.get(2)?,
                    event_kind: row.get(3)?,
                    notification_type: row.get(4)?,
                    title: row.get(5)?,
                    preview: row.get(6)?,
                    read: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(notifications)
    }

    /// Mark a notification as read
    pub fn mark_notification_read(&self, notification_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE notifications SET read = 1 WHERE id = ?",
            params![notification_id],
        )?;

        debug!("Marked notification as read: {}", notification_id);
        Ok(())
    }

    /// Mark all notifications as read for a user
    pub fn mark_all_notifications_read(&self, user_pubkey: &str) -> Result<usize> {
        let updated = self.conn.execute(
            "UPDATE notifications SET read = 1 WHERE user_pubkey = ? AND read = 0",
            params![user_pubkey],
        )?;

        debug!(
            "Marked {} notifications as read for user {}",
            updated, user_pubkey
        );
        Ok(updated)
    }

    /// Get unread notification count for a user
    pub fn get_unread_count(&self, user_pubkey: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE user_pubkey = ? AND read = 0",
            params![user_pubkey],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Clear all cached events
    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        self.conn.execute("DELETE FROM events", [])?;
        info!("Cache cleared");
        Ok(())
    }

    /// Get all pull requests in a stack by stack ID
    ///
    /// Returns PRs ordered by layer position (layer 1, 2, 3, etc.)
    #[allow(dead_code)]
    pub fn get_pull_requests_by_stack(&self, stack_id: &str) -> Result<Vec<Event>> {
        // First get all PR events with the stack tag
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1618
             ORDER BY created_at ASC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter events that have the stack tag matching stack_id
        let mut stack_prs: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "stack" && tag[1] == stack_id)
            })
            .collect();

        // Sort by layer position (extract layer number from tags)
        stack_prs.sort_by_key(|event| {
            event
                .tags
                .iter()
                .find(|tag| tag.len() >= 2 && tag[0] == "layer")
                .and_then(|tag| tag.get(1))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
        });

        Ok(stack_prs)
    }

    /// Get the dependency PR for a given PR (via depends_on tag)
    #[allow(dead_code)]
    pub fn get_dependency_pr(&self, pr_event: &Event) -> Result<Option<Event>> {
        // Find depends_on tag
        let dep_id = pr_event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
            .and_then(|tag| tag.get(1));

        if let Some(event_id) = dep_id {
            self.get_event(event_id)
        } else {
            Ok(None)
        }
    }

    /// Check if a PR's dependencies are satisfied (all dependent PRs are merged)
    #[allow(dead_code)]
    pub fn is_pr_mergeable(&self, pr_event: &Event) -> Result<bool> {
        // If no depends_on tag, it's mergeable
        let dep_id = pr_event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "depends_on")
            .and_then(|tag| tag.get(1));

        if let Some(event_id) = dep_id {
            // Check if dependency PR is merged (has a status event of kind 1631)
            let status_events = self.get_status_events_for_pr(event_id)?;
            let is_merged = status_events.iter().any(|e| e.kind == 1631);
            Ok(is_merged)
        } else {
            // No dependency, mergeable
            Ok(true)
        }
    }

    /// Get PRs that depend on the given PR (later layers in a stack)
    /// Returns PRs that have a depends_on tag referencing this PR's ID
    pub fn get_dependent_prs(&self, pr_id: &str) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, pubkey, created_at, content, tags, sig
             FROM events
             WHERE kind = 1618
             ORDER BY created_at DESC",
        )?;

        let events = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(5)?;
                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

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

        // Filter PRs that have depends_on tag referencing this PR
        let dependent_prs: Vec<Event> = events
            .into_iter()
            .filter(|event| {
                event
                    .tags
                    .iter()
                    .any(|tag| tag.len() >= 2 && tag[0] == "depends_on" && tag[1] == pr_id)
            })
            .collect();

        Ok(dependent_prs)
    }

    /// Full-text search using FTS5
    pub fn search_content(&self, query: &str, limit: usize) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
            FROM events e
            JOIN events_fts ON events_fts.id = e.id
            WHERE events_fts MATCH ?1
            ORDER BY e.created_at DESC
            LIMIT ?2
            "#,
        )?;

        let events = stmt
            .query_map(params![query, limit as i64], |row| {
                let id: String = row.get(0)?;
                let kind: u16 = row.get(1)?;
                let pubkey: String = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                let content: String = row.get(4)?;
                let tags_json: String = row.get(5)?;
                let sig: String = row.get(6)?;

                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).unwrap_or_default();

                Ok(Event {
                    id,
                    kind,
                    pubkey,
                    created_at: created_at as u64,
                    content,
                    tags,
                    sig,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Search by kind and tag value (fast indexed lookup)
    pub fn search_by_kind_and_tag(
        &self,
        kind: u16,
        tag_name: &str,
        tag_value: &str,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
            FROM events e
            JOIN event_tags et ON et.event_id = e.id
            WHERE e.kind = ?1 AND et.tag_name = ?2 AND et.tag_value = ?3
            ORDER BY e.created_at DESC
            LIMIT ?4
            "#,
        )?;

        let events = stmt
            .query_map(params![kind, tag_name, tag_value, limit as i64], |row| {
                let id: String = row.get(0)?;
                let kind: u16 = row.get(1)?;
                let pubkey: String = row.get(2)?;
                let created_at: i64 = row.get(3)?;
                let content: String = row.get(4)?;
                let tags_json: String = row.get(5)?;
                let sig: String = row.get(6)?;

                let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).unwrap_or_default();

                Ok(Event {
                    id,
                    kind,
                    pubkey,
                    created_at: created_at as u64,
                    content,
                    tags,
                    sig,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }

    /// Complex query: repo + kind + optional author filter
    pub fn search_repo_events(
        &self,
        repo_address: &str,
        kind: Option<u16>,
        author_pubkey: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Event>> {
        // Build query based on provided filters
        match (kind, author_pubkey) {
            (Some(k), Some(author)) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
                    FROM events e
                    JOIN event_tags et ON et.event_id = e.id
                    WHERE et.tag_name = 'a' AND et.tag_value = ?1
                      AND e.kind = ?2 AND e.pubkey = ?3
                    ORDER BY e.created_at DESC LIMIT ?4
                    "#,
                )?;

                let events = stmt
                    .query_map(params![repo_address, k, author, limit as i64], |row| {
                        self.parse_event_row(row)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(events)
            }
            (Some(k), None) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
                    FROM events e
                    JOIN event_tags et ON et.event_id = e.id
                    WHERE et.tag_name = 'a' AND et.tag_value = ?1 AND e.kind = ?2
                    ORDER BY e.created_at DESC LIMIT ?3
                    "#,
                )?;

                let events = stmt
                    .query_map(params![repo_address, k, limit as i64], |row| {
                        self.parse_event_row(row)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(events)
            }
            (None, Some(author)) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
                    FROM events e
                    JOIN event_tags et ON et.event_id = e.id
                    WHERE et.tag_name = 'a' AND et.tag_value = ?1 AND e.pubkey = ?2
                    ORDER BY e.created_at DESC LIMIT ?3
                    "#,
                )?;

                let events = stmt
                    .query_map(params![repo_address, author, limit as i64], |row| {
                        self.parse_event_row(row)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(events)
            }
            (None, None) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT DISTINCT e.id, e.kind, e.pubkey, e.created_at, e.content, e.tags, e.sig
                    FROM events e
                    JOIN event_tags et ON et.event_id = e.id
                    WHERE et.tag_name = 'a' AND et.tag_value = ?1
                    ORDER BY e.created_at DESC LIMIT ?2
                    "#,
                )?;

                let events = stmt
                    .query_map(params![repo_address, limit as i64], |row| {
                        self.parse_event_row(row)
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(events)
            }
        }
    }

    /// Helper to parse event from row
    fn parse_event_row(&self, row: &rusqlite::Row) -> rusqlite::Result<Event> {
        let id: String = row.get(0)?;
        let kind: u16 = row.get(1)?;
        let pubkey: String = row.get(2)?;
        let created_at: i64 = row.get(3)?;
        let content: String = row.get(4)?;
        let tags_json: String = row.get(5)?;
        let sig: String = row.get(6)?;

        let tags: Vec<Vec<String>> = serde_json::from_str(&tags_json).unwrap_or_default();

        Ok(Event {
            id,
            kind,
            pubkey,
            created_at: created_at as u64,
            content,
            tags,
            sig,
        })
    }

    /// Explicitly close the database connection
    /// This ensures all pending writes are flushed and allows checking for errors
    /// The connection will be closed automatically when dropped, but calling this
    /// method explicitly allows you to handle potential close errors.
    #[allow(dead_code)]
    pub fn close(self) -> Result<()> {
        self.conn
            .close()
            .map_err(|(_, e)| e)
            .context("Failed to close cache database")
    }
}

// Note: EventCache does not implement Drop explicitly because:
// - rusqlite's Connection already has a Drop impl that handles cleanup
// - The Connection automatically rolls back uncommitted transactions
// - The Connection automatically closes the database connection
// - We provide an explicit close() method for error handling if needed

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

/// Notification stored in cache
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: String,
    pub user_pubkey: String,
    pub event_id: String,
    pub event_kind: u16,
    pub notification_type: String,
    pub title: String,
    pub preview: Option<String>,
    pub read: bool,
    pub created_at: i64,
}
