PRAGMA foreign_keys = OFF;

DROP INDEX IF EXISTS agent_search_requests_actor_created_idx;
DROP INDEX IF EXISTS agent_search_requests_credential_created_idx;

ALTER TABLE agent_search_requests RENAME TO agent_search_requests_old;

CREATE TABLE agent_search_requests (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  request_body_digest TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  query_text TEXT,
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  provider TEXT NOT NULL CHECK (provider IN ('exa')),
  provider_request_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  cache_status TEXT NOT NULL CHECK (cache_status IN ('hit', 'miss')),
  charge_state TEXT NOT NULL CHECK (charge_state IN ('free_allowance', 'paid_entitlement')),
  product_id TEXT,
  entitlement_ref TEXT,
  provider_cost_dollars REAL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT
);

INSERT INTO agent_search_requests
SELECT
  id,
  receipt_ref,
  actor_ref,
  agent_user_id,
  credential_id,
  token_prefix,
  idempotency_key_hash,
  request_body_digest,
  query_hash,
  query_text,
  mode,
  provider,
  provider_request_id,
  status,
  cache_status,
  charge_state,
  product_id,
  entitlement_ref,
  provider_cost_dollars,
  public_projection_json,
  created_at,
  completed_at,
  archived_at
FROM agent_search_requests_old;

DROP TABLE agent_search_requests_old;

CREATE INDEX IF NOT EXISTS agent_search_requests_actor_created_idx
  ON agent_search_requests(actor_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS agent_search_requests_credential_created_idx
  ON agent_search_requests(credential_id, created_at DESC);

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS agent_search_payment_challenges (
  id TEXT PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  method TEXT NOT NULL CHECK (method IN ('POST')),
  path TEXT NOT NULL CHECK (path = '/api/agents/search'),
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  request_body_digest TEXT NOT NULL,
  product_id TEXT NOT NULL,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('credits')),
  price_denomination TEXT NOT NULL CHECK (price_denomination = 'credit'),
  price_value INTEGER NOT NULL CHECK (price_value > 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('credits')),
  spend_cap_denomination TEXT NOT NULL CHECK (spend_cap_denomination = 'credit'),
  spend_cap_value INTEGER NOT NULL CHECK (spend_cap_value >= 0),
  expires_at TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS agent_search_payment_challenges_actor_idx
  ON agent_search_payment_challenges(actor_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_payment_receipts (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_search_payment_challenges(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('credits')),
  amount_denomination TEXT NOT NULL CHECK (amount_denomination = 'credit'),
  amount_value INTEGER NOT NULL CHECK (amount_value > 0),
  entitlement_ref TEXT NOT NULL UNIQUE,
  redacted_payment_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS agent_search_payment_receipts_actor_idx
  ON agent_search_payment_receipts(actor_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_entitlements (
  id TEXT PRIMARY KEY,
  entitlement_ref TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_search_payment_challenges(id) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES agent_search_payment_receipts(receipt_ref) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  product_id TEXT NOT NULL,
  scope_ref TEXT NOT NULL,
  method TEXT NOT NULL CHECK (method IN ('POST')),
  path TEXT NOT NULL CHECK (path = '/api/agents/search'),
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  request_body_digest TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'consumed', 'expired')),
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  consumed_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS agent_search_entitlements_actor_status_idx
  ON agent_search_entitlements(actor_ref, status, expires_at);

CREATE TABLE IF NOT EXISTS agent_search_payment_redemptions (
  id TEXT PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_search_payment_challenges(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  proof_ref TEXT NOT NULL,
  entitlement_ref TEXT NOT NULL REFERENCES agent_search_entitlements(entitlement_ref) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES agent_search_payment_receipts(receipt_ref) ON DELETE CASCADE,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);
