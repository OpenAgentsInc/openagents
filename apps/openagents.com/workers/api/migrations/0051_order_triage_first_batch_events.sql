CREATE TABLE IF NOT EXISTS order_triage_events (
  id TEXT PRIMARY KEY NOT NULL,
  triage_record_id TEXT NOT NULL REFERENCES order_triage_records(id) ON DELETE CASCADE,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  event_type TEXT NOT NULL CHECK (length(event_type) > 0),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  summary TEXT NOT NULL CHECK (length(summary) > 0),
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  payload_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS order_triage_events_order_created_idx
  ON order_triage_events(software_order_id, created_at DESC);

CREATE INDEX IF NOT EXISTS order_triage_events_assignment_created_idx
  ON order_triage_events(assignment_id, created_at DESC)
  WHERE assignment_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS order_triage_events_site_created_idx
  ON order_triage_events(site_id, created_at DESC)
  WHERE site_id IS NOT NULL;
