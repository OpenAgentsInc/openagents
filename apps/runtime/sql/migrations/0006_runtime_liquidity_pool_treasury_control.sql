CREATE SCHEMA IF NOT EXISTS runtime;

-- Durable signer-set policy for liquidity pool treasury actions.
CREATE TABLE IF NOT EXISTS runtime.liquidity_pool_signer_sets (
    pool_id TEXT PRIMARY KEY REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    schema TEXT NOT NULL,
    threshold INTEGER NOT NULL,
    signers_json JSONB NOT NULL DEFAULT '[]'::JSONB,
    policy_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    canonical_json_sha256 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Pending approvals for high-risk pool actions (channel ops, large payouts, etc).
CREATE TABLE IF NOT EXISTS runtime.liquidity_pool_signing_requests (
    request_id TEXT PRIMARY KEY,
    pool_id TEXT NOT NULL REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    action_class TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload_json JSONB NOT NULL,
    payload_sha256 TEXT NOT NULL,
    required_signatures INTEGER NOT NULL,
    status TEXT NOT NULL,
    execution_result_json JSONB,
    executed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (pool_id, action_class, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_signing_requests_pool_status
    ON runtime.liquidity_pool_signing_requests (pool_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_signing_requests_payload_sha256
    ON runtime.liquidity_pool_signing_requests (payload_sha256);

CREATE TABLE IF NOT EXISTS runtime.liquidity_pool_signing_approvals (
    approval_id TEXT PRIMARY KEY,
    request_id TEXT NOT NULL REFERENCES runtime.liquidity_pool_signing_requests(request_id) ON DELETE CASCADE,
    signer_pubkey TEXT NOT NULL,
    signature_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (request_id, signer_pubkey)
);

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_signing_approvals_request
    ON runtime.liquidity_pool_signing_approvals (request_id, created_at DESC);

