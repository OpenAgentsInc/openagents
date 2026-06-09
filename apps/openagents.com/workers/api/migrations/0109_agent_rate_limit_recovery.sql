CREATE TABLE agent_rate_limit_challenges (
  id TEXT PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  route_key TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  submission_idempotency_key_hash TEXT NOT NULL,
  client_fingerprint_hash TEXT NOT NULL,
  request_body_digest TEXT NOT NULL,
  price_asset TEXT NOT NULL CHECK (price_asset IN ('bitcoin', 'credits', 'usd')),
  price_denomination TEXT NOT NULL,
  price_value INTEGER NOT NULL CHECK (price_value >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination TEXT NOT NULL,
  spend_cap_value INTEGER NOT NULL CHECK (spend_cap_value >= 0),
  entitlement_kind TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX agent_rate_limit_challenges_actor_route_idx
  ON agent_rate_limit_challenges(actor_ref, route_key, created_at DESC);

CREATE TABLE agent_rate_limit_receipts (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_rate_limit_challenges(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  route_key TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL,
  amount_value INTEGER NOT NULL CHECK (amount_value >= 0),
  entitlement_ref TEXT NOT NULL UNIQUE,
  redacted_payment_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX agent_rate_limit_receipts_actor_route_idx
  ON agent_rate_limit_receipts(actor_ref, route_key, created_at DESC);

CREATE TABLE agent_rate_limit_entitlements (
  id TEXT PRIMARY KEY,
  entitlement_ref TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_rate_limit_challenges(id) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES agent_rate_limit_receipts(receipt_ref) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT NOT NULL,
  route_key TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  submission_idempotency_key_hash TEXT NOT NULL,
  client_fingerprint_hash TEXT NOT NULL,
  request_body_digest TEXT NOT NULL,
  entitlement_kind TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('active', 'consumed', 'expired')),
  expires_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  consumed_at TEXT,
  archived_at TEXT
);

CREATE INDEX agent_rate_limit_entitlements_actor_route_idx
  ON agent_rate_limit_entitlements(actor_ref, route_key, status, expires_at);

CREATE TABLE agent_rate_limit_redemptions (
  id TEXT PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  challenge_id TEXT NOT NULL UNIQUE REFERENCES agent_rate_limit_challenges(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  proof_ref TEXT NOT NULL,
  entitlement_ref TEXT NOT NULL REFERENCES agent_rate_limit_entitlements(entitlement_ref) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL REFERENCES agent_rate_limit_receipts(receipt_ref) ON DELETE CASCADE,
  replayed INTEGER NOT NULL DEFAULT 0 CHECK (replayed IN (0, 1)),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);
