CREATE TABLE IF NOT EXISTS site_referral_payout_ledger_entries (
  id TEXT PRIMARY KEY NOT NULL,
  payout_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  referral_attribution_id TEXT NOT NULL
    REFERENCES referral_attributions(id) ON DELETE RESTRICT,
  referral_source_id TEXT NOT NULL
    REFERENCES site_referral_sources(id) ON DELETE RESTRICT,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  referrer_user_id TEXT NOT NULL,
  referred_user_id TEXT,
  qualifying_event_ref TEXT NOT NULL,
  qualifying_event_kind TEXT NOT NULL,
  qualifying_amount_sats INTEGER NOT NULL DEFAULT 0 CHECK (qualifying_amount_sats >= 0),
  amount_sats INTEGER NOT NULL,
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
  previous_entry_id TEXT REFERENCES site_referral_payout_ledger_entries(id),
  reversal_of_entry_id TEXT REFERENCES site_referral_payout_ledger_entries(id),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS site_referral_payout_ledger_payout_idx
  ON site_referral_payout_ledger_entries(payout_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_referral_payout_ledger_referrer_period_idx
  ON site_referral_payout_ledger_entries(referrer_user_id, period_key, state)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_referral_payout_ledger_attribution_idx
  ON site_referral_payout_ledger_entries(referral_attribution_id, created_at DESC)
  WHERE archived_at IS NULL;
