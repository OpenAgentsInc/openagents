use crate::types::{
    AttachmentStorageMode, AuditResponse, DraftRecord, DraftStatus, EventRecord, MessageRecord,
    PolicyDecision, PrivacyMode, RiskTier, SettingsResponse, TemplateSuggestion, ThreadCategory,
    ThreadDetailResponse, ThreadSummary,
};
use crate::vault::Vault;
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertThread {
    pub id: String,
    pub gmail_thread_id: String,
    pub subject: String,
    pub snippet: String,
    pub from_address: String,
    pub last_message_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertMessage {
    pub id: String,
    pub thread_id: String,
    pub gmail_message_id: String,
    pub sender: String,
    pub recipient: String,
    pub subject: String,
    pub snippet: String,
    pub body: String,
    pub inbound: bool,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ClassificationDecision {
    pub category: ThreadCategory,
    pub risk: RiskTier,
    pub policy: PolicyDecision,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NewDraft {
    pub thread_id: String,
    pub body: String,
    pub source_summary: String,
    pub model_used: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DraftQualitySample {
    pub thread_id: String,
    pub category: ThreadCategory,
    pub generated_draft: String,
    pub sent_reply: String,
}

#[derive(Clone)]
pub struct Database {
    conn: std::sync::Arc<Mutex<Connection>>,
    vault: Vault,
}

impl Database {
    pub fn open(db_path: PathBuf, vault: Vault) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create db parent directory {}", parent.display())
            })?;
        }
        let conn = Connection::open(db_path).context("failed to open sqlite db")?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS oauth_tokens (
                provider TEXT PRIMARY KEY,
                access_token TEXT,
                refresh_token TEXT,
                expires_at INTEGER,
                scope TEXT,
                token_type TEXT,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS threads (
                id TEXT PRIMARY KEY,
                gmail_thread_id TEXT NOT NULL UNIQUE,
                subject TEXT NOT NULL,
                snippet TEXT NOT NULL,
                from_address TEXT NOT NULL,
                category TEXT,
                risk TEXT,
                policy TEXT,
                reason_codes TEXT,
                similar_thread_ids TEXT,
                external_model_used INTEGER NOT NULL DEFAULT 0,
                last_message_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                gmail_message_id TEXT NOT NULL UNIQUE,
                sender TEXT NOT NULL,
                recipient TEXT NOT NULL,
                subject TEXT NOT NULL,
                snippet TEXT NOT NULL,
                body TEXT NOT NULL,
                inbound INTEGER NOT NULL,
                sent_at INTEGER NOT NULL,
                FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS drafts (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                body TEXT NOT NULL,
                status TEXT NOT NULL,
                source_summary TEXT NOT NULL,
                model_used TEXT,
                gmail_message_id TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                thread_id TEXT,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(thread_id) REFERENCES threads(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_threads_last_message_at ON threads(last_message_at DESC);
            CREATE INDEX IF NOT EXISTS idx_messages_thread_sent ON messages(thread_id, sent_at DESC);
            CREATE INDEX IF NOT EXISTS idx_drafts_status ON drafts(status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_events_thread ON events(thread_id, created_at DESC);
            ",
        )
        .context("failed running migrations")?;

        let db = Self {
            conn: std::sync::Arc::new(Mutex::new(conn)),
            vault,
        };
        db.ensure_default_settings()?;
        Ok(db)
    }

    fn ensure_default_settings(&self) -> Result<()> {
        let defaults: [(&str, &str); 8] = [
            ("privacy_mode", "hybrid"),
            ("backfill_days", "90"),
            ("allowed_recipient_domains", "[]"),
            ("attachment_storage_mode", "metadata"),
            ("signature", ""),
            ("template_scheduling", ""),
            ("template_report_delivery", ""),
            ("sync_interval_seconds", "60"),
        ];
        let conn = self.conn.lock();
        for (key, value) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .with_context(|| format!("failed inserting default setting {key}"))?;
        }
        Ok(())
    }

    pub fn store_provider_token(
        &self,
        provider: &str,
        access_token: Option<&str>,
        refresh_token: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
        scope: Option<&str>,
        token_type: Option<&str>,
    ) -> Result<()> {
        let encrypted_access = access_token
            .map(|v| self.vault.encrypt(v))
            .transpose()
            .context("failed to encrypt access token")?;
        let encrypted_refresh = refresh_token
            .map(|v| self.vault.encrypt(v))
            .transpose()
            .context("failed to encrypt refresh token")?;

        let conn = self.conn.lock();
        conn.execute(
            "
            INSERT INTO oauth_tokens(provider, access_token, refresh_token, expires_at, scope, token_type, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(provider) DO UPDATE SET
                access_token=excluded.access_token,
                refresh_token=COALESCE(excluded.refresh_token, oauth_tokens.refresh_token),
                expires_at=excluded.expires_at,
                scope=excluded.scope,
                token_type=excluded.token_type,
                updated_at=excluded.updated_at
            ",
            params![
                provider,
                encrypted_access,
                encrypted_refresh,
                expires_at.map(|v| v.timestamp()),
                scope,
                token_type,
                Utc::now().timestamp()
            ],
        )
        .context("failed storing provider token")?;
        Ok(())
    }

    pub fn get_provider_token(&self, provider: &str) -> Result<Option<ProviderToken>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT provider, access_token, refresh_token, expires_at, scope, token_type, updated_at
                FROM oauth_tokens WHERE provider = ?1
                ",
                params![provider],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()
            .context("failed querying provider token")?;

        let Some((
            provider_name,
            access_enc,
            refresh_enc,
            expires_at_ts,
            scope,
            token_type,
            updated_at_ts,
        )) = row
        else {
            return Ok(None);
        };

        let access_token = access_enc
            .map(|v| self.vault.decrypt(&v))
            .transpose()
            .context("failed decrypting access token")?;
        let refresh_token = refresh_enc
            .map(|v| self.vault.decrypt(&v))
            .transpose()
            .context("failed decrypting refresh token")?;

        Ok(Some(ProviderToken {
            provider: provider_name,
            access_token,
            refresh_token,
            expires_at: expires_at_ts.map(ts_to_datetime),
            scope,
            token_type,
            updated_at: ts_to_datetime(updated_at_ts),
        }))
    }

    pub fn oauth_status(&self, provider: &str) -> Result<(bool, Option<DateTime<Utc>>)> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT updated_at FROM oauth_tokens WHERE provider = ?1",
                params![provider],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .context("failed querying oauth status")?;

        Ok(match row {
            Some(updated_at) => (true, Some(ts_to_datetime(updated_at))),
            None => (false, None),
        })
    }

    pub fn upsert_thread(&self, thread: UpsertThread) -> Result<()> {
        let now = Utc::now().timestamp();
        let conn = self.conn.lock();
        conn.execute(
            "
            INSERT INTO threads(id, gmail_thread_id, subject, snippet, from_address, last_message_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(gmail_thread_id) DO UPDATE SET
                subject=excluded.subject,
                snippet=excluded.snippet,
                from_address=excluded.from_address,
                last_message_at=excluded.last_message_at,
                updated_at=excluded.updated_at
            ",
            params![
                thread.id,
                thread.gmail_thread_id,
                thread.subject,
                thread.snippet,
                thread.from_address,
                thread.last_message_at.timestamp(),
                now,
            ],
        )
        .context("failed upserting thread")?;
        Ok(())
    }

    pub fn upsert_message(&self, message: UpsertMessage) -> Result<()> {
        let encrypted_body = self
            .vault
            .encrypt(&message.body)
            .context("failed encrypting message body")?;
        let conn = self.conn.lock();
        conn.execute(
            "
            INSERT INTO messages(
                id, thread_id, gmail_message_id, sender, recipient, subject, snippet, body, inbound, sent_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(gmail_message_id) DO UPDATE SET
                sender=excluded.sender,
                recipient=excluded.recipient,
                subject=excluded.subject,
                snippet=excluded.snippet,
                body=excluded.body,
                inbound=excluded.inbound,
                sent_at=excluded.sent_at
            ",
            params![
                message.id,
                message.thread_id,
                message.gmail_message_id,
                message.sender,
                message.recipient,
                message.subject,
                message.snippet,
                encrypted_body,
                if message.inbound { 1 } else { 0 },
                message.sent_at.timestamp(),
            ],
        )
        .context("failed upserting message")?;
        Ok(())
    }

    pub fn list_threads(&self, search: Option<String>, limit: usize) -> Result<Vec<ThreadSummary>> {
        let conn = self.conn.lock();
        let limit = limit.clamp(1, 500) as i64;

        let sql_with_search = "
            SELECT
                t.id, t.subject, t.snippet, t.from_address, t.category, t.risk, t.policy,
                t.last_message_at,
                EXISTS(SELECT 1 FROM drafts d WHERE d.thread_id = t.id AND d.status = 'pending')
            FROM threads t
            WHERE LOWER(t.subject) LIKE LOWER(?1)
               OR LOWER(t.snippet) LIKE LOWER(?1)
               OR LOWER(t.from_address) LIKE LOWER(?1)
            ORDER BY t.last_message_at DESC
            LIMIT ?2
        ";

        let sql_without_search = "
            SELECT
                t.id, t.subject, t.snippet, t.from_address, t.category, t.risk, t.policy,
                t.last_message_at,
                EXISTS(SELECT 1 FROM drafts d WHERE d.thread_id = t.id AND d.status = 'pending')
            FROM threads t
            ORDER BY t.last_message_at DESC
            LIMIT ?1
        ";

        let mut results = Vec::new();
        if let Some(search_term) = search {
            let pattern = format!("%{}%", search_term.trim());
            let mut stmt = conn.prepare(sql_with_search)?;
            let rows = stmt.query_map(params![pattern, limit], row_to_thread_summary)?;
            for row in rows {
                results.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(sql_without_search)?;
            let rows = stmt.query_map(params![limit], row_to_thread_summary)?;
            for row in rows {
                results.push(row?);
            }
        }
        Ok(results)
    }

    pub fn get_thread(&self, thread_id: &str) -> Result<Option<ThreadSummary>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT
                    t.id, t.subject, t.snippet, t.from_address, t.category, t.risk, t.policy,
                    t.last_message_at,
                    EXISTS(SELECT 1 FROM drafts d WHERE d.thread_id = t.id AND d.status = 'pending')
                FROM threads t
                WHERE t.id = ?1
                ",
                params![thread_id],
                row_to_thread_summary,
            )
            .optional()
            .context("failed querying thread")?;
        Ok(row)
    }

    pub fn get_thread_detail(&self, thread_id: &str) -> Result<Option<ThreadDetailResponse>> {
        let thread = match self.get_thread(thread_id)? {
            Some(t) => t,
            None => return Ok(None),
        };

        let conn = self.conn.lock();
        let mut msg_stmt = conn.prepare(
            "
            SELECT id, thread_id, sender, recipient, body, snippet, inbound, sent_at
            FROM messages
            WHERE thread_id = ?1
            ORDER BY sent_at ASC
            ",
        )?;
        let mut messages = Vec::new();
        let rows = msg_stmt.query_map(params![thread_id], |row| {
            let body_raw: String = row.get(4)?;
            Ok(MessageRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                sender: row.get(2)?,
                recipient: row.get(3)?,
                body: self.decrypt_corpus_text(body_raw),
                snippet: row.get(5)?,
                inbound: row.get::<_, i64>(6)? == 1,
                sent_at: ts_to_datetime(row.get(7)?),
            })
        })?;
        for row in rows {
            messages.push(row?);
        }

        let draft = self.latest_draft_for_thread(thread_id)?;

        Ok(Some(ThreadDetailResponse {
            thread,
            messages,
            draft,
        }))
    }

    pub fn latest_inbound_message(&self, thread_id: &str) -> Result<Option<MessageRecord>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT id, thread_id, sender, recipient, body, snippet, inbound, sent_at
                FROM messages
                WHERE thread_id = ?1 AND inbound = 1
                ORDER BY sent_at DESC
                LIMIT 1
                ",
                params![thread_id],
                |row| {
                    let body_raw: String = row.get(4)?;
                    Ok(MessageRecord {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        sender: row.get(2)?,
                        recipient: row.get(3)?,
                        body: self.decrypt_corpus_text(body_raw),
                        snippet: row.get(5)?,
                        inbound: row.get::<_, i64>(6)? == 1,
                        sent_at: ts_to_datetime(row.get(7)?),
                    })
                },
            )
            .optional()
            .context("failed querying inbound message")?;
        Ok(row)
    }

    pub fn recent_sent_messages(&self, limit: usize) -> Result<Vec<MessageRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "
            SELECT id, thread_id, sender, recipient, body, snippet, inbound, sent_at
            FROM messages
            WHERE inbound = 0
            ORDER BY sent_at DESC
            LIMIT ?1
            ",
        )?;
        let mut out = Vec::new();
        let rows = stmt.query_map(params![limit as i64], |row| {
            let body_raw: String = row.get(4)?;
            Ok(MessageRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                sender: row.get(2)?,
                recipient: row.get(3)?,
                body: self.decrypt_corpus_text(body_raw),
                snippet: row.get(5)?,
                inbound: row.get::<_, i64>(6)? == 1,
                sent_at: ts_to_datetime(row.get(7)?),
            })
        })?;
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn apply_classification(
        &self,
        thread_id: &str,
        decision: &ClassificationDecision,
    ) -> Result<()> {
        let reason_codes = serde_json::to_string(&decision.reason_codes)
            .context("failed serializing reason codes")?;
        let conn = self.conn.lock();
        conn.execute(
            "
            UPDATE threads
            SET category = ?2,
                risk = ?3,
                policy = ?4,
                reason_codes = ?5,
                updated_at = ?6
            WHERE id = ?1
            ",
            params![
                thread_id,
                decision.category.as_str(),
                risk_to_str(decision.risk),
                decision.policy.as_str(),
                reason_codes,
                Utc::now().timestamp(),
            ],
        )
        .context("failed updating classification")?;
        Ok(())
    }

    pub fn set_thread_draft_metadata(
        &self,
        thread_id: &str,
        similar_thread_ids: &[String],
        external_model_used: bool,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "
            UPDATE threads
            SET similar_thread_ids = ?2,
                external_model_used = ?3,
                updated_at = ?4
            WHERE id = ?1
            ",
            params![
                thread_id,
                serde_json::to_string(similar_thread_ids)
                    .context("failed serializing similar thread ids")?,
                if external_model_used { 1 } else { 0 },
                Utc::now().timestamp(),
            ],
        )
        .context("failed updating draft metadata")?;
        Ok(())
    }

    pub fn create_draft(&self, new_draft: NewDraft) -> Result<DraftRecord> {
        let now = Utc::now();
        let draft_id = Uuid::new_v4().to_string();
        let encrypted_body = self
            .vault
            .encrypt(&new_draft.body)
            .context("failed encrypting draft body")?;
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE drafts SET status = 'rejected', updated_at = ?2 WHERE thread_id = ?1 AND status = 'pending'",
            params![new_draft.thread_id, now.timestamp()],
        )
        .context("failed archiving existing pending drafts")?;

        conn.execute(
            "
            INSERT INTO drafts(id, thread_id, body, status, source_summary, model_used, created_at, updated_at)
            VALUES(?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7)
            ",
            params![
                draft_id,
                new_draft.thread_id,
                encrypted_body,
                new_draft.source_summary,
                new_draft.model_used,
                now.timestamp(),
                now.timestamp(),
            ],
        )
        .context("failed creating draft")?;

        Ok(DraftRecord {
            id: draft_id,
            thread_id: new_draft.thread_id,
            body: new_draft.body,
            status: DraftStatus::Pending,
            source_summary: new_draft.source_summary,
            model_used: new_draft.model_used,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn latest_draft_for_thread(&self, thread_id: &str) -> Result<Option<DraftRecord>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT id, thread_id, body, status, source_summary, model_used, created_at, updated_at
                FROM drafts
                WHERE thread_id = ?1
                ORDER BY created_at DESC
                LIMIT 1
                ",
                params![thread_id],
                |row| self.row_to_draft_record(row),
            )
            .optional()
            .context("failed querying draft")?;
        Ok(row)
    }

    pub fn pending_drafts(&self, limit: usize) -> Result<Vec<DraftRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "
            SELECT id, thread_id, body, status, source_summary, model_used, created_at, updated_at
            FROM drafts
            WHERE status = 'pending'
            ORDER BY created_at DESC
            LIMIT ?1
            ",
        )?;
        let mut out = Vec::new();
        let rows = stmt.query_map(params![limit as i64], |row| self.row_to_draft_record(row))?;
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn update_draft_status(
        &self,
        draft_id: &str,
        status: DraftStatus,
        gmail_message_id: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "
            UPDATE drafts
            SET status = ?2,
                gmail_message_id = COALESCE(?3, gmail_message_id),
                updated_at = ?4
            WHERE id = ?1
            ",
            params![
                draft_id,
                status.as_str(),
                gmail_message_id,
                Utc::now().timestamp(),
            ],
        )
        .context("failed updating draft status")?;
        Ok(())
    }

    pub fn draft_by_thread(&self, thread_id: &str) -> Result<Option<DraftRecord>> {
        self.latest_draft_for_thread(thread_id)
    }

    pub fn append_event(
        &self,
        thread_id: Option<&str>,
        event_type: &str,
        payload: &Value,
    ) -> Result<EventRecord> {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let payload_json =
            serde_json::to_string(payload).context("failed serializing event payload")?;
        let conn = self.conn.lock();
        conn.execute(
            "
            INSERT INTO events(id, thread_id, event_type, payload_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![id, thread_id, event_type, payload_json, now.timestamp()],
        )
        .context("failed appending event")?;

        Ok(EventRecord {
            id,
            thread_id: thread_id.map(ToString::to_string),
            event_type: event_type.to_string(),
            payload: payload.clone(),
            created_at: now,
        })
    }

    pub fn events(&self, thread_id: Option<&str>, limit: usize) -> Result<Vec<EventRecord>> {
        let conn = self.conn.lock();
        let limit = limit.clamp(1, 1000) as i64;
        let mut events = Vec::new();

        if let Some(thread) = thread_id {
            let mut stmt = conn.prepare(
                "
                SELECT id, thread_id, event_type, payload_json, created_at
                FROM events
                WHERE thread_id = ?1
                ORDER BY created_at DESC
                LIMIT ?2
                ",
            )?;
            let rows = stmt.query_map(params![thread, limit], row_to_event)?;
            for row in rows {
                events.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "
                SELECT id, thread_id, event_type, payload_json, created_at
                FROM events
                ORDER BY created_at DESC
                LIMIT ?1
                ",
            )?;
            let rows = stmt.query_map(params![limit], row_to_event)?;
            for row in rows {
                events.push(row?);
            }
        }

        events.reverse();
        Ok(events)
    }

    pub fn similar_threads(
        &self,
        thread_id: &str,
        category: ThreadCategory,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "
            SELECT id
            FROM threads
            WHERE id != ?1 AND category = ?2
            ORDER BY last_message_at DESC
            LIMIT 3
            ",
        )?;

        let mut out = Vec::new();
        let rows = stmt.query_map(params![thread_id, category.as_str()], |row| row.get(0))?;
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn draft_quality_samples(
        &self,
        categories: &[ThreadCategory],
        limit_per_category: usize,
    ) -> Result<Vec<DraftQualitySample>> {
        if categories.is_empty() {
            return Ok(Vec::new());
        }

        let limit_per_category = limit_per_category.clamp(1, 500) as i64;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "
            SELECT
                t.id,
                d.body,
                m.body
            FROM threads t
            JOIN drafts d ON d.id = (
                SELECT d2.id
                FROM drafts d2
                WHERE d2.thread_id = t.id
                ORDER BY d2.created_at DESC
                LIMIT 1
            )
            JOIN messages m ON m.id = (
                SELECT m2.id
                FROM messages m2
                WHERE m2.thread_id = t.id
                  AND m2.inbound = 0
                ORDER BY m2.sent_at DESC
                LIMIT 1
            )
            WHERE t.category = ?1
            ORDER BY t.last_message_at DESC
            LIMIT ?2
            ",
        )?;

        let mut out = Vec::new();
        for category in categories {
            let rows = stmt.query_map(params![category.as_str(), limit_per_category], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;

            for row in rows {
                let (thread_id, draft_body, sent_body) = row?;
                out.push(DraftQualitySample {
                    thread_id,
                    category: *category,
                    generated_draft: self.decrypt_corpus_text(draft_body),
                    sent_reply: self.decrypt_corpus_text(sent_body),
                });
            }
        }

        Ok(out)
    }

    pub fn audit_for_thread(&self, thread_id: &str) -> Result<Option<AuditResponse>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT category, risk, policy, similar_thread_ids, external_model_used
                FROM threads
                WHERE id = ?1
                ",
                params![thread_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()
            .context("failed querying thread audit info")?;

        let Some((category_str, risk_str, policy_str, similar_json, external_model_used)) = row
        else {
            return Ok(None);
        };

        let category = category_str.as_deref().and_then(parse_category);
        let risk = risk_str.as_deref().and_then(parse_risk);
        let policy = policy_str.as_deref().and_then(parse_policy);
        let similar_thread_ids: Vec<String> = similar_json
            .as_deref()
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_default();
        let events = self.events(Some(thread_id), 500)?;

        Ok(Some(AuditResponse {
            category,
            risk,
            policy,
            similar_thread_ids,
            external_model_used: external_model_used == 1,
            events,
        }))
    }

    pub fn export_thread_events(
        &self,
        thread_id: &str,
        output_dir: PathBuf,
    ) -> Result<(PathBuf, usize)> {
        std::fs::create_dir_all(&output_dir).context("failed to create export directory")?;
        let events = self.events(Some(thread_id), 10_000)?;
        let mut out_path = output_dir;
        out_path.push(format!("audit-{thread_id}.json"));
        let payload =
            serde_json::to_string_pretty(&events).context("failed serializing event export")?;
        std::fs::write(&out_path, payload).context("failed writing event export file")?;
        Ok((out_path, events.len()))
    }

    pub fn settings(&self) -> Result<SettingsResponse> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut settings = std::collections::HashMap::<String, String>::new();
        for row in rows {
            let (key, value) = row?;
            settings.insert(key, value);
        }

        let privacy_mode = settings
            .get("privacy_mode")
            .and_then(|v| parse_privacy_mode(v))
            .unwrap_or_default();

        let backfill_days = settings
            .get("backfill_days")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(90);

        let allowed_recipient_domains = settings
            .get("allowed_recipient_domains")
            .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok())
            .unwrap_or_default();

        let attachment_storage_mode = settings
            .get("attachment_storage_mode")
            .and_then(|v| parse_attachment_storage_mode(v))
            .unwrap_or_default();

        let signature = settings.get("signature").cloned().filter(|v| !v.is_empty());
        let template_scheduling = settings
            .get("template_scheduling")
            .cloned()
            .filter(|v| !v.is_empty());
        let template_report_delivery = settings
            .get("template_report_delivery")
            .cloned()
            .filter(|v| !v.is_empty());

        let sync_interval_seconds = settings
            .get("sync_interval_seconds")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);

        Ok(SettingsResponse {
            privacy_mode,
            backfill_days,
            allowed_recipient_domains,
            attachment_storage_mode,
            signature,
            template_scheduling,
            template_report_delivery,
            sync_interval_seconds,
        })
    }

    pub fn update_settings(&self, settings: &SettingsResponse) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('privacy_mode', ?1)",
            params![privacy_mode_to_str(settings.privacy_mode)],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('backfill_days', ?1)",
            params![settings.backfill_days.to_string()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('allowed_recipient_domains', ?1)",
            params![serde_json::to_string(&settings.allowed_recipient_domains)?],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('attachment_storage_mode', ?1)",
            params![attachment_storage_mode_to_str(
                settings.attachment_storage_mode
            )],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('signature', ?1)",
            params![settings.signature.clone().unwrap_or_default()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('template_scheduling', ?1)",
            params![settings.template_scheduling.clone().unwrap_or_default()],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('template_report_delivery', ?1)",
            params![
                settings
                    .template_report_delivery
                    .clone()
                    .unwrap_or_default()
            ],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO settings(key, value) VALUES ('sync_interval_seconds', ?1)",
            params![settings.sync_interval_seconds.to_string()],
        )?;
        Ok(())
    }

    pub fn mark_draft_needs_human(&self, draft_id: &str) -> Result<()> {
        self.update_draft_status(draft_id, DraftStatus::NeedsHuman, None)
    }

    pub fn latest_thread_subject_and_recipient(
        &self,
        thread_id: &str,
    ) -> Result<Option<(String, String, String)>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT m.subject, m.sender, m.recipient
                FROM messages m
                WHERE m.thread_id = ?1 AND m.inbound = 1
                ORDER BY m.sent_at DESC
                LIMIT 1
                ",
                params![thread_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .context("failed querying message subject/recipient")?;
        Ok(row)
    }

    pub fn get_pending_draft_for_thread(&self, thread_id: &str) -> Result<Option<DraftRecord>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "
                SELECT id, thread_id, body, status, source_summary, model_used, created_at, updated_at
                FROM drafts
                WHERE thread_id = ?1 AND status IN ('pending', 'approved')
                ORDER BY created_at DESC
                LIMIT 1
                ",
                params![thread_id],
                |row| self.row_to_draft_record(row),
            )
            .optional()
            .context("failed querying pending draft")?;
        Ok(row)
    }

    pub fn set_draft_as_sent(&self, draft_id: &str, gmail_message_id: &str) -> Result<()> {
        self.update_draft_status(draft_id, DraftStatus::Sent, Some(gmail_message_id))
    }

    fn decrypt_corpus_text(&self, value: String) -> String {
        self.vault.decrypt(&value).unwrap_or(value)
    }

    fn row_to_draft_record(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<DraftRecord> {
        let encrypted_body: String = row.get(2)?;
        Ok(DraftRecord {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            body: self.decrypt_corpus_text(encrypted_body),
            status: parse_draft_status(row.get::<_, String>(3)?.as_str())
                .unwrap_or(DraftStatus::Pending),
            source_summary: row.get(4)?,
            model_used: row.get(5)?,
            created_at: ts_to_datetime(row.get(6)?),
            updated_at: ts_to_datetime(row.get(7)?),
        })
    }

    pub fn mine_template_suggestions(&self, limit: usize) -> Result<Vec<TemplateSuggestion>> {
        let messages = self.recent_sent_messages(300)?;
        let mut counts: std::collections::HashMap<(ThreadCategory, String), usize> =
            std::collections::HashMap::new();

        for message in messages {
            let normalized = normalize_template_candidate(&message.body);
            if normalized.len() < 40 {
                continue;
            }

            let category = infer_template_category(&normalized);
            *counts.entry((category, normalized)).or_insert(0) += 1;
        }

        let mut suggestions: Vec<TemplateSuggestion> = counts
            .into_iter()
            .map(
                |((category, template_text), occurrences)| TemplateSuggestion {
                    id: Uuid::new_v4().to_string(),
                    category,
                    template_text,
                    occurrences,
                },
            )
            .filter(|item| item.occurrences >= 2)
            .collect();

        suggestions.sort_by(|left, right| {
            right
                .occurrences
                .cmp(&left.occurrences)
                .then_with(|| left.template_text.len().cmp(&right.template_text.len()))
        });
        suggestions.truncate(limit.clamp(1, 50));
        Ok(suggestions)
    }

    pub fn delete_local_corpus(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM drafts", [])?;
        conn.execute("DELETE FROM messages", [])?;
        conn.execute("DELETE FROM threads", [])?;
        conn.execute("DELETE FROM events", [])?;
        Ok(())
    }

    pub fn factory_reset(&self) -> Result<()> {
        self.delete_local_corpus()?;
        let conn = self.conn.lock();
        conn.execute("DELETE FROM oauth_tokens", [])?;
        conn.execute("DELETE FROM settings", [])?;
        drop(conn);
        self.ensure_default_settings()?;
        Ok(())
    }
}

fn normalize_template_candidate(body: &str) -> String {
    let mut out = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('>'))
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");

    if let Some(idx) = out.find("Reference:") {
        out.truncate(idx);
    }

    if out.len() > 420 {
        out.truncate(420);
    }

    out
}

fn infer_template_category(text: &str) -> ThreadCategory {
    let normalized = text.to_lowercase();
    if normalized.contains("schedule")
        || normalized.contains("availability")
        || normalized.contains("calendar")
    {
        return ThreadCategory::Scheduling;
    }
    if normalized.contains("report")
        || normalized.contains("findings")
        || normalized.contains("attached")
    {
        return ThreadCategory::ReportDelivery;
    }
    if normalized.contains("price")
        || normalized.contains("quote")
        || normalized.contains("invoice")
    {
        return ThreadCategory::Pricing;
    }
    ThreadCategory::Other
}

fn row_to_thread_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadSummary> {
    let category = row
        .get::<_, Option<String>>(4)?
        .as_deref()
        .and_then(parse_category);
    let risk = row
        .get::<_, Option<String>>(5)?
        .as_deref()
        .and_then(parse_risk);
    let policy = row
        .get::<_, Option<String>>(6)?
        .as_deref()
        .and_then(parse_policy);

    Ok(ThreadSummary {
        id: row.get(0)?,
        subject: row.get(1)?,
        snippet: row.get(2)?,
        from_address: row.get(3)?,
        category,
        risk,
        policy,
        last_message_at: ts_to_datetime(row.get(7)?),
        has_pending_draft: row.get::<_, i64>(8)? == 1,
    })
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRecord> {
    let payload_raw: String = row.get(3)?;
    let payload: Value = serde_json::from_str(&payload_raw).unwrap_or(Value::Null);
    Ok(EventRecord {
        id: row.get(0)?,
        thread_id: row.get(1)?,
        event_type: row.get(2)?,
        payload,
        created_at: ts_to_datetime(row.get(4)?),
    })
}

pub fn ts_to_datetime(ts: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now)
}

pub fn parse_category(raw: &str) -> Option<ThreadCategory> {
    match raw {
        "scheduling" => Some(ThreadCategory::Scheduling),
        "report_delivery" => Some(ThreadCategory::ReportDelivery),
        "findings_clarification" => Some(ThreadCategory::FindingsClarification),
        "pricing" => Some(ThreadCategory::Pricing),
        "complaint_dispute" => Some(ThreadCategory::ComplaintDispute),
        "legal_insurance" => Some(ThreadCategory::LegalInsurance),
        "other" => Some(ThreadCategory::Other),
        _ => None,
    }
}

pub fn parse_risk(raw: &str) -> Option<RiskTier> {
    match raw {
        "low" => Some(RiskTier::Low),
        "medium" => Some(RiskTier::Medium),
        "high" => Some(RiskTier::High),
        _ => None,
    }
}

pub fn risk_to_str(risk: RiskTier) -> &'static str {
    match risk {
        RiskTier::Low => "low",
        RiskTier::Medium => "medium",
        RiskTier::High => "high",
    }
}

pub fn parse_policy(raw: &str) -> Option<PolicyDecision> {
    match raw {
        "draft_only" => Some(PolicyDecision::DraftOnly),
        "send_with_approval" => Some(PolicyDecision::SendWithApproval),
        "blocked" => Some(PolicyDecision::Blocked),
        _ => None,
    }
}

pub fn parse_draft_status(raw: &str) -> Option<DraftStatus> {
    match raw {
        "pending" => Some(DraftStatus::Pending),
        "approved" => Some(DraftStatus::Approved),
        "rejected" => Some(DraftStatus::Rejected),
        "needs_human" => Some(DraftStatus::NeedsHuman),
        "sent" => Some(DraftStatus::Sent),
        _ => None,
    }
}

pub fn parse_privacy_mode(raw: &str) -> Option<PrivacyMode> {
    match raw {
        "local_only" => Some(PrivacyMode::LocalOnly),
        "hybrid" => Some(PrivacyMode::Hybrid),
        "cloud" => Some(PrivacyMode::Cloud),
        _ => None,
    }
}

pub fn privacy_mode_to_str(mode: PrivacyMode) -> &'static str {
    match mode {
        PrivacyMode::LocalOnly => "local_only",
        PrivacyMode::Hybrid => "hybrid",
        PrivacyMode::Cloud => "cloud",
    }
}

pub fn parse_attachment_storage_mode(raw: &str) -> Option<AttachmentStorageMode> {
    match raw {
        "none" => Some(AttachmentStorageMode::None),
        "metadata" => Some(AttachmentStorageMode::Metadata),
        "full" => Some(AttachmentStorageMode::Full),
        _ => None,
    }
}

pub fn attachment_storage_mode_to_str(mode: AttachmentStorageMode) -> &'static str {
    match mode {
        AttachmentStorageMode::None => "none",
        AttachmentStorageMode::Metadata => "metadata",
        AttachmentStorageMode::Full => "full",
    }
}
