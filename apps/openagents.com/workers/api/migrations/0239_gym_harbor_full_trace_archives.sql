CREATE TABLE IF NOT EXISTS gym_harbor_full_trace_archives (
  archive_ref TEXT PRIMARY KEY,
  run_ref TEXT NOT NULL,
  job_ref TEXT NOT NULL,
  source_kind TEXT NOT NULL DEFAULT 'harbor_job_tarball'
    CHECK (source_kind = 'harbor_job_tarball'),
  artifact_r2_key TEXT NOT NULL,
  artifact_sha256 TEXT NOT NULL UNIQUE,
  artifact_bytes INTEGER NOT NULL CHECK (artifact_bytes > 0),
  content_type TEXT NOT NULL DEFAULT 'application/gzip',
  capture_started_at TEXT,
  capture_completed_at TEXT NOT NULL,
  visibility TEXT NOT NULL DEFAULT 'operator_only'
    CHECK (visibility = 'operator_only'),
  contains_raw_prompts INTEGER NOT NULL DEFAULT 1
    CHECK (contains_raw_prompts = 1),
  contains_raw_logs INTEGER NOT NULL DEFAULT 1
    CHECK (contains_raw_logs = 1),
  contains_private_material INTEGER NOT NULL DEFAULT 1
    CHECK (contains_private_material = 1),
  demand_kind TEXT NOT NULL DEFAULT 'internal'
    CHECK (demand_kind = 'internal'),
  demand_source TEXT NOT NULL DEFAULT 'harbor_terminal_bench',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gym_harbor_full_trace_archives_run_capture
  ON gym_harbor_full_trace_archives(run_ref, capture_completed_at DESC);

CREATE INDEX IF NOT EXISTS idx_gym_harbor_full_trace_archives_job_capture
  ON gym_harbor_full_trace_archives(job_ref, capture_completed_at DESC);
