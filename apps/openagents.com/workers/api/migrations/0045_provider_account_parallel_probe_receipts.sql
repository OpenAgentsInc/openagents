CREATE TABLE IF NOT EXISTS provider_account_parallel_probe_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  probe_run_id TEXT NOT NULL,
  probe_id TEXT NOT NULL,
  lease_id TEXT NOT NULL,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider_account_ref TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NOT NULL,
  terminal_status TEXT NOT NULL CHECK (
    terminal_status IN ('passed', 'failed')
  ),
  classification TEXT NOT NULL,
  collision_class TEXT NOT NULL,
  metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS provider_account_parallel_probe_receipts_run_idx
  ON provider_account_parallel_probe_receipts(probe_run_id, started_at);

CREATE INDEX IF NOT EXISTS provider_account_parallel_probe_receipts_account_idx
  ON provider_account_parallel_probe_receipts(provider_account_id, started_at DESC);
