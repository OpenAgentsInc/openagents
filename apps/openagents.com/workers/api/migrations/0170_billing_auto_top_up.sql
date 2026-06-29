PRAGMA foreign_keys = off;

CREATE TABLE billing_ledger_entries_next (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  team_id TEXT REFERENCES teams(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  source TEXT NOT NULL CHECK (
    source IN (
      'trial_grant',
      'coupon',
      'credit_card_placeholder',
      'stripe_checkout',
      'stripe_auto_top_up',
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

INSERT INTO billing_ledger_entries_next
SELECT * FROM billing_ledger_entries;

DROP TABLE billing_ledger_entries;

ALTER TABLE billing_ledger_entries_next RENAME TO billing_ledger_entries;

CREATE INDEX IF NOT EXISTS idx_billing_ledger_entries_user_created
  ON billing_ledger_entries(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_billing_ledger_entries_run
  ON billing_ledger_entries(run_id, created_at DESC);

PRAGMA foreign_keys = on;

CREATE TABLE IF NOT EXISTS stripe_saved_payment_methods (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  currency TEXT NOT NULL DEFAULT 'USD',
  livemode INTEGER NOT NULL DEFAULT 0 CHECK (livemode IN (0, 1)),
  stripe_customer_id TEXT NOT NULL,
  stripe_payment_method_id TEXT NOT NULL,
  setup_intent_id TEXT,
  brand TEXT,
  last4 TEXT,
  exp_month INTEGER,
  exp_year INTEGER,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'detached', 'failed', 'requires_action')
  ),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_payment_method_id, livemode)
);

CREATE INDEX IF NOT EXISTS idx_stripe_saved_payment_methods_customer
  ON stripe_saved_payment_methods(stripe_customer_id, livemode);

CREATE TABLE IF NOT EXISTS billing_auto_top_up_policies (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  currency TEXT NOT NULL DEFAULT 'USD',
  enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
  threshold_cents INTEGER NOT NULL,
  amount_cents INTEGER NOT NULL,
  monthly_cap_cents INTEGER NOT NULL,
  spent_this_month_cents INTEGER NOT NULL DEFAULT 0,
  cap_period_yyyymm TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'disabled', 'paused')),
  pause_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency)
);

CREATE TABLE IF NOT EXISTS billing_auto_top_up_events (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (
    status IN (
      'succeeded',
      'declined',
      'cap_reached',
      'skipped',
      'requires_payment_method'
    )
  ),
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  balance_before_cents INTEGER,
  balance_after_cents INTEGER,
  stripe_payment_intent_id TEXT,
  ledger_entry_id TEXT REFERENCES billing_ledger_entries(id) ON DELETE SET NULL,
  reason TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_billing_auto_top_up_events_user_created
  ON billing_auto_top_up_events(user_id, created_at DESC);
