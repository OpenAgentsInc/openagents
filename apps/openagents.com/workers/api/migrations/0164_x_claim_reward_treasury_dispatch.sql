ALTER TABLE x_claim_reward_ledger
  ADD COLUMN treasury_payment_id TEXT;

CREATE INDEX IF NOT EXISTS x_claim_reward_ledger_dispatch_requested_idx
  ON x_claim_reward_ledger(state, updated_at ASC)
  WHERE state = 'dispatch_requested';

CREATE INDEX IF NOT EXISTS x_claim_reward_ledger_pending_treasury_payment_idx
  ON x_claim_reward_ledger(state, treasury_payment_id, updated_at ASC)
  WHERE state = 'dispatched' AND treasury_payment_id IS NOT NULL;
