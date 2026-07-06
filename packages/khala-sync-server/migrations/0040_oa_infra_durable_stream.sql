-- Durable inference streams on Postgres (CFG-6, issue #8521, epic #8515).
--
-- The oa-infra DurableStream backend tables (packages/oa-infra
-- migrations/0003_oa_infra_durable_stream.sql — keep the DDL in sync; both
-- systems use IF NOT EXISTS so they compose on the same database). The
-- openagents.com Worker replaces the DurableInferenceStreamObject Durable
-- Object with this Postgres append log, reached through the KHALA_SYNC_DB
-- Hyperdrive binding with postgres.js (transaction-mode safe: single
-- BEGIN..COMMIT appends, row locks on the header row only).
--
-- One stream per durable inference request id: the header row owns the
-- monotonically increasing next_offset and the closed flag; chunks are
-- immutable SSE frames once written. Appends bump next_offset and insert
-- the chunk in ONE transaction so offsets are gapless per stream.

CREATE TABLE IF NOT EXISTS oa_infra_streams (
  stream_id   text        PRIMARY KEY,
  closed      boolean     NOT NULL DEFAULT false,
  next_offset bigint      NOT NULL DEFAULT 0,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS oa_infra_stream_chunks (
  stream_id    text        NOT NULL
                           REFERENCES oa_infra_streams (stream_id)
                           ON DELETE CASCADE,
  chunk_offset bigint      NOT NULL,
  chunk        text        NOT NULL,
  created_at   timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (stream_id, chunk_offset)
);

-- TTL sweep support (the DO backend expired per-stream state with a storage
-- alarm; on Postgres the Worker's producer path opportunistically deletes
-- headers older than the stream TTL — chunks follow via ON DELETE CASCADE).
CREATE INDEX IF NOT EXISTS oa_infra_streams_updated_at_idx
  ON oa_infra_streams (updated_at);
