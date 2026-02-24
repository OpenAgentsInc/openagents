CREATE SCHEMA IF NOT EXISTS runtime;

CREATE TABLE IF NOT EXISTS runtime.liquidity_pools (
    pool_id TEXT PRIMARY KEY,
    pool_kind TEXT NOT NULL,
    operator_id TEXT NOT NULL,
    status TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS runtime.liquidity_lp_accounts (
    pool_id TEXT NOT NULL REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    lp_id TEXT NOT NULL,
    shares_total BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (pool_id, lp_id)
);

CREATE TABLE IF NOT EXISTS runtime.liquidity_deposits (
    deposit_id TEXT PRIMARY KEY,
    pool_id TEXT NOT NULL REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    lp_id TEXT NOT NULL,
    rail TEXT NOT NULL,
    amount_sats BIGINT NOT NULL,
    share_price_sats BIGINT NOT NULL,
    shares_minted BIGINT NOT NULL,
    status TEXT NOT NULL,
    request_fingerprint_sha256 TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    invoice_bolt11 TEXT,
    invoice_hash TEXT,
    deposit_address TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    UNIQUE (pool_id, lp_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_liquidity_deposits_pool_status
    ON runtime.liquidity_deposits (pool_id, status);

CREATE INDEX IF NOT EXISTS idx_liquidity_deposits_invoice_hash
    ON runtime.liquidity_deposits (invoice_hash);

CREATE TABLE IF NOT EXISTS runtime.liquidity_withdrawals (
    withdrawal_id TEXT PRIMARY KEY,
    pool_id TEXT NOT NULL REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    lp_id TEXT NOT NULL,
    shares_burned BIGINT NOT NULL,
    amount_sats_estimate BIGINT NOT NULL,
    rail_preference TEXT NOT NULL,
    status TEXT NOT NULL,
    request_fingerprint_sha256 TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    earliest_settlement_at TIMESTAMPTZ NOT NULL,
    payout_invoice_hash TEXT,
    payout_address TEXT,
    wallet_receipt_sha256 TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    paid_at TIMESTAMPTZ,
    UNIQUE (pool_id, lp_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_liquidity_withdrawals_pool_status
    ON runtime.liquidity_withdrawals (pool_id, status);

CREATE TABLE IF NOT EXISTS runtime.liquidity_pool_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    pool_id TEXT NOT NULL REFERENCES runtime.liquidity_pools(pool_id) ON DELETE CASCADE,
    as_of TIMESTAMPTZ NOT NULL,
    assets_json JSONB NOT NULL,
    liabilities_json JSONB NOT NULL,
    share_price_sats BIGINT NOT NULL,
    canonical_json_sha256 TEXT NOT NULL,
    signature_json JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_pool_as_of
    ON runtime.liquidity_pool_snapshots (pool_id, as_of DESC);

CREATE TABLE IF NOT EXISTS runtime.liquidity_pool_receipts (
    receipt_id TEXT PRIMARY KEY,
    entity_kind TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    schema TEXT NOT NULL,
    canonical_json_sha256 TEXT NOT NULL,
    signature_json JSONB,
    receipt_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_kind, entity_id, schema)
);

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_receipts_entity
    ON runtime.liquidity_pool_receipts (entity_kind, entity_id);
