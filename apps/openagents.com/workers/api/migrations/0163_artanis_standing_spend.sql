-- Artanis standing spend grants (issue #4703): the owner grants a
-- bounded, revocable envelope; mind-proposed treasury spends within the
-- envelope auto-satisfy their approval requirement WITH the grant cited.
-- Anything outside blocks for explicit approval. Spends log per-day
-- accounting against the grant.

CREATE TABLE IF NOT EXISTS artanis_standing_spend_grants (
  grant_ref TEXT PRIMARY KEY,
  per_payout_cap_sat INTEGER NOT NULL CHECK (per_payout_cap_sat > 0),
  per_day_cap_sat INTEGER NOT NULL CHECK (per_day_cap_sat > 0),
  authority_ref TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  revoked_at TEXT
);

CREATE TABLE IF NOT EXISTS artanis_spend_decisions (
  id TEXT PRIMARY KEY,
  grant_ref TEXT NOT NULL REFERENCES artanis_standing_spend_grants (grant_ref),
  state TEXT NOT NULL CHECK (
    state IN ('proposed', 'paid', 'refused', 'blocked_over_cap')
  ),
  intended_amount_sat INTEGER NOT NULL CHECK (intended_amount_sat > 0),
  paid_amount_sat INTEGER,
  destination_source_ref TEXT NOT NULL,
  recipient_ref TEXT NOT NULL,
  rationale TEXT NOT NULL DEFAULT '',
  payment_ref TEXT,
  policy_applied TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_spend_decisions_day
  ON artanis_spend_decisions (created_at DESC);
