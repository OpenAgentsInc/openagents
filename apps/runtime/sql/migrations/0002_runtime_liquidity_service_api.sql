CREATE SCHEMA IF NOT EXISTS runtime;

CREATE TABLE IF NOT EXISTS runtime.liquidity_quotes (
    quote_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    request_fingerprint_sha256 TEXT NOT NULL,
    invoice TEXT NOT NULL,
    invoice_hash TEXT NOT NULL,
    host TEXT NOT NULL,
    quoted_amount_msats BIGINT NOT NULL,
    max_amount_msats BIGINT NOT NULL,
    max_fee_msats BIGINT NOT NULL,
    urgency TEXT,
    policy_context_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    policy_context_sha256 TEXT NOT NULL,
    valid_until TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_liquidity_quotes_invoice_hash
    ON runtime.liquidity_quotes (invoice_hash);

CREATE TABLE IF NOT EXISTS runtime.liquidity_payments (
    quote_id TEXT PRIMARY KEY REFERENCES runtime.liquidity_quotes(quote_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    request_fingerprint_sha256 TEXT NOT NULL,
    run_id TEXT,
    trajectory_hash TEXT,
    wallet_request_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    latency_ms BIGINT,
    wallet_response_json JSONB,
    wallet_receipt_sha256 TEXT,
    preimage_sha256 TEXT,
    paid_at_ms BIGINT,
    error_code TEXT,
    error_message TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_liquidity_payments_status_started_at
    ON runtime.liquidity_payments (status, started_at);

CREATE TABLE IF NOT EXISTS runtime.liquidity_receipts (
    quote_id TEXT PRIMARY KEY REFERENCES runtime.liquidity_quotes(quote_id) ON DELETE CASCADE,
    schema TEXT NOT NULL,
    canonical_json_sha256 TEXT NOT NULL,
    signature_json JSONB,
    receipt_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

