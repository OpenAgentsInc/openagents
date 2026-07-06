-- CFG-4 (#8519, epic #8515): credits-domain HARD cutover — Postgres becomes
-- the SOLE authority for `pay_ins` / `pay_in_legs` / `agent_balances` (and
-- the labor-escrow rows riding the same atomic ledger batches). This
-- migration adds the read accelerators the newly Postgres-served credit
-- reads need; the twins themselves landed in 0015 (billing) / 0016
-- (treasury) and are verified column-exact against D1 by the domain
-- backfill `--verify` sweeps.

-- Mobile credits transaction history + registered-agent self-view: keyset
-- pagination over (payer_ref, created_at DESC, id DESC), with an OR branch
-- on payout legs by (party_ref, direction).
CREATE INDEX IF NOT EXISTS pay_ins_payer_created_id_idx
  ON pay_ins (payer_ref, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS pay_in_legs_party_direction_pay_in_idx
  ON pay_in_legs (party_ref, direction, pay_in_id);

-- TipsSweep.runTick / reconcileForwarding scans: pending sweep dedupe by
-- (pay_in_type, state, payer_ref) and forwarding reconcile by state.
CREATE INDEX IF NOT EXISTS pay_ins_type_state_payer_idx
  ON pay_ins (pay_in_type, state, payer_ref);

CREATE INDEX IF NOT EXISTS pay_ins_state_created_idx
  ON pay_ins (state, created_at);
