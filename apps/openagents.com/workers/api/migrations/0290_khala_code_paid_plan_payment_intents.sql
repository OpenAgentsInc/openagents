CREATE TABLE IF NOT EXISTS khala_code_paid_plan_payment_intents (
  purchase_ref TEXT PRIMARY KEY,
  account_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  rail TEXT NOT NULL CHECK (rail IN ('stripe_checkout', 'lightning_mpp')),
  status TEXT NOT NULL CHECK (status IN ('requires_payment', 'fulfilled', 'failed', 'expired')),
  plan_id TEXT NOT NULL,
  amount_cents INTEGER,
  amount_sats INTEGER,
  stripe_checkout_session_id TEXT UNIQUE,
  stripe_checkout_url TEXT,
  lightning_payment_hash TEXT UNIQUE,
  lightning_invoice TEXT,
  lightning_network TEXT CHECK (lightning_network IS NULL OR lightning_network IN ('mainnet', 'regtest', 'signet')),
  lightning_invoice_expires_at TEXT,
  entitlement_receipt_ref TEXT,
  failure_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  fulfilled_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_khala_code_paid_plan_intents_account
  ON khala_code_paid_plan_payment_intents (account_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_khala_code_paid_plan_intents_status
  ON khala_code_paid_plan_payment_intents (status, updated_at);
