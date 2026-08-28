use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

pub struct Store {
    conn: Mutex<Connection>,
}

#[derive(Clone, Debug)]
pub struct ThreadRow {
    pub id: String,
    pub owner: String,
    pub status: String,
    pub objective: String,
    pub lane: String,
    pub model: Option<String>,
    pub generation: i64,
}

#[derive(Clone, Debug)]
pub struct GrantRow {
    pub thread_id: String,
    pub token_digest: String,
    pub model: String,
    pub generation: i64,
    pub status: String,
    pub call_count: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS threads (
              id TEXT PRIMARY KEY,
              owner TEXT NOT NULL,
              status TEXT NOT NULL,
              objective TEXT NOT NULL,
              lane TEXT NOT NULL,
              model TEXT,
              generation INTEGER NOT NULL DEFAULT 1,
              event_count INTEGER NOT NULL DEFAULT 0,
              report TEXT,
              error_code TEXT,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS grants (
              id TEXT PRIMARY KEY,
              thread_id TEXT NOT NULL,
              token_digest TEXT NOT NULL UNIQUE,
              model TEXT NOT NULL,
              generation INTEGER NOT NULL,
              status TEXT NOT NULL,
              call_count INTEGER NOT NULL DEFAULT 0,
              prompt_tokens INTEGER NOT NULL DEFAULT 0,
              completion_tokens INTEGER NOT NULL DEFAULT 0,
              total_tokens INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              thread_id TEXT NOT NULL,
              event_type TEXT NOT NULL,
              payload TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_thread(
        &self,
        owner: &str,
        objective: &str,
        lane: &str,
        model: Option<&str>,
    ) -> rusqlite::Result<ThreadRow> {
        let id = format!("th_{}", uuid::Uuid::new_v4().simple());
        let now = now_stamp();
        let conn = self.conn.lock().expect("store");
        conn.execute(
            "INSERT INTO threads (id, owner, status, objective, lane, model, generation, created_at)
             VALUES (?1, ?2, 'open', ?3, ?4, ?5, 1, ?6)",
            params![id, owner, objective, lane, model, now],
        )?;
        Ok(ThreadRow {
            id,
            owner: owner.to_string(),
            status: "open".into(),
            objective: objective.to_string(),
            lane: lane.to_string(),
            model: model.map(str::to_string),
            generation: 1,
        })
    }

    pub fn mint_grant(
        &self,
        thread: &ThreadRow,
        model: &str,
        token: &str,
    ) -> rusqlite::Result<GrantRow> {
        let digest = token_digest(token);
        let now = now_stamp();
        let id = format!("gr_{}", uuid::Uuid::new_v4().simple());
        let conn = self.conn.lock().expect("store");
        conn.execute(
            "UPDATE grants SET status = 'revoked' WHERE thread_id = ?1 AND status = 'active'",
            params![thread.id],
        )?;
        conn.execute(
            "INSERT INTO grants (id, thread_id, token_digest, model, generation, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)",
            params![id, thread.id, digest, model, thread.generation, now],
        )?;
        Ok(GrantRow {
            thread_id: thread.id.clone(),
            token_digest: digest,
            model: model.to_string(),
            generation: thread.generation,
            status: "active".into(),
            call_count: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        })
    }

    pub fn bump_and_mint(
        &self,
        thread_id: &str,
        owner: &str,
        token: &str,
    ) -> rusqlite::Result<Option<(ThreadRow, GrantRow)>> {
        let mut thread = match self.get_thread(thread_id, owner)? {
            Some(thread) if thread.status == "open" || thread.status == "succeeded" => thread,
            _ => return Ok(None),
        };
        {
            let conn = self.conn.lock().expect("store");
            conn.execute(
                "UPDATE threads SET generation = generation + 1, status = 'open' WHERE id = ?1",
                params![thread.id],
            )?;
            conn.execute(
                "UPDATE grants SET status = 'revoked' WHERE thread_id = ?1 AND status = 'active'",
                params![thread.id],
            )?;
        }
        thread.generation += 1;
        thread.status = "open".into();
        let model = thread
            .model
            .clone()
            .unwrap_or_else(|| "glm-5.3-flash".into());
        let grant = self.mint_grant(&thread, &model, token)?;
        Ok(Some((thread, grant)))
    }

    pub fn list_threads(&self, owner: &str) -> rusqlite::Result<Vec<ThreadRow>> {
        let conn = self.conn.lock().expect("store");
        let mut stmt = conn.prepare(
            "SELECT id, owner, status, objective, lane, model, generation FROM threads
             WHERE owner = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![owner], |row| {
            Ok(ThreadRow {
                id: row.get(0)?,
                owner: row.get(1)?,
                status: row.get(2)?,
                objective: row.get(3)?,
                lane: row.get(4)?,
                model: row.get(5)?,
                generation: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_thread(&self, id: &str, owner: &str) -> rusqlite::Result<Option<ThreadRow>> {
        let conn = self.conn.lock().expect("store");
        conn.query_row(
            "SELECT id, owner, status, objective, lane, model, generation FROM threads
             WHERE id = ?1 AND owner = ?2",
            params![id, owner],
            |row| {
                Ok(ThreadRow {
                    id: row.get(0)?,
                    owner: row.get(1)?,
                    status: row.get(2)?,
                    objective: row.get(3)?,
                    lane: row.get(4)?,
                    model: row.get(5)?,
                    generation: row.get(6)?,
                })
            },
        )
        .optional()
    }

    pub fn grant_by_token(&self, token: &str) -> rusqlite::Result<Option<GrantRow>> {
        let digest = token_digest(token);
        let conn = self.conn.lock().expect("store");
        conn.query_row(
            "SELECT thread_id, token_digest, model, generation, status, call_count,
                    prompt_tokens, completion_tokens, total_tokens
             FROM grants WHERE token_digest = ?1",
            params![digest],
            |row| {
                Ok(GrantRow {
                    thread_id: row.get(0)?,
                    token_digest: row.get(1)?,
                    model: row.get(2)?,
                    generation: row.get(3)?,
                    status: row.get(4)?,
                    call_count: row.get(5)?,
                    prompt_tokens: row.get(6)?,
                    completion_tokens: row.get(7)?,
                    total_tokens: row.get(8)?,
                })
            },
        )
        .optional()
    }

    pub fn record_usage(
        &self,
        digest: &str,
        prompt: i64,
        completion: i64,
        total: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("store");
        conn.execute(
            "UPDATE grants SET call_count = call_count + 1,
                 prompt_tokens = prompt_tokens + ?2,
                 completion_tokens = completion_tokens + ?3,
                 total_tokens = total_tokens + ?4
             WHERE token_digest = ?1",
            params![digest, prompt, completion, total],
        )?;
        Ok(())
    }

    pub fn append_events(
        &self,
        thread_id: &str,
        events: &[(String, serde_json::Value)],
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("store");
        for (event_type, payload) in events {
            conn.execute(
                "INSERT INTO events (thread_id, event_type, payload) VALUES (?1, ?2, ?3)",
                params![thread_id, event_type, payload.to_string()],
            )?;
        }
        conn.execute(
            "UPDATE threads SET event_count = event_count + ?2 WHERE id = ?1",
            params![thread_id, events.len() as i64],
        )?;
        Ok(())
    }

    pub fn list_events(
        &self,
        thread_id: &str,
    ) -> rusqlite::Result<Vec<(String, serde_json::Value)>> {
        let conn = self.conn.lock().expect("store");
        let mut stmt = conn.prepare(
            "SELECT event_type, payload FROM events WHERE thread_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            let event_type: String = row.get(0)?;
            let payload: String = row.get(1)?;
            let value = serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
            Ok((event_type, value))
        })?;
        rows.collect()
    }

    pub fn finish(
        &self,
        thread_id: &str,
        status: &str,
        report: &str,
        error_code: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().expect("store");
        conn.execute(
            "UPDATE threads SET status = ?2, report = ?3, error_code = ?4 WHERE id = ?1",
            params![thread_id, status, report, error_code],
        )?;
        conn.execute(
            "UPDATE grants SET status = 'revoked' WHERE thread_id = ?1 AND status = 'active'",
            params![thread_id],
        )?;
        Ok(())
    }

    pub fn credit(&self, owner: &str, _allowance: i64) -> rusqlite::Result<(i64, u64, bool)> {
        let conn = self.conn.lock().expect("store");
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(g.total_tokens), 0) FROM grants g
             JOIN threads t ON t.id = g.thread_id WHERE t.owner = ?1",
            params![owner],
            |row| row.get(0),
        )?;
        Ok((total, 0, true))
    }
}

pub fn token_digest(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn mint_token() -> String {
    format!("sig_{}", hex::encode(uuid::Uuid::new_v4().as_bytes()))
}

fn now_stamp() -> String {
    chrono::Utc::now().to_rfc3339()
}
