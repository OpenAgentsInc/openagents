CREATE TABLE IF NOT EXISTS billing_accounts (
  user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  currency TEXT NOT NULL DEFAULT 'USD',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS billing_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  source TEXT NOT NULL CHECK (
    source IN (
      'trial_grant',
      'coupon',
      'credit_card_placeholder',
      'container_usage',
      'codex_usage',
      'manual_adjustment'
    )
  ),
  description TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  quantity INTEGER,
  unit TEXT,
  unit_rate_cents INTEGER,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_billing_ledger_entries_user_created
  ON billing_ledger_entries(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_billing_ledger_entries_run
  ON billing_ledger_entries(run_id, created_at DESC);

CREATE TABLE IF NOT EXISTS billing_usage_cursors (
  run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  meter TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  last_billed_at TEXT NOT NULL,
  total_billed_quantity INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, meter)
);

CREATE INDEX IF NOT EXISTS idx_billing_usage_cursors_user
  ON billing_usage_cursors(user_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS billing_coupon_redemptions (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  coupon_code TEXT NOT NULL,
  ledger_entry_id TEXT NOT NULL REFERENCES billing_ledger_entries(id) ON DELETE CASCADE,
  redeemed_at TEXT NOT NULL,
  PRIMARY KEY (user_id, coupon_code)
);
