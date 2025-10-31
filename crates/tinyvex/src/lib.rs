use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use tracing::info;
use ts_rs::TS;

pub struct Tinyvex {
    db_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export, export_to = "../../docs/types/")]
pub struct ThreadRow {
    pub id: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub project_id: Option<String>,
    pub resume_id: Option<String>,
    pub rollout_path: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "messageCount", skip_serializing_if = "Option::is_none")]
    pub message_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_ts: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export, export_to = "../../docs/types/")]
pub struct MessageRow {
    pub id: i64,
    pub thread_id: String,
    pub role: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub item_id: Option<String>,
    pub partial: Option<i64>,
    pub seq: Option<i64>,
    pub ts: i64,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export, export_to = "../../docs/types/")]
pub struct ToolCallRow {
    pub thread_id: String,
    pub tool_call_id: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub content_json: Option<String>,
    pub locations_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Tinyvex {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        let tvx = Self { db_path };
        tvx.init_schema()?;
        info!("tinyvex ready");
        Ok(tvx)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        // Apply recommended SQLite pragmas for local, single-writer usage.
        // journal_mode=WAL persists at the DB level; synchronous governs durability
        // vs performance. busy_timeout helps avoid transient contention.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
        // threads
        conn.execute_batch(
            r#"
        CREATE TABLE IF NOT EXISTS threads (
          id TEXT PRIMARY KEY,
          threadId TEXT,
          title TEXT NOT NULL,
          projectId TEXT,
          resumeId TEXT,
          rolloutPath TEXT,
          source TEXT,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_threads_updated ON threads(updatedAt DESC);
        "#,
        )?;
        // messages
        conn.execute_batch(
            r#"
        CREATE TABLE IF NOT EXISTS messages (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          threadId TEXT NOT NULL,
          role TEXT,
          kind TEXT NOT NULL,
          text TEXT,
          data TEXT,
          itemId TEXT,
          partial INTEGER,
          seq INTEGER,
          ts INTEGER NOT NULL,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_msgs_thread_ts ON messages(threadId, ts);
        CREATE INDEX IF NOT EXISTS idx_msgs_thread_item ON messages(threadId, itemId);
        CREATE UNIQUE INDEX IF NOT EXISTS uniq_msgs_thread_item ON messages(threadId, itemId);

        -- Unified ACP event log (append-only)
        CREATE TABLE IF NOT EXISTS acp_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sessionId TEXT,
          clientThreadDocId TEXT,
          ts INTEGER NOT NULL,
          seq INTEGER,
          updateKind TEXT NOT NULL,
          role TEXT,
          text TEXT,
          toolCallId TEXT,
          status TEXT,
          kind TEXT,
          content_json TEXT,
          locations_json TEXT,
          raw_json TEXT,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_acp_events_thread_ts ON acp_events(clientThreadDocId, ts);
        CREATE INDEX IF NOT EXISTS idx_acp_events_session_ts ON acp_events(sessionId, ts);
        CREATE INDEX IF NOT EXISTS idx_acp_events_kind_thread ON acp_events(updateKind, clientThreadDocId, ts DESC);

        -- ACP: tool calls (store content/locations as JSON strings)
        CREATE TABLE IF NOT EXISTS acp_tool_calls (
          threadId TEXT NOT NULL,
          toolCallId TEXT NOT NULL,
          title TEXT,
          kind TEXT,
          status TEXT,
          content_json TEXT,
          locations_json TEXT,
          -- Canonical column names aligned with Convex schema; store JSON text
          content TEXT,
          locations TEXT,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER NOT NULL,
          PRIMARY KEY (threadId, toolCallId)
        );
        CREATE INDEX IF NOT EXISTS idx_acp_tool_calls_thread_tool ON acp_tool_calls(threadId, toolCallId);
        CREATE INDEX IF NOT EXISTS idx_acp_tool_calls_thread_updated ON acp_tool_calls(threadId, updatedAt);

        -- ACP: plan (entries array as JSON)
        CREATE TABLE IF NOT EXISTS acp_plan (
          threadId TEXT PRIMARY KEY,
          entries_json TEXT,
          -- Canonical name per Convex schema; store JSON text
          entries TEXT,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_acp_plan_thread ON acp_plan(threadId);
        CREATE INDEX IF NOT EXISTS idx_acp_plan_thread_updated ON acp_plan(threadId, updatedAt);

        -- ACP: state (current mode id and available commands as JSON)
        CREATE TABLE IF NOT EXISTS acp_state (
          threadId TEXT PRIMARY KEY,
          currentModeId TEXT,
          available_commands_json TEXT,
          -- Canonical name per Convex schema; store JSON text
          available_commands TEXT,
          createdAt INTEGER NOT NULL,
          updatedAt INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_acp_state_thread ON acp_state(threadId);
        CREATE INDEX IF NOT EXISTS idx_acp_state_thread_updated ON acp_state(threadId, updatedAt);
        "#,
        )?;
        Ok(())
    }

    pub fn upsert_thread(&self, row: &ThreadRow) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        // last-write-wins by updatedAt
        conn.execute(
            r#"
            INSERT INTO threads (id, threadId, title, projectId, resumeId, rolloutPath, source, createdAt, updatedAt)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
              threadId=excluded.threadId,
              title=excluded.title,
              projectId=excluded.projectId,
              resumeId=excluded.resumeId,
              rolloutPath=excluded.rolloutPath,
              source=excluded.source,
              updatedAt=excluded.updatedAt
            WHERE excluded.updatedAt >= threads.updatedAt
            "#,
            params![
                row.id,
                row.thread_id,
                row.title,
                row.project_id,
                row.resume_id,
                row.rollout_path,
                row.source,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn list_threads(&self, limit: i64) -> Result<Vec<ThreadRow>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT \
                id, threadId, title, projectId, resumeId, rolloutPath, source, createdAt, updatedAt, \
                (SELECT COUNT(*) FROM messages m WHERE m.threadId = threads.id) AS messageCount, \
                (SELECT MAX(ts) FROM messages m2 WHERE m2.threadId = threads.id) AS lastTs \
             FROM threads \
             ORDER BY updatedAt DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(ThreadRow {
                    id: r.get(0)?,
                    thread_id: r.get(1)?,
                    title: r.get(2)?,
                    project_id: r.get(3)?,
                    resume_id: r.get(4)?,
                    rollout_path: r.get(5)?,
                    source: r.get(6)?,
                    created_at: r.get(7)?,
                    updated_at: r.get(8)?,
                    message_count: r.get(9).ok(),
                    last_message_ts: r.get(10).ok(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_messages(&self, thread_id: &str, limit: i64) -> Result<Vec<MessageRow>> {
        let conn = Connection::open(&self.db_path)?;
        // Return the most recent `limit` messages in ascending order by ts
        let mut stmt = conn.prepare(
            "SELECT id, threadId, role, kind, text, itemId, partial, seq, ts, createdAt, updatedAt \
             FROM (
               SELECT id, threadId, role, kind, text, itemId, partial, seq, ts, createdAt, updatedAt \
               FROM messages WHERE threadId=?1 ORDER BY ts DESC LIMIT ?2
             ) sub ORDER BY ts ASC",
        )?;
        let rows = stmt
            .query_map(params![thread_id, limit], |r| {
                Ok(MessageRow {
                    id: r.get(0)?,
                    thread_id: r.get(1)?,
                    role: r.get(2)?,
                    kind: r.get(3)?,
                    text: r.get(4)?,
                    item_id: r.get(5)?,
                    partial: r.get(6)?,
                    seq: r.get(7)?,
                    ts: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_tool_calls(&self, thread_id: &str, limit: i64) -> Result<Vec<ToolCallRow>> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT threadId, toolCallId, title, kind, status, content_json, locations_json, createdAt, updatedAt \
             FROM acp_tool_calls WHERE threadId=?1 ORDER BY updatedAt DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![thread_id, limit], |r| {
                Ok(ToolCallRow {
                    thread_id: r.get(0)?,
                    tool_call_id: r.get(1)?,
                    title: r.get(2).ok(),
                    kind: r.get(3).ok(),
                    status: r.get(4).ok(),
                    content_json: r.get(5).ok(),
                    locations_json: r.get(6).ok(),
                    created_at: r.get(7)?,
                    updated_at: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn upsert_streamed_message(
        &self,
        thread_id: &str,
        kind: &str,
        role: Option<&str>,
        text: &str,
        item_id: &str,
        seq: i64,
        ts: i64,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            r#"
            INSERT INTO messages (threadId, role, kind, text, data, itemId, partial, seq, ts, createdAt, updatedAt)
            VALUES (?1, ?2, ?3, ?4, NULL, ?5, 1, ?6, ?7, ?7, ?7)
            ON CONFLICT(threadId, itemId) DO UPDATE SET
              text=excluded.text,
              seq=excluded.seq,
              partial=1,
              updatedAt=excluded.updatedAt
            "#,
            params![thread_id, role, kind, text, item_id, seq, ts],
        )?;
        Ok(())
    }

    pub fn finalize_streamed_message(
        &self,
        thread_id: &str,
        item_id: &str,
        text: &str,
        ts: i64,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        // Try update; if no row, insert finalized
        let n = conn.execute(
            r#"UPDATE messages
                SET text=?3, partial=0, updatedAt=?4
              WHERE threadId=?1 AND itemId=?2"#,
            params![thread_id, item_id, text, ts],
        )?;
        if n == 0 {
            conn.execute(
                r#"INSERT INTO messages (threadId, role, kind, text, data, itemId, partial, seq, ts, createdAt, updatedAt)
                    VALUES (?1, NULL, 'message', ?3, NULL, ?2, 0, 0, ?4, ?4, ?4)"#,
                params![thread_id, item_id, text, ts],
            )?;
        }
        Ok(())
    }

    pub fn upsert_acp_tool_call(
        &self,
        thread_id: &str,
        tool_call_id: &str,
        title: Option<&str>,
        kind: Option<&str>,
        status: Option<&str>,
        content_json: Option<&str>,
        locations_json: Option<&str>,
        ts: i64,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            r#"
            INSERT INTO acp_tool_calls (threadId, toolCallId, title, kind, status, content_json, locations_json, content, locations, createdAt, updatedAt)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?6, ?7, ?8, ?8)
            ON CONFLICT(threadId, toolCallId) DO UPDATE SET
              title=COALESCE(excluded.title, acp_tool_calls.title),
              kind=COALESCE(excluded.kind, acp_tool_calls.kind),
              status=COALESCE(excluded.status, acp_tool_calls.status),
              content_json=COALESCE(excluded.content_json, acp_tool_calls.content_json),
              locations_json=COALESCE(excluded.locations_json, acp_tool_calls.locations_json),
              content=COALESCE(excluded.content, acp_tool_calls.content),
              locations=COALESCE(excluded.locations, acp_tool_calls.locations),
              updatedAt=excluded.updatedAt
            "#,
            params![thread_id, tool_call_id, title, kind, status, content_json, locations_json, ts],
        )?;
        Ok(())
    }

    pub fn upsert_acp_plan(&self, thread_id: &str, entries_json: &str, ts: i64) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            r#"
            INSERT INTO acp_plan (threadId, entries_json, entries, createdAt, updatedAt)
            VALUES (?1, ?2, ?2, ?3, ?3)
            ON CONFLICT(threadId) DO UPDATE SET entries_json=excluded.entries_json, entries=excluded.entries, updatedAt=excluded.updatedAt
            "#,
            params![thread_id, entries_json, ts],
        )?;
        Ok(())
    }

    pub fn upsert_acp_state(
        &self,
        thread_id: &str,
        current_mode_id: Option<&str>,
        available_commands_json: Option<&str>,
        ts: i64,
    ) -> Result<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            r#"
            INSERT INTO acp_state (threadId, currentModeId, available_commands_json, available_commands, createdAt, updatedAt)
            VALUES (?1, ?2, ?3, ?3, ?4, ?4)
            ON CONFLICT(threadId) DO UPDATE SET
              currentModeId = COALESCE(excluded.currentModeId, acp_state.currentModeId),
              available_commands_json = COALESCE(excluded.available_commands_json, acp_state.available_commands_json),
              available_commands = COALESCE(excluded.available_commands, acp_state.available_commands),
              updatedAt = excluded.updatedAt
            "#,
            params![thread_id, current_mode_id, available_commands_json, ts],
        )?;
        Ok(())
    }

    pub fn insert_acp_event(
        &self,
        session_id: Option<&str>,
        client_thread_doc_id: Option<&str>,
        ts: i64,
        seq: Option<i64>,
        update_kind: &str,
        role: Option<&str>,
        text: Option<&str>,
        tool_call_id: Option<&str>,
        status: Option<&str>,
        kind: Option<&str>,
        content_json: Option<&str>,
        locations_json: Option<&str>,
        raw_json: Option<&str>,
    ) -> Result<i64> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            r#"
            INSERT INTO acp_events (sessionId, clientThreadDocId, ts, seq, updateKind, role, text, toolCallId, status, kind, content_json, locations_json, raw_json, createdAt, updatedAt)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?3, ?3)
            "#,
            params![
                session_id,
                client_thread_doc_id,
                ts,
                seq,
                update_kind,
                role,
                text,
                tool_call_id,
                status,
                kind,
                content_json,
                locations_json,
                raw_json,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ms() -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
    }

    #[test]
    fn tinyvex_upsert_and_list_threads() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let tvx = Tinyvex::open(tmp.path()).unwrap();
        let t0 = now_ms();
        let row = ThreadRow {
            id: "t1".into(),
            thread_id: Some("t1".into()),
            title: "Hello".into(),
            project_id: None,
            resume_id: None,
            rollout_path: None,
            source: Some("test".into()),
            created_at: t0,
            updated_at: t0,
            message_count: None,
            last_message_ts: None,
        };
        tvx.upsert_thread(&row).unwrap();
        let out = tvx.list_threads(10).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "t1");
        assert_eq!(out[0].title, "Hello");
    }

    #[test]
    fn list_tool_calls_returns_recent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let tvx = Tinyvex::open(tmp.path()).unwrap();
        let t = "t-tools";
        // Insert 3 tool calls with increasing timestamps
        for i in 0..3i64 {
            let ts = now_ms() + i;
            let id = format!("tc{}", i);
            let content = format!("[{{\"type\":\"text\",\"text\":\"#{}\"}}]", i);
            tvx.upsert_acp_tool_call(t, &id, Some("call"), Some("Execute"), Some("Completed"), Some(&content), Some("[]"), ts).unwrap();
        }
        let rows = tvx.list_tool_calls(t, 2).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0].tool_call_id == "tc2" || rows[0].tool_call_id == "tc1");
    }

    #[test]
    fn list_messages_returns_last_n_in_ascending_order() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let tvx = Tinyvex::open(tmp.path()).unwrap();
        let thread = "t-tail";
        // Insert 100 finalized messages with increasing timestamps
        for i in 0..100i64 {
            let ts = now_ms() + i;
            let item_id = format!("it{}", i);
            tvx.finalize_streamed_message(thread, &item_id, &format!("m{}", i), ts).unwrap();
        }
        let rows = tvx.list_messages(thread, 50).unwrap();
        assert_eq!(rows.len(), 50, "should return last 50 messages");
        // Ensure ascending by ts and that they correspond to items 50..99
        for (idx, row) in rows.iter().enumerate() {
            let expected = 50 + idx as i64;
            assert!(row.text.as_deref().unwrap_or("").contains(&format!("m{}", expected)), "expected m{}", expected);
        }
        assert!(rows.windows(2).all(|w| w[0].ts <= w[1].ts), "rows should be ascending by ts");
    }
}

pub mod writer;
pub use writer::{Writer, WriterNotification};
