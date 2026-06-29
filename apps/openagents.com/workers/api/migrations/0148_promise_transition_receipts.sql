CREATE TABLE IF NOT EXISTS promise_transition_receipts (
  id TEXT PRIMARY KEY,
  promise_id TEXT NOT NULL,
  from_state TEXT NOT NULL,
  to_state TEXT NOT NULL,
  registry_version TEXT NOT NULL,
  result TEXT NOT NULL,
  checks_json TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  exception_json TEXT,
  checked_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_promise_transition_receipts_promise
  ON promise_transition_receipts(promise_id, checked_at DESC);

CREATE INDEX IF NOT EXISTS idx_promise_transition_receipts_checked
  ON promise_transition_receipts(checked_at DESC);
