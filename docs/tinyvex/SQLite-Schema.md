# SQLite Schema — Log‑Structured Storage

Goals

- Append‑only log with timestamped rows to support snapshot reads and reactive invalidation.
- Single writer (actor) with WAL mode for concurrency with readers.

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

indexes

- Columns:
  - index_id BLOB NOT NULL
  - ts INTEGER NOT NULL
  - key BLOB NOT NULL
  - deleted INTEGER NOT NULL DEFAULT 0
  - table_id BLOB NULL
  - document_id BLOB NULL
- Primary key: (index_id, key, ts)

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

CREATE TABLE IF NOT EXISTS indexes (
  index_id BLOB NOT NULL,
  ts INTEGER NOT NULL,
  key BLOB NOT NULL,
  deleted INTEGER NOT NULL,
  table_id BLOB NULL,
  document_id BLOB NULL,
  PRIMARY KEY (index_id, key, ts)
);

CREATE TABLE IF NOT EXISTS globals (
  key TEXT NOT NULL,
  json_value TEXT NOT NULL,
  PRIMARY KEY (key)
);
```

Read Patterns

- Latest document snapshot: for a given (table_id, id), select the row with max(ts) ≤ read_ts; if deleted=1, treat as tombstone.
- Index scan: select max(ts) per (index_id, key) ≤ read_ts, then join to documents to materialize values.

Write Patterns

- Group document and index updates in a single transaction.
- Overwrite policy for idempotency: INSERT OR REPLACE by (ts, table, id) as needed.

Retention & Validation

- Configurable retention horizon; run periodic compaction/GC to prune rows older than horizon where safe.
- Validate snapshot timestamps on reads when resuming from a journal.

