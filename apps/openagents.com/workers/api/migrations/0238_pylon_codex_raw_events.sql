CREATE TABLE IF NOT EXISTS pylon_codex_raw_events (
  raw_event_ref TEXT PRIMARY KEY,
  assignment_ref TEXT NOT NULL,
  lease_ref TEXT NOT NULL,
  pylon_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  run_ref TEXT,
  session_ref TEXT,
  workspace_ref TEXT,
  turn_index INTEGER NOT NULL,
  event_count INTEGER NOT NULL,
  byte_length INTEGER NOT NULL,
  content_digest TEXT NOT NULL,
  r2_key TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  demand_kind TEXT NOT NULL DEFAULT 'own_capacity',
  demand_source TEXT NOT NULL DEFAULT 'khala_coding_delegation',
  UNIQUE(content_digest),
  UNIQUE(assignment_ref, lease_ref, pylon_ref, turn_index)
);

CREATE INDEX IF NOT EXISTS idx_pylon_codex_raw_events_owner_observed
  ON pylon_codex_raw_events(owner_user_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_codex_raw_events_assignment_turn
  ON pylon_codex_raw_events(assignment_ref, turn_index);

CREATE INDEX IF NOT EXISTS idx_pylon_codex_raw_events_session_turn
  ON pylon_codex_raw_events(session_ref, turn_index);
