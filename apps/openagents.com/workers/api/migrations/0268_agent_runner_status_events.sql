CREATE TABLE IF NOT EXISTS pylon_agent_runner_status_events (
  event_ref TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  runner_ref TEXT NOT NULL,
  runner_kind TEXT NOT NULL,
  pylon_ref TEXT,
  assignment_ref TEXT,
  state TEXT NOT NULL,
  state_started_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  retention_state TEXT NOT NULL,
  event_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  retained_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pylon_agent_runner_status_owner_retention_updated
  ON pylon_agent_runner_status_events(owner_agent_user_id, retention_state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_agent_runner_status_owner_runner_live
  ON pylon_agent_runner_status_events(owner_agent_user_id, runner_ref, retention_state);
