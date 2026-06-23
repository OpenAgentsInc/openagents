-- Add the `lightning_charge` pay-in type (EPIC #6049, draft-lightning-charge-00).
--
-- WHY a new pay-in type: a settled MPP Lightning charge mints inference credit
-- from REAL Bitcoin inbound (a paid BOLT11 invoice), unlike the USDC/card MPP
-- rails which settle a USD/Stripe liability. The Bitcoin-origin credit must land
-- in `balance_msat` WITHOUT bumping `usd_credit_msat` (so it stays
-- Bitcoin-withdrawable per the RL-3 asset boundary — INVARIANTS
-- "Credit<->Bitcoin Asset Boundary"), and it must NOT pollute the forum `tip`
-- queries (tip-earnings, tip-ladder, sweep tip attribution). A dedicated
-- `lightning_charge` type keeps the accounting honest and queryable.
--
-- This rebuilds `pay_ins` (and the leaf `pay_in_legs`) to extend the
-- `pay_in_type` CHECK, copying the exact 0211 rebuild discipline so the FK and
-- every index are preserved verbatim. No data is transformed; only the CHECK
-- gains one allowed value.
--
-- INERT NOTE: nothing writes `lightning_charge` rows until the Lightning MPP
-- rail is armed (KHALA_MPP_LIGHTNING_ENABLED + KHALA_MPP_ENABLED + the MDK
-- wallet binding); this migration is data-model only.

ALTER TABLE pay_ins RENAME TO pay_ins_old;

CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN (
      'tip', 'sweep', 'buffer_funding', 'reward', 'adjustment',
      'usd_credit_grant', 'lightning_charge'
    )
  ),
  payer_ref TEXT NOT NULL,
  cost_msat INTEGER NOT NULL CHECK (cost_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('pending', 'forwarding', 'paid', 'failed')
  ),
  failure_reason TEXT,
  rung TEXT CHECK (rung IN ('credited', 'direct_bolt12') OR rung IS NULL),
  context_ref TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  genesis_id TEXT,
  successor_id TEXT,
  created_at TEXT NOT NULL,
  state_changed_at TEXT NOT NULL,
  public_receipt_ref TEXT
);

INSERT INTO pay_ins (
  id, pay_in_type, payer_ref, cost_msat, state, failure_reason, rung,
  context_ref, idempotency_key, genesis_id, successor_id, created_at,
  state_changed_at, public_receipt_ref
)
SELECT
  id, pay_in_type, payer_ref, cost_msat, state, failure_reason, rung,
  context_ref, idempotency_key, genesis_id, successor_id, created_at,
  state_changed_at, public_receipt_ref
FROM pay_ins_old;

-- Rebuild pay_in_legs so its FK targets the new pay_ins (the parent rename
-- rewrote it to point at pay_ins_old). pay_in_legs is a leaf table.
ALTER TABLE pay_in_legs RENAME TO pay_in_legs_old;

CREATE TABLE pay_in_legs (
  id TEXT PRIMARY KEY,
  pay_in_id TEXT NOT NULL REFERENCES pay_ins (id),
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  kind TEXT NOT NULL CHECK (kind IN ('balance', 'lightning')),
  party_ref TEXT NOT NULL,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  resulting_balance_msat INTEGER,
  external_ref TEXT,
  refund_of_leg_id TEXT,
  created_at TEXT NOT NULL
);

INSERT INTO pay_in_legs (
  id, pay_in_id, direction, kind, party_ref, amount_msat,
  resulting_balance_msat, external_ref, refund_of_leg_id, created_at
)
SELECT
  id, pay_in_id, direction, kind, party_ref, amount_msat,
  resulting_balance_msat, external_ref, refund_of_leg_id, created_at
FROM pay_in_legs_old;

DROP TABLE pay_in_legs_old;
DROP TABLE pay_ins_old;

-- Recreate every index that lived on the rebuilt tables (0160 + 0169 + 0211).
CREATE INDEX IF NOT EXISTS idx_pay_ins_state ON pay_ins (state);
CREATE INDEX IF NOT EXISTS idx_pay_ins_payer ON pay_ins (payer_ref);
CREATE INDEX IF NOT EXISTS idx_pay_ins_public_receipt_ref
  ON pay_ins (public_receipt_ref)
  WHERE public_receipt_ref IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_pay_in ON pay_in_legs (pay_in_id);
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_party ON pay_in_legs (party_ref);
