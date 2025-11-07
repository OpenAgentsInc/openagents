# SQLite Schema — Log‑Structured Storage

Goals

- Append‑only log with timestamped rows to support snapshot reads and reactive invalidation.
- Single writer (actor) with WAL mode for concurrency with readers.
 - Predictable snapshot validation for resume, with optional journal horizon.

Tables

documents

- Columns:
  - id BLOB NOT NULL
  - ts INTEGER NOT NULL
  - table_id BLOB NOT NULL
  - json_value TEXT NULL
  - deleted INTEGER NOT NULL DEFAULT 0
  - prev_ts INTEGER NULL
- Primary key: (ts, table_id, id)
- Index: documents_by_table_and_id (table_id, id, ts)
 - Optional: partial index on non‑deleted for common lookups.

indexes

- Columns:
  - index_id BLOB NOT NULL
  - ts INTEGER NOT NULL
  - key BLOB NOT NULL
  - deleted INTEGER NOT NULL DEFAULT 0
  - table_id BLOB NULL
  - document_id BLOB NULL
- Primary key: (index_id, key, ts)
 - Secondary index: indexes_by_ts (ts) if needed for retention jobs.

globals

- Columns:
  - key TEXT NOT NULL PRIMARY KEY
  - json_value TEXT NOT NULL

Creation SQL (idempotent)

```sql
CREATE TABLE IF NOT EXISTS documents (
  id BLOB NOT NULL,
  ts INTEGER NOT NULL,
  table_id BLOB NOT NULL,
  json_value TEXT NULL,
  deleted INTEGER NOT NULL,
  prev_ts INTEGER,
  PRIMARY KEY (ts, table_id, id)
);
CREATE INDEX IF NOT EXISTS documents_by_table_and_id ON documents (table_id, id, ts);
-- Optional: CREATE INDEX IF NOT EXISTS documents_live ON documents (table_id, id, ts) WHERE deleted = 0;

CREATE TABLE IF NOT EXISTS indexes (
  index_id BLOB NOT NULL,
  ts INTEGER NOT NULL,
  key BLOB NOT NULL,
  deleted INTEGER NOT NULL,
  table_id BLOB NULL,
  document_id BLOB NULL,
  PRIMARY KEY (index_id, key, ts)
);
-- Optional: CREATE INDEX IF NOT EXISTS indexes_by_ts ON indexes (ts);

CREATE TABLE IF NOT EXISTS globals (
  key TEXT NOT NULL,
  json_value TEXT NOT NULL,
  PRIMARY KEY (key)
);
```

ACP Event Log

- Canonical, append‑only storage of ACP session updates.
- Columns:
  - event_id INTEGER PRIMARY KEY AUTOINCREMENT
  - session_id TEXT NOT NULL
  - seq INTEGER NOT NULL                 -- per‑session sequence (monotonic)
  - ts INTEGER NOT NULL                  -- server timestamp
  - update_json TEXT NOT NULL            -- full ACP.Client.SessionUpdate JSON
  - meta_json TEXT NULL                  -- optional `_meta`
  - index_hints TEXT NULL                -- optional materialized keys (for projections)
- Indexes:
  - acp_events_by_session_seq (session_id, seq)
  - acp_events_by_ts (ts)

Example SQL

```sql
CREATE TABLE IF NOT EXISTS acp_events (
  event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id TEXT NOT NULL,
  seq INTEGER NOT NULL,
  ts INTEGER NOT NULL,
  update_json TEXT NOT NULL,
  meta_json TEXT NULL,
  index_hints TEXT NULL
);
CREATE INDEX IF NOT EXISTS acp_events_by_session_seq ON acp_events (session_id, seq);
CREATE INDEX IF NOT EXISTS acp_events_by_ts ON acp_events (ts);
```

Projection Tables (optional, minimal)

- sessions(session_id TEXT PRIMARY KEY, created_ts INTEGER, last_update_ts INTEGER, title TEXT NULL, mode_id TEXT, meta_json TEXT)
- messages(message_id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT, role TEXT, content_json TEXT, ts INTEGER)
- tool_calls(call_id TEXT PRIMARY KEY, session_id TEXT, name TEXT, status TEXT, args_json TEXT, output_json TEXT, ts INTEGER)

Notes

- ACP JSON is the source of truth; projections are derived for common UI queries.
- Use projections sparingly at MVP; prefer reading from `acp_events` and aggregating in code where feasible.

Read Patterns

- Latest document snapshot: for a given (table_id, id), select the row with max(ts) ≤ read_ts; if deleted=1, treat as tombstone.
- Index scan: select max(ts) per (index_id, key) ≤ read_ts, then join to documents to materialize values.
 - Query normalization: normalize params to canonical JSON and derive a deterministic key for reuse across subscribers.

Write Patterns

- Group document and index updates in a single transaction.
- Overwrite policy for idempotency: INSERT OR REPLACE by (ts, table, id) as needed.
 - Use a single writer actor; apply `busy_timeout` and retries on SQLITE_BUSY.

Retention & Validation

- Configurable retention horizon; run periodic compaction/GC to prune rows older than horizon where safe.
- Validate snapshot timestamps on reads when resuming from a journal.
 - Consider CHECKPOINT FULL on idle to bound WAL size; do not auto-checkpoint to avoid burst stalls.
