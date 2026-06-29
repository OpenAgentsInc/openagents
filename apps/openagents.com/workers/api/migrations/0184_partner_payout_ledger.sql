CREATE TABLE IF NOT EXISTS partner_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  partner_role TEXT NOT NULL CHECK (
    partner_role IN (
      'design_partner',
      'referral',
      'affiliate'
    )
  ),
  partner_user_id TEXT NOT NULL,
  partner_ref TEXT NOT NULL,
  beneficiary_user_id TEXT,
  asset TEXT NOT NULL CHECK (
    asset IN (
      'usd',
      'credits',
      'sats'
    )
  ),
  qualifying_event_ref TEXT NOT NULL,
  qualifying_event_kind TEXT NOT NULL,
  qualifying_amount INTEGER NOT NULL DEFAULT 0 CHECK (qualifying_amount >= 0),
  amount INTEGER NOT NULL,
  period_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'eligible',
      'approved',
      'dispatched',
      'settled',
      'failed',
      'refused',
      'reversed'
    )
  ),
  state_reason_ref TEXT,
  previous_entry_id TEXT REFERENCES partner_payout_ledger_entries(id),
  reversal_of_entry_id TEXT REFERENCES partner_payout_ledger_entries(id),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS partner_payout_ledger_payout_idx
  ON partner_payout_ledger_entries(payout_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS partner_payout_ledger_partner_period_idx
  ON partner_payout_ledger_entries(partner_user_id, period_key, state)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS partner_payout_ledger_partner_ref_idx
  ON partner_payout_ledger_entries(partner_role, partner_ref, created_at DESC)
  WHERE archived_at IS NULL;
