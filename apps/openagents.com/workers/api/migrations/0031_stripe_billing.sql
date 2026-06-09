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

CREATE TABLE IF NOT EXISTS stripe_customers (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  currency TEXT NOT NULL DEFAULT 'USD',
  stripe_customer_id TEXT NOT NULL,
  livemode INTEGER NOT NULL DEFAULT 0 CHECK (livemode IN (0, 1)),
  email_snapshot TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, currency, livemode),
  UNIQUE (stripe_customer_id, livemode)
);

CREATE TABLE IF NOT EXISTS stripe_checkout_sessions (
  session_id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  package_id TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  payment_status TEXT NOT NULL,
  fulfillment_status TEXT NOT NULL CHECK (
    fulfillment_status IN ('pending', 'fulfilled', 'unpaid', 'expired', 'mismatched')
  ),
  ledger_entry_id TEXT REFERENCES billing_ledger_entries(id) ON DELETE SET NULL,
  stripe_customer_id TEXT NOT NULL,
  checkout_url TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_stripe_checkout_sessions_user_created
  ON stripe_checkout_sessions(user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_stripe_checkout_sessions_fulfillment
  ON stripe_checkout_sessions(fulfillment_status, updated_at DESC);

CREATE TABLE IF NOT EXISTS stripe_webhook_events (
  event_id TEXT PRIMARY KEY NOT NULL,
  type TEXT NOT NULL,
  processing_status TEXT NOT NULL CHECK (
    processing_status IN ('received', 'processed', 'ignored', 'failed')
  ),
  checkout_session_id TEXT,
  received_at TEXT NOT NULL,
  processed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_stripe_webhook_events_received
  ON stripe_webhook_events(received_at DESC);
