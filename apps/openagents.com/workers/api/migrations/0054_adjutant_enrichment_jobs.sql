CREATE TABLE IF NOT EXISTS adjutant_enrichment_jobs (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  enrichment_run_id TEXT REFERENCES exa_enrichment_runs(id) ON DELETE SET NULL,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'running', 'succeeded', 'failed', 'skipped', 'canceled')
  ),
  trigger_kind TEXT NOT NULL CHECK (
    trigger_kind IN ('research_required', 'operator_requested', 'operator_refresh')
  ),
  refresh INTEGER NOT NULL DEFAULT 0 CHECK (refresh IN (0, 1)),
  requested_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  request_json TEXT,
  error_code TEXT,
  error_summary TEXT,
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS adjutant_enrichment_jobs_active_assignment_idx
  ON adjutant_enrichment_jobs(assignment_id)
  WHERE archived_at IS NULL
    AND status IN ('queued', 'running');

CREATE INDEX IF NOT EXISTS adjutant_enrichment_jobs_assignment_updated_idx
  ON adjutant_enrichment_jobs(assignment_id, updated_at DESC)
  WHERE archived_at IS NULL;
