ALTER TABLE provider_accounts
  ADD COLUMN operator_priority INTEGER NOT NULL DEFAULT 100;

ALTER TABLE provider_accounts
  ADD COLUMN cooldown_until TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN low_credit_flag INTEGER NOT NULL DEFAULT 0;

ALTER TABLE provider_accounts
  ADD COLUMN recent_failure_class TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN last_selected_at TEXT;

CREATE TABLE IF NOT EXISTS provider_account_leases (
  id TEXT PRIMARY KEY NOT NULL,
  lease_ref TEXT NOT NULL UNIQUE,
  provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  provider TEXT NOT NULL CHECK (provider IN ('chatgpt_codex')),
  provider_account_ref TEXT NOT NULL,
  requested_action TEXT NOT NULL,
  run_id TEXT,
  assignment_id TEXT,
  selected_by_policy_version TEXT NOT NULL,
  selection_reason TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'expired', 'released', 'succeeded', 'failed')
  ),
  started_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  released_at TEXT,
  terminal_outcome TEXT,
  metadata_json TEXT
);

CREATE INDEX IF NOT EXISTS provider_account_leases_active_idx
  ON provider_account_leases(provider_account_id, status, expires_at);

CREATE INDEX IF NOT EXISTS provider_account_leases_user_idx
  ON provider_account_leases(user_id, started_at DESC);
