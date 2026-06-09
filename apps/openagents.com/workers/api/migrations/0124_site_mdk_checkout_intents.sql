CREATE TABLE IF NOT EXISTS site_mdk_checkout_intents (
  id TEXT PRIMARY KEY NOT NULL,
  checkout_intent_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  site_id TEXT NOT NULL,
  site_version_id TEXT NOT NULL,
  catalog_ref TEXT NOT NULL,
  product_id TEXT NOT NULL,
  challenge_ref TEXT NOT NULL REFERENCES buyer_payment_challenges(challenge_ref) ON DELETE CASCADE,
  checkout_ref TEXT NOT NULL UNIQUE,
  checkout_url_ref TEXT NOT NULL,
  checkout_launch_path TEXT,
  provider_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('created', 'expired', 'payment_received', 'pending_payment')
  ),
  environment TEXT NOT NULL CHECK (environment IN ('production', 'sandbox')),
  sandbox INTEGER NOT NULL CHECK (sandbox IN (0, 1)),
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'usd')),
  amount_denomination TEXT NOT NULL,
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  success_return_path TEXT NOT NULL,
  cancel_return_path TEXT NOT NULL,
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  hosted_checkout_projection_json TEXT NOT NULL DEFAULT '{}',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS site_mdk_checkout_intents_site_created_idx
  ON site_mdk_checkout_intents(site_id, site_version_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_mdk_checkout_intents_provider_status_idx
  ON site_mdk_checkout_intents(provider_ref, status, updated_at DESC)
  WHERE archived_at IS NULL;
