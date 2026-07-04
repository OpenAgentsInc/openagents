ALTER TABLE event_ledger_entries
  ADD COLUMN handled_state TEXT NOT NULL DEFAULT 'open'
    CHECK (handled_state IN ('open', 'handled', 'responded', 'ignored'));

ALTER TABLE event_ledger_entries
  ADD COLUMN handled_by_run_id TEXT;

ALTER TABLE event_ledger_entries
  ADD COLUMN handled_by_definition_id TEXT;

ALTER TABLE event_ledger_entries
  ADD COLUMN handled_at TEXT;

ALTER TABLE event_ledger_entries
  ADD COLUMN handled_reason_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_state_sequence
  ON event_ledger_entries(owner_agent_user_id, handled_state, ordering_sequence);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_handled_run
  ON event_ledger_entries(owner_agent_user_id, handled_by_run_id);
