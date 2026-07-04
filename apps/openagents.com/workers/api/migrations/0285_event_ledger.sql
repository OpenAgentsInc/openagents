CREATE TABLE IF NOT EXISTS event_ledger_entries (
  entry_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('github')),
  external_ref TEXT NOT NULL,
  actor_ref TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  event_type TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  payload_summary_json TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  received_at TEXT NOT NULL,
  ordering_key TEXT NOT NULL,
  ordering_sequence INTEGER NOT NULL CHECK (ordering_sequence >= 1),
  training_consent INTEGER NOT NULL DEFAULT 0 CHECK (training_consent = 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(owner_agent_user_id, source, external_ref),
  UNIQUE(owner_agent_user_id, ordering_sequence)
);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_received
  ON event_ledger_entries(owner_agent_user_id, received_at DESC);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_source_subject
  ON event_ledger_entries(owner_agent_user_id, source, subject_ref);
