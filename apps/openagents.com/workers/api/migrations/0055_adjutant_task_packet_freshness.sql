CREATE TABLE IF NOT EXISTS adjutant_task_packet_freshness (
  assignment_id TEXT PRIMARY KEY NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  task_spec_path TEXT NOT NULL,
  commit_sha TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('current', 'stale', 'kept_current')
  ),
  research_brief_id TEXT REFERENCES adjutant_research_briefs(id) ON DELETE SET NULL,
  research_brief_approved_at TEXT,
  source_card_count INTEGER NOT NULL DEFAULT 0,
  operator_keep_reason TEXT,
  customer_safe_summary TEXT,
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  stale_at TEXT,
  kept_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS adjutant_task_packet_freshness_status_updated_idx
  ON adjutant_task_packet_freshness(status, updated_at DESC)
  WHERE archived_at IS NULL;
