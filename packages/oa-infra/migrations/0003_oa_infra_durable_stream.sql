-- oa-infra DurableStream backend (CFG-2, issue #8517).
--
-- Append-only chunk log per stream (durable-stream-postgres.ts). The stream
-- header row owns the monotonically increasing next_offset and the closed
-- flag; chunks are immutable once written. Appends bump next_offset and
-- insert the chunk in ONE transaction so offsets are gapless per stream.

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
