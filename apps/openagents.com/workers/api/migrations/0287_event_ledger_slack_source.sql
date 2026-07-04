CREATE TABLE IF NOT EXISTS event_ledger_entries_next (
  entry_id TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  owner_ref TEXT NOT NULL,
  source TEXT NOT NULL CHECK (source IN ('github', 'slack')),
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
  handled_state TEXT NOT NULL DEFAULT 'open'
    CHECK (handled_state IN ('open', 'handled', 'responded', 'ignored')),
  handled_by_run_id TEXT,
  handled_by_definition_id TEXT,
  handled_at TEXT,
  handled_reason_ref TEXT,
  training_consent INTEGER NOT NULL DEFAULT 0 CHECK (training_consent = 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(owner_agent_user_id, source, external_ref),
  UNIQUE(owner_agent_user_id, ordering_sequence)
);

INSERT OR IGNORE INTO event_ledger_entries_next (
  entry_id, owner_agent_user_id, owner_ref, source, external_ref,
  actor_ref, content_ref, subject_ref, event_type, source_refs_json,
  payload_summary_json, occurred_at, received_at, ordering_key,
  ordering_sequence, handled_state, handled_by_run_id,
  handled_by_definition_id, handled_at, handled_reason_ref, training_consent,
  created_at, updated_at
)
SELECT
  entry_id, owner_agent_user_id, owner_ref, source, external_ref,
  actor_ref, content_ref, subject_ref, event_type, source_refs_json,
  payload_summary_json, occurred_at, received_at, ordering_key,
  ordering_sequence, handled_state, handled_by_run_id,
  handled_by_definition_id, handled_at, handled_reason_ref, training_consent,
  created_at, updated_at
FROM event_ledger_entries;

DROP TABLE event_ledger_entries;

ALTER TABLE event_ledger_entries_next RENAME TO event_ledger_entries;

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_received
  ON event_ledger_entries(owner_agent_user_id, received_at DESC);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_source_subject
  ON event_ledger_entries(owner_agent_user_id, source, subject_ref);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_state_sequence
  ON event_ledger_entries(owner_agent_user_id, handled_state, ordering_sequence);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_handled_run
  ON event_ledger_entries(owner_agent_user_id, handled_by_run_id);
