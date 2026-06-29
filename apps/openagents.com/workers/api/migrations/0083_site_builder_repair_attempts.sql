CREATE TABLE IF NOT EXISTS site_builder_repair_attempts (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  session_id TEXT NOT NULL,
  preview_id TEXT,
  phase_kind TEXT,
  attempt_number INTEGER NOT NULL,
  retry_budget INTEGER NOT NULL,
  status TEXT NOT NULL,
  failure_kind TEXT NOT NULL,
  redacted_summary TEXT NOT NULL,
  stop_reason TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT,
  FOREIGN KEY (session_id) REFERENCES site_builder_sessions(id),
  FOREIGN KEY (preview_id) REFERENCES site_builder_previews(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_site_builder_repair_attempts_session_attempt
  ON site_builder_repair_attempts(session_id, attempt_number);

CREATE INDEX IF NOT EXISTS idx_site_builder_repair_attempts_session_created
  ON site_builder_repair_attempts(session_id, created_at DESC);
