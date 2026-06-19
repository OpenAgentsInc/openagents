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

-- 1. Extend the pay_ins type CHECK to allow `usd_credit_grant`. SQLite cannot
--    alter a CHECK in place, so rebuild the table preserving rows + constraints.
PRAGMA foreign_keys = off;

CREATE TABLE pay_ins_next (
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
  state_changed_at TEXT NOT NULL
);

INSERT INTO pay_ins_next
SELECT id, pay_in_type, payer_ref, cost_msat, state, failure_reason, rung,
       context_ref, idempotency_key, genesis_id, successor_id, created_at,
       state_changed_at
FROM pay_ins;

DROP TABLE pay_ins;

ALTER TABLE pay_ins_next RENAME TO pay_ins;

CREATE INDEX IF NOT EXISTS idx_pay_ins_state ON pay_ins (state);
CREATE INDEX IF NOT EXISTS idx_pay_ins_payer ON pay_ins (payer_ref);

PRAGMA foreign_keys = on;

-- 2. USD-origin (non-Bitcoin-withdrawable) portion of the agent balance.
ALTER TABLE agent_balances
  ADD COLUMN usd_credit_msat INTEGER NOT NULL DEFAULT 0 CHECK (usd_credit_msat >= 0);
