-- The purchased compute balance (docs/cloud/retail-contract.md). Amounts are
-- millisatoshis; one credit is one sat. Secrets never enter these tables:
-- a principal's credential is stored as its SHA-256 digest.
CREATE TABLE IF NOT EXISTS compute_account (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS compute_principal (
    id TEXT PRIMARY KEY,
    account TEXT NOT NULL REFERENCES compute_account(id),
    kind TEXT NOT NULL CHECK(kind IN ('window','workshop','cli','api_key','phone')),
    credential TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK(generation >= 1),
    can_read INTEGER NOT NULL CHECK(can_read IN (0, 1)),
    can_spend INTEGER NOT NULL CHECK(can_spend IN (0, 1)),
    bound_at INTEGER NOT NULL,
    revoked_at INTEGER
);
CREATE INDEX IF NOT EXISTS compute_principal_account ON compute_principal(account);
