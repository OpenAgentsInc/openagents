//! SQLite sink for FRLM trace events.
//!
//! Batches inserts and flushes on completion for efficient persistence.

use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::{FrlmError, Result};
use crate::trace::TraceEvent;

const TRACE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS trace_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    event_json TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_trace_events_run ON trace_events(run_id);
CREATE INDEX IF NOT EXISTS idx_trace_events_run_seq ON trace_events(run_id, seq);
"#;

/// SQLite writer for trace events.
pub struct TraceDbWriter {
    conn: Connection,
    buffer: Vec<TraceEvent>,
    next_seq: i64,
    flush_size: usize,
}

impl TraceDbWriter {
    /// Open a SQLite database at the given path and ensure the schema exists.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .map_err(|e| FrlmError::Internal(format!("trace db open failed: {}", e)))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| FrlmError::Internal(format!("trace db pragma failed: {}", e)))?;
        Self::ensure_schema(&conn)?;
        Ok(Self::with_connection(conn))
    }

    /// Create a writer from an existing connection.
    pub fn with_connection(conn: Connection) -> Self {
        Self {
            conn,
            buffer: Vec::new(),
            next_seq: 0,
            flush_size: 50,
        }
    }

    /// Ensure the trace schema exists.
    pub fn ensure_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(TRACE_SCHEMA)
            .map_err(|e| FrlmError::Internal(format!("trace db schema failed: {}", e)))?;
        Ok(())
    }

    /// Add a trace event to the buffer and flush if needed.
    pub fn push(&mut self, event: TraceEvent) -> Result<()> {
        let flush_now = matches!(event, TraceEvent::RunDone { .. });
        self.buffer.push(event);
        if self.buffer.len() >= self.flush_size || flush_now {
            self.flush()?;
        }
        Ok(())
    }

    /// Drain all events from the receiver, flushing at the end.
    pub fn drain(&mut self, rx: std::sync::mpsc::Receiver<TraceEvent>) -> Result<()> {
        while let Ok(event) = rx.recv() {
            self.push(event)?;
        }
        self.flush()
    }

    /// Flush buffered events in a single transaction.
    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let tx = self
            .conn
            .transaction()
            .map_err(|e| FrlmError::Internal(format!("trace db tx start failed: {}", e)))?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT INTO trace_events (run_id, seq, event_type, timestamp_ms, event_json)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(|e| FrlmError::Internal(format!("trace db prepare failed: {}", e)))?;

            for event in self.buffer.drain(..) {
                let event_json = serde_json::to_string(&event).map_err(|e| {
                    FrlmError::Internal(format!("trace event encode failed: {}", e))
                })?;
                let event_type = event_type(&event);
                let timestamp_ms = event.timestamp_ms();
                let run_id = event.run_id().to_string();
                let seq = self.next_seq;
                self.next_seq += 1;

                stmt.execute(params![run_id, seq, event_type, timestamp_ms, event_json])
                    .map_err(|e| FrlmError::Internal(format!("trace db insert failed: {}", e)))?;
            }
        }
        tx.commit()
            .map_err(|e| FrlmError::Internal(format!("trace db commit failed: {}", e)))?;
        Ok(())
    }
}

fn event_type(event: &TraceEvent) -> &'static str {
    match event {
        TraceEvent::RunInit { .. } => "run_init",
        TraceEvent::RunDone { .. } => "run_done",
        TraceEvent::EnvLoadFragment { .. } => "env_load_fragment",
        TraceEvent::EnvSelectFragments { .. } => "env_select_fragments",
        TraceEvent::SubQuerySubmit { .. } => "subquery_submit",
        TraceEvent::SubQueryExecute { .. } => "subquery_execute",
        TraceEvent::SubQueryReturn { .. } => "subquery_return",
        TraceEvent::SubQueryTimeout { .. } => "subquery_timeout",
        TraceEvent::VerifyRedundant { .. } => "verify_redundant",
        TraceEvent::VerifyObjective { .. } => "verify_objective",
        TraceEvent::BudgetReserve { .. } => "budget_reserve",
        TraceEvent::BudgetSettle { .. } => "budget_settle",
        TraceEvent::Aggregate { .. } => "aggregate",
        TraceEvent::FallbackLocal { .. } => "fallback_local",
    }
}
