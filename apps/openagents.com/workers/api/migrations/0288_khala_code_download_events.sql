CREATE TABLE IF NOT EXISTS khala_code_download_events (
  event_ref TEXT PRIMARY KEY,
  product TEXT NOT NULL,
  artifact_kind TEXT NOT NULL CHECK (
    artifact_kind IN ('desktop_dmg', 'npm_cli', 'source_build')
  ),
  channel TEXT NOT NULL,
  artifact_ref TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  public_countable INTEGER NOT NULL DEFAULT 1 CHECK (public_countable IN (0, 1)),
  source_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_code_download_events_public_counts
  ON khala_code_download_events (
    product,
    public_countable,
    artifact_kind,
    channel,
    occurred_at
  );
