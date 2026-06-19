-- USD -> msat credit bridge (#5497): close the paid-inference revenue loop.
--
-- A card (Stripe) purchase raises the USD `billing_ledger_entries` balance, but
-- the inference gateway meters the separate msat `agent_balances` ledger funded
-- only by Lightning. This migration adds the bridge primitives:
--
--   1. A new `usd_credit_grant` pay-in type on `pay_ins`, representing a
--      USD-purchased, inference-spendable credit grant into a user's agent
--      balance (debited from the user's USD ledger).
--
--   2. A `usd_credit_msat` column on `agent_balances` tracking the USD-ORIGIN
--      portion of the balance. RL-3 ASSET-BOUNDARY (openagents #5460): a card
--      dollar is a USD liability, NOT Bitcoin. USD-funded msat is
--      inference-spendable (it lives in `balance_msat`, the gate/metering read)
--      but it is NOT Bitcoin-withdrawable: the Lightning sweep (tips-sweep.ts)
--      subtracts `usd_credit_msat` from the sweepable amount so a USD-purchased
--      credit can never leak into a real Bitcoin payout. Bitcoin-funded value is
--      `balance_msat - held_msat - usd_credit_msat`.
--
-- FK-SAFE REBUILD (#5497 prod-migration repair): the prior shape of this
-- migration dropped `pay_ins` and recreated it from a `pay_ins_next` scratch
-- table while relying on `PRAGMA foreign_keys = off`. That pragma is silently
-- ignored inside the implicit transaction `wrangler d1 migrations apply` wraps
-- around each migration, so on a real database the `DROP TABLE pay_ins` fails
-- with FOREIGN KEY constraint failed, because `pay_in_legs.pay_in_id`
-- REFERENCES pay_ins (id) and production carries live legs. The fresh staging
-- DB happened to have zero `pay_in_legs` rows, which masked the bug. The prior
-- shape also silently dropped the `public_receipt_ref` column and its index
-- added by migration 0169, which the live ledger code (payments-ledger.ts,
-- tips-sweep.ts, tip-ladder.ts, forum receipts) requires.
--
-- Standard 12-step table redefinition: rename the parent to `_old` first.
-- SQLite's (non-legacy) ALTER TABLE RENAME rewrites the child FK in
-- `pay_in_legs` to follow the rename, so we must rebuild `pay_in_legs` too to
-- re-point its FK at the new `pay_ins` before dropping `pay_ins_old`. Every
-- column, the UNIQUE constraint, the extra `public_receipt_ref` column, and all
-- indexes are preserved verbatim; only the `pay_in_type` CHECK gains
-- `usd_credit_grant`.

-- 1. Extend the pay_ins type CHECK to allow `usd_credit_grant`.
ALTER TABLE pay_ins RENAME TO pay_ins_old;

CREATE TABLE pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN (
      'tip', 'sweep', 'buffer_funding', 'reward', 'adjustment', 'usd_credit_grant'
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
-- rewrote it to point at pay_ins_old). pay_in_legs is a leaf table; nothing
-- references it, so this rebuild is safe.
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

-- Recreate every index that lived on the rebuilt tables (0160 + 0169).
CREATE INDEX IF NOT EXISTS idx_pay_ins_state ON pay_ins (state);
CREATE INDEX IF NOT EXISTS idx_pay_ins_payer ON pay_ins (payer_ref);
CREATE INDEX IF NOT EXISTS idx_pay_ins_public_receipt_ref
  ON pay_ins (public_receipt_ref)
  WHERE public_receipt_ref IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_pay_in ON pay_in_legs (pay_in_id);
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_party ON pay_in_legs (party_ref);

-- 2. USD-origin (non-Bitcoin-withdrawable) portion of the agent balance.
ALTER TABLE agent_balances
  ADD COLUMN usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0);
