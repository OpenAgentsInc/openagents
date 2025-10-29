use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use tracing::info;

pub struct Tinyvex {
    db_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        let mut stmt = conn
            .prepare("SELECT id, threadId, title, projectId, resumeId, rolloutPath, source, createdAt, updatedAt FROM threads ORDER BY updatedAt DESC LIMIT ?1")?;
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
        };
        tvx.upsert_thread(&row).unwrap();
        let out = tvx.list_threads(10).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "t1");
        assert_eq!(out[0].title, "Hello");
    }
}
