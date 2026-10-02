PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS rule (
    version INTEGER PRIMARY KEY,
    digest TEXT NOT NULL UNIQUE,
    effective INTEGER NOT NULL UNIQUE,
    toml TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settlement (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    payment_hash TEXT NOT NULL UNIQUE,
    resource TEXT NOT NULL,
    plugin_id TEXT,
    price_msat INTEGER NOT NULL CHECK(price_msat >= 0),
    received_msat INTEGER NOT NULL CHECK(received_msat >= 0 AND received_msat <= price_msat),
    lsp_fee_msat INTEGER NOT NULL CHECK(lsp_fee_msat = price_msat - received_msat),
    rail TEXT NOT NULL CHECK(rail IN ('lightning', 'balance')),
    payer_alias TEXT,
    settled_at INTEGER NOT NULL,
    rule_version INTEGER NOT NULL REFERENCES rule(version),
    short INTEGER NOT NULL CHECK(short IN (0, 1))
);
CREATE TABLE IF NOT EXISTS share (
    settlement TEXT NOT NULL REFERENCES settlement(payment_hash),
    party TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('author','resource','openagents','lsp_fee','bonus','provider','balance_credit')),
    amount_msat INTEGER NOT NULL CHECK(amount_msat >= 0),
    PRIMARY KEY(settlement, party, role)
);
CREATE TABLE IF NOT EXISTS payee (
    party TEXT PRIMARY KEY,
    destination_kind TEXT NOT NULL,
    destination_value TEXT NOT NULL,
    source TEXT NOT NULL,
    verified_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS payout (
    id TEXT PRIMARY KEY,
    party TEXT NOT NULL REFERENCES payee(party),
    amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
    destination TEXT NOT NULL,
    rail TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('pending','unknown','succeeded','failed')),
    wallet_reference TEXT,
    attempts INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS payout_item (
    payout TEXT NOT NULL REFERENCES payout(id),
    settlement TEXT NOT NULL,
    party TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY(payout, settlement, party, role),
    FOREIGN KEY(settlement, party, role) REFERENCES share(settlement, party, role)
);
CREATE TABLE IF NOT EXISTS balance (
    account TEXT PRIMARY KEY,
    prepaid_msat INTEGER NOT NULL CHECK(prepaid_msat >= 0)
);
CREATE INDEX IF NOT EXISTS share_party ON share(party);
CREATE INDEX IF NOT EXISTS payout_item_share ON payout_item(settlement, party, role);
