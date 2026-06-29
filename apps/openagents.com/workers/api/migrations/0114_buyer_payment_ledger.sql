CREATE TABLE IF NOT EXISTS buyer_payment_challenges (
  id TEXT PRIMARY KEY NOT NULL,
  challenge_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  product_id TEXT NOT NULL,
  surface TEXT NOT NULL CHECK (
    surface IN ('agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout')
  ),
  method TEXT NOT NULL CHECK (method IN ('GET', 'POST', 'PUT', 'PATCH', 'DELETE')),
  path TEXT NOT NULL,
  request_body_digest TEXT NOT NULL,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('bitcoin', 'credits', 'usd')),
  price_denomination TEXT NOT NULL,
  price_amount_minor_units INTEGER NOT NULL CHECK (price_amount_minor_units >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination TEXT NOT NULL,
  spend_cap_amount_minor_units INTEGER NOT NULL CHECK (spend_cap_amount_minor_units >= 0),
  status TEXT NOT NULL CHECK (status IN ('issued', 'expired', 'cancelled')),
  expires_at TEXT NOT NULL,
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS buyer_payment_challenges_actor_product_idx
  ON buyer_payment_challenges(actor_ref, product_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  challenge_ref TEXT NOT NULL REFERENCES buyer_payment_challenges(challenge_ref) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  product_id TEXT NOT NULL,
  surface TEXT NOT NULL CHECK (
    surface IN ('agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout')
  ),
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL,
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  entitlement_ref TEXT NOT NULL UNIQUE,
  redacted_payment_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('issued', 'voided')),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS buyer_payment_receipts_actor_product_idx
  ON buyer_payment_receipts(actor_ref, product_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_entitlements (
  id TEXT PRIMARY KEY NOT NULL,
  entitlement_ref TEXT NOT NULL UNIQUE,
  challenge_ref TEXT NOT NULL REFERENCES buyer_payment_challenges(challenge_ref) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES buyer_payment_receipts(receipt_ref) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  product_id TEXT NOT NULL,
  surface TEXT NOT NULL CHECK (
    surface IN ('agent_api', 'billing', 'forum_paid_action', 'runner', 'site_checkout')
  ),
  scope_refs_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK (status IN ('active', 'consumed', 'expired', 'revoked')),
  expires_at TEXT,
  created_at TEXT NOT NULL,
  consumed_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS buyer_payment_entitlements_actor_status_idx
  ON buyer_payment_entitlements(actor_ref, product_id, status, expires_at)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_redemptions (
  id TEXT PRIMARY KEY NOT NULL,
  redemption_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  challenge_ref TEXT NOT NULL REFERENCES buyer_payment_challenges(challenge_ref) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  proof_ref TEXT NOT NULL,
  entitlement_ref TEXT NOT NULL REFERENCES buyer_payment_entitlements(entitlement_ref) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES buyer_payment_receipts(receipt_ref) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('redeemed', 'replayed', 'rejected')),
  replayed INTEGER NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (challenge_ref)
);

CREATE INDEX IF NOT EXISTS buyer_payment_redemptions_actor_created_idx
  ON buyer_payment_redemptions(actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_spend_limits (
  id TEXT PRIMARY KEY NOT NULL,
  spend_limit_ref TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  product_id TEXT,
  scope_ref TEXT NOT NULL,
  window_ref TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL,
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  status TEXT NOT NULL CHECK (status IN ('active', 'exhausted', 'revoked')),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, scope_ref, window_ref)
);

CREATE INDEX IF NOT EXISTS buyer_payment_spend_limits_actor_status_idx
  ON buyer_payment_spend_limits(actor_ref, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_credit_debits (
  id TEXT PRIMARY KEY NOT NULL,
  debit_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  product_id TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits')),
  amount_denomination TEXT NOT NULL,
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  billing_ledger_entry_ref TEXT,
  receipt_ref TEXT REFERENCES buyer_payment_receipts(receipt_ref) ON DELETE SET NULL,
  status TEXT NOT NULL CHECK (status IN ('reserved', 'captured', 'released', 'voided')),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS buyer_payment_credit_debits_actor_created_idx
  ON buyer_payment_credit_debits(actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS buyer_payment_reconciliation_events (
  id TEXT PRIMARY KEY NOT NULL,
  event_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  provider_ref TEXT NOT NULL,
  external_event_ref TEXT NOT NULL,
  challenge_ref TEXT REFERENCES buyer_payment_challenges(challenge_ref) ON DELETE SET NULL,
  receipt_ref TEXT REFERENCES buyer_payment_receipts(receipt_ref) ON DELETE SET NULL,
  product_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('observed', 'matched', 'replayed', 'rejected')),
  result_ref TEXT NOT NULL,
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_event_ref)
);

CREATE INDEX IF NOT EXISTS buyer_payment_reconciliation_events_created_idx
  ON buyer_payment_reconciliation_events(created_at DESC)
  WHERE archived_at IS NULL;
