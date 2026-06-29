CREATE TABLE IF NOT EXISTS world_projection_rows (
  row_ref TEXT PRIMARY KEY,
  row_kind TEXT NOT NULL,
  region_ref TEXT,
  run_ref TEXT,
  source_ref TEXT NOT NULL,
  cursor TEXT,
  payload_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_world_projection_rows_region_kind
  ON world_projection_rows(region_ref, row_kind, updated_at);

CREATE INDEX IF NOT EXISTS idx_world_projection_rows_run_kind
  ON world_projection_rows(run_ref, row_kind, updated_at);

CREATE TABLE IF NOT EXISTS world_projection_cursors (
  source_ref TEXT PRIMARY KEY,
  cursor TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  lag_seconds INTEGER,
  diagnostic_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS world_bridge_ingest_log (
  ingest_ref TEXT PRIMARY KEY,
  source_ref TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  accepted_at TEXT NOT NULL,
  cursor TEXT,
  row_count INTEGER NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_world_bridge_ingest_log_source_observed
  ON world_bridge_ingest_log(source_ref, observed_at);
