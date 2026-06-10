CREATE TABLE IF NOT EXISTS orange_check_entitlements (
  id TEXT PRIMARY KEY,
  agent_user_id TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK (state IN ('active', 'revoked')),
  receipt_ref TEXT NOT NULL UNIQUE,
  action_ref TEXT,
  paid_amount_cents INTEGER NOT NULL DEFAULT 500,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS orange_check_entitlements_state_idx
  ON orange_check_entitlements(state, updated_at DESC);
