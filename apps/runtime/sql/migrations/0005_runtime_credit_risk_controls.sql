CREATE SCHEMA IF NOT EXISTS runtime;

-- Underwriting audit records store the inputs and decisions used for a given offer_id.
-- These are not "receipts" (they are internal admin/audit artifacts).
CREATE TABLE IF NOT EXISTS runtime.credit_underwriting_audit (
    offer_id TEXT PRIMARY KEY REFERENCES runtime.credit_offers(offer_id) ON DELETE CASCADE,
    canonical_json_sha256 TEXT NOT NULL,
    audit_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_underwriting_audit_created
    ON runtime.credit_underwriting_audit (created_at DESC);

-- CEP uses Liquidity Service to pay provider invoices (issuer-pays). We record pay outcomes
-- (success/failure) so the CEP can compute LN failure-rate circuit breakers deterministically.
CREATE TABLE IF NOT EXISTS runtime.credit_liquidity_pay_events (
    quote_id TEXT PRIMARY KEY,
    envelope_id TEXT NOT NULL REFERENCES runtime.credit_envelopes(envelope_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    error_code TEXT,
    amount_msats BIGINT NOT NULL,
    host TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_liquidity_pay_events_created
    ON runtime.credit_liquidity_pay_events (created_at DESC);

