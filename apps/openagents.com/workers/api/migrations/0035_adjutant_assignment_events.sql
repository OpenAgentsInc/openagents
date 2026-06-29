CREATE TABLE IF NOT EXISTS adjutant_assignment_events (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  goal_id TEXT REFERENCES agent_goals(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  event_type TEXT NOT NULL CHECK (length(event_type) > 0),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  summary TEXT NOT NULL CHECK (length(summary) > 0),
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS adjutant_assignment_events_assignment_created_idx
  ON adjutant_assignment_events(assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS adjutant_assignment_events_order_created_idx
  ON adjutant_assignment_events(software_order_id, created_at DESC)
  WHERE software_order_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS adjutant_assignment_events_site_created_idx
  ON adjutant_assignment_events(site_id, created_at DESC)
  WHERE site_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS adjutant_assignment_events_goal_created_idx
  ON adjutant_assignment_events(goal_id, created_at DESC)
  WHERE goal_id IS NOT NULL;
