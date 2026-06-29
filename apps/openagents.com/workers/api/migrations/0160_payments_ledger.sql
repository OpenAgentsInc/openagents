-- Agent credit ledger: sweepable balances on a PayIn-shaped ledger
-- (issue #4705; design: docs/payments/reliable-tips.md; promise
-- payments.reliable_tips_sweepable_balances.v1).
--
-- Discipline carried from the Stacker News audit:
-- - balances change only by increment/decrement, never read-then-write
-- - every paid attempt is one pay_ins row created atomically (D1 batch)
--   with the legs that fund it and the legs that say where value goes
-- - balance-touching legs store the resulting balance for built-in audit
-- - FAILED refunds funding debits atomically with the state change
-- - retries chain through genesis_id/successor_id with a set-if-null
--   optimistic lock so no attempt can be retried twice

CREATE TABLE IF NOT EXISTS agent_balances (
  actor_ref TEXT PRIMARY KEY,
  balance_msat INTEGER NOT NULL DEFAULT 0 CHECK (balance_msat >= 0),
  sweep_enabled INTEGER NOT NULL DEFAULT 1,
  sweep_threshold_sat INTEGER NOT NULL DEFAULT 210,
  send_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  receive_credits_below_sat INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pay_ins (
  id TEXT PRIMARY KEY,
  pay_in_type TEXT NOT NULL CHECK (
    pay_in_type IN ('tip', 'sweep', 'buffer_funding', 'reward', 'adjustment')
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

CREATE TABLE IF NOT EXISTS pay_in_legs (
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

CREATE INDEX IF NOT EXISTS idx_pay_ins_state ON pay_ins (state);
CREATE INDEX IF NOT EXISTS idx_pay_ins_payer ON pay_ins (payer_ref);
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_pay_in ON pay_in_legs (pay_in_id);
CREATE INDEX IF NOT EXISTS idx_pay_in_legs_party ON pay_in_legs (party_ref);
CREATE INDEX IF NOT EXISTS idx_agent_balances_sweep
  ON agent_balances (sweep_enabled, balance_msat);
