-- Private invoice identities and owner-selected public earnings receipts.
CREATE SCHEMA IF NOT EXISTS money;

CREATE TABLE money.named_invoices (
    payment_hash text PRIMARY KEY CHECK (payment_hash ~ '^[0-9a-f]{64}$'),
    account text NOT NULL,
    party text NOT NULL CHECK (party ~ '^[0-9a-f]{64}$'),
    wallet text NOT NULL CHECK (wallet ~ '^[0-9a-f]{66}$'),
    name text NOT NULL,
    revision bigint NOT NULL CHECK (revision > 0),
    amount_msat bigint NOT NULL CHECK (amount_msat > 0 AND amount_msat <= 2100000000000000000),
    issued_at bigint NOT NULL CHECK (issued_at > 0),
    expires_at bigint NOT NULL CHECK (expires_at > issued_at),
    tip boolean NOT NULL,
    paid_at bigint CHECK (paid_at > 0),
    checked_at bigint NOT NULL DEFAULT 0,
    verified_unpaid_at bigint NOT NULL DEFAULT 0
);
CREATE INDEX named_invoices_account ON money.named_invoices(account, party, issued_at DESC);
CREATE INDEX named_invoices_waiting ON money.named_invoices(account, party, issued_at DESC)
    WHERE tip AND paid_at IS NULL;

CREATE TABLE money.published_earnings (
    source_id text PRIMARY KEY CHECK (source_id ~ '^[0-9a-f]{64}$'),
    account text NOT NULL,
    party text NOT NULL CHECK (party ~ '^[0-9a-f]{64}$'),
    source text NOT NULL,
    event_id text NOT NULL UNIQUE CHECK (event_id ~ '^[0-9a-f]{64}$'),
    event text NOT NULL,
    created_at bigint NOT NULL CHECK (created_at > 0)
);
CREATE INDEX published_earnings_party ON money.published_earnings(party, created_at DESC);

CREATE TABLE money.published_earning_versions (
    event_id text PRIMARY KEY CHECK (event_id ~ '^[0-9a-f]{64}$'),
    source_id text NOT NULL REFERENCES money.published_earnings(source_id) ON DELETE CASCADE,
    account text NOT NULL,
    source text NOT NULL,
    event text NOT NULL
);

-- This signs service receipts; it is not a customer's wallet or recovery key.
CREATE TABLE money.receipt_service_key (
    id smallint PRIMARY KEY CHECK (id = 1),
    secret text NOT NULL CHECK (secret ~ '^[0-9a-f]{64}$')
);
