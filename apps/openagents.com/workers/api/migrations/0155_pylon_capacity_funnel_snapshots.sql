CREATE TABLE IF NOT EXISTS pylon_capacity_funnel_snapshots (
  id TEXT PRIMARY KEY,
  bucket_kind TEXT NOT NULL CHECK (bucket_kind IN ('hourly', 'daily')),
  bucket_start_at TEXT NOT NULL,
  snapshot_at TEXT NOT NULL,
  total_count INTEGER NOT NULL,
  aggregate_json TEXT NOT NULL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(bucket_kind, bucket_start_at)
);

CREATE INDEX IF NOT EXISTS idx_pylon_capacity_funnel_snapshots_bucket_start
  ON pylon_capacity_funnel_snapshots(bucket_kind, bucket_start_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_capacity_funnel_snapshots_updated
  ON pylon_capacity_funnel_snapshots(updated_at DESC);
