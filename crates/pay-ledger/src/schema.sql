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
    release_id TEXT,
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
CREATE TABLE IF NOT EXISTS call (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    route TEXT NOT NULL,
    resource TEXT NOT NULL,
    plugin_id TEXT,
    release_id TEXT,
    outcome TEXT NOT NULL,
    paid INTEGER NOT NULL CHECK(paid IN (0, 1)),
    price_msat INTEGER CHECK(price_msat IS NULL OR price_msat >= 0)
);
CREATE INDEX IF NOT EXISTS share_party ON share(party);
CREATE INDEX IF NOT EXISTS payout_item_share ON payout_item(settlement, party, role);
CREATE TABLE IF NOT EXISTS bonus (
    settlement TEXT NOT NULL REFERENCES settlement(payment_hash),
    party TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('first_paid_call','launch_match')),
    plugin_id TEXT NOT NULL,
    month TEXT NOT NULL,
    requested_msat INTEGER NOT NULL CHECK(requested_msat >= 0),
    amount_msat INTEGER NOT NULL CHECK(amount_msat >= 0 AND amount_msat <= requested_msat),
    outcome TEXT NOT NULL CHECK(outcome IN ('awarded','bonus_unfunded')),
    PRIMARY KEY(settlement, party, kind)
);
CREATE UNIQUE INDEX IF NOT EXISTS bonus_first_plugin ON bonus(plugin_id) WHERE kind='first_paid_call';
CREATE INDEX IF NOT EXISTS bonus_month ON bonus(party, month, kind);
CREATE TABLE IF NOT EXISTS bonus_funding (
    settlement TEXT NOT NULL,
    party TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind='first_paid_call'),
    source_settlement TEXT NOT NULL,
    source_party TEXT NOT NULL CHECK(source_party='openagents'),
    source_role TEXT NOT NULL CHECK(source_role='openagents'),
    amount_msat INTEGER NOT NULL CHECK(amount_msat > 0),
    PRIMARY KEY(settlement, party, kind, source_settlement),
    FOREIGN KEY(settlement, party, kind) REFERENCES bonus(settlement, party, kind),
    FOREIGN KEY(source_settlement, source_party, source_role) REFERENCES share(settlement, party, role)
);
CREATE INDEX IF NOT EXISTS bonus_funding_source ON bonus_funding(source_settlement);
CREATE TABLE IF NOT EXISTS bonus_payout_item (
    payout TEXT NOT NULL REFERENCES payout(id),
    settlement TEXT NOT NULL,
    party TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role='first_paid_call'),
    PRIMARY KEY(payout, settlement, party, role),
    FOREIGN KEY(settlement, party, role) REFERENCES bonus(settlement, party, kind)
);
CREATE INDEX IF NOT EXISTS bonus_payout_share ON bonus_payout_item(settlement, party, role);
-- Settlement shares remain immutable. This view exposes the remaining claims
-- after first-call bonuses transfer ownership of unpaid OpenAgents funds.
CREATE VIEW IF NOT EXISTS payable_share AS
SELECT s.settlement,s.party,s.role,s.amount_msat - CASE
    WHEN s.party='openagents' AND s.role='openagents' THEN
        COALESCE((SELECT SUM(f.amount_msat) FROM bonus_funding f WHERE f.source_settlement=s.settlement),0)
    ELSE 0 END AS amount_msat FROM share s
UNION ALL
SELECT settlement,party,kind,amount_msat FROM bonus WHERE kind='first_paid_call';
