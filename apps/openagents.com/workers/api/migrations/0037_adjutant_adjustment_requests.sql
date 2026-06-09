CREATE TABLE IF NOT EXISTS adjutant_adjustment_requests (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
  goal_id TEXT REFERENCES agent_goals(id) ON DELETE SET NULL,
  requested_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  instruction TEXT NOT NULL CHECK (length(instruction) > 0 AND length(instruction) <= 4000),
  status TEXT NOT NULL CHECK (
    status IN (
      'requested',
      'queued',
      'running',
      'review_needed',
      'completed',
      'rejected',
      'canceled',
      'failed'
    )
  ),
  continuation_mode TEXT CHECK (
    continuation_mode IS NULL OR continuation_mode IN ('follow_up_turn', 'new_goal_run')
  ),
  source_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  continuation_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  resulting_version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS adjutant_adjustment_requests_assignment_created_idx
  ON adjutant_adjustment_requests(assignment_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_adjustment_requests_order_created_idx
  ON adjutant_adjustment_requests(software_order_id, created_at DESC)
  WHERE software_order_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_adjustment_requests_site_created_idx
  ON adjutant_adjustment_requests(site_id, created_at DESC)
  WHERE archived_at IS NULL;
