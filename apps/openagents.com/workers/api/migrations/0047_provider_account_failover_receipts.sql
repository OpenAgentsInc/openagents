CREATE TABLE IF NOT EXISTS provider_account_failover_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT,
  assignment_id TEXT,
  requested_action TEXT NOT NULL,
  previous_lease_ref TEXT,
  previous_provider_account_ref TEXT,
  next_lease_ref TEXT,
  next_provider_account_ref TEXT,
  failure_class TEXT NOT NULL,
  account_state_action TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (
    outcome IN ('retrying', 'blocked')
  ),
  attempt_number INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  customer_safe_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_assignment_idx
  ON provider_account_failover_receipts(assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_run_idx
  ON provider_account_failover_receipts(run_id, created_at DESC);
