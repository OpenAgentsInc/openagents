CREATE SCHEMA IF NOT EXISTS runtime;

CREATE TABLE IF NOT EXISTS runtime.credit_intents (
    intent_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    max_sats BIGINT NOT NULL,
    exp TIMESTAMPTZ NOT NULL,
    raw_json JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_credit_intents_agent_scope
    ON runtime.credit_intents (agent_id, scope_type, scope_id);

CREATE TABLE IF NOT EXISTS runtime.credit_offers (
    offer_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    pool_id TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    max_sats BIGINT NOT NULL,
    fee_bps INT NOT NULL,
    requires_verifier BOOL NOT NULL DEFAULT false,
    exp TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    request_fingerprint_sha256 TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_credit_offers_agent_scope
    ON runtime.credit_offers (agent_id, scope_type, scope_id);

CREATE TABLE IF NOT EXISTS runtime.credit_envelopes (
    envelope_id TEXT PRIMARY KEY,
    offer_id TEXT NOT NULL REFERENCES runtime.credit_offers(offer_id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL,
    pool_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    max_sats BIGINT NOT NULL,
    fee_bps INT NOT NULL,
    exp TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    request_fingerprint_sha256 TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_credit_envelopes_agent_status
    ON runtime.credit_envelopes (agent_id, status, issued_at DESC);

CREATE INDEX IF NOT EXISTS idx_credit_envelopes_scope
    ON runtime.credit_envelopes (scope_type, scope_id);

CREATE TABLE IF NOT EXISTS runtime.credit_settlements (
    settlement_id TEXT PRIMARY KEY,
    envelope_id TEXT NOT NULL REFERENCES runtime.credit_envelopes(envelope_id) ON DELETE CASCADE,
    outcome TEXT NOT NULL,
    spent_sats BIGINT NOT NULL,
    fee_sats BIGINT NOT NULL,
    verification_receipt_sha256 TEXT NOT NULL,
    liquidity_receipt_sha256 TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    request_fingerprint_sha256 TEXT NOT NULL,
    UNIQUE (envelope_id)
);

CREATE INDEX IF NOT EXISTS idx_credit_settlements_outcome_created
    ON runtime.credit_settlements (outcome, created_at DESC);

CREATE TABLE IF NOT EXISTS runtime.credit_receipts (
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

CREATE INDEX IF NOT EXISTS idx_credit_receipts_entity
    ON runtime.credit_receipts (entity_kind, entity_id);

