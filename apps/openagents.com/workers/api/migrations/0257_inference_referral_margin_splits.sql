CREATE TABLE IF NOT EXISTS inference_referral_margin_splits (
  id TEXT PRIMARY KEY NOT NULL,
  request_id TEXT NOT NULL UNIQUE,
  account_ref TEXT NOT NULL,
  referred_user_id TEXT NOT NULL,
  referrer_user_id TEXT NOT NULL,
  referral_attribution_id TEXT NOT NULL
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT,
  payout_ref TEXT NOT NULL,
  qualifying_event_ref TEXT NOT NULL,
  charge_receipt_ref TEXT NOT NULL,
  funding_kind TEXT NOT NULL,
  adapter_id TEXT NOT NULL,
  requested_model TEXT NOT NULL,
  served_model TEXT NOT NULL,
  served_by_contributor INTEGER NOT NULL DEFAULT 0 CHECK (served_by_contributor IN (0, 1)),
  serving_node_count INTEGER NOT NULL DEFAULT 0 CHECK (serving_node_count >= 0),
  charge_usd REAL NOT NULL CHECK (charge_usd >= 0),
  cost_usd REAL NOT NULL CHECK (cost_usd >= 0),
  margin_usd REAL NOT NULL CHECK (margin_usd >= 0),
  margin_sats INTEGER NOT NULL CHECK (margin_sats >= 0),
  openagents_usd REAL NOT NULL CHECK (openagents_usd >= 0),
  openagents_sats INTEGER NOT NULL CHECK (openagents_sats >= 0),
  serving_node_usd REAL NOT NULL CHECK (serving_node_usd >= 0),
  serving_node_sats INTEGER NOT NULL CHECK (serving_node_sats >= 0),
  referrer_usd REAL NOT NULL CHECK (referrer_usd >= 0),
  referrer_sats INTEGER NOT NULL CHECK (referrer_sats >= 0),
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_inference_referral_margin_splits_referrer
  ON inference_referral_margin_splits(referrer_user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_inference_referral_margin_splits_attribution
  ON inference_referral_margin_splits(referral_attribution_id, created_at DESC);

