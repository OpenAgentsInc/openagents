-- Labor escrow extends the agent credit ledger with held balances.
-- Escrow is a bounded claim on the existing 1:1 buffer-backed ledger:
-- reserve moves available balance into held state, release debits the
-- requester and credits the provider, and refund releases the hold.

ALTER TABLE agent_balances
  ADD COLUMN held_msat INTEGER NOT NULL DEFAULT 0 CHECK (held_msat >= 0);

CREATE TRIGGER IF NOT EXISTS agent_balances_available_insert
BEFORE INSERT ON agent_balances
WHEN NEW.balance_msat < NEW.held_msat
BEGIN
  SELECT RAISE(ABORT, 'agent_balance_available_nonnegative');
END;

CREATE TRIGGER IF NOT EXISTS agent_balances_available_update
BEFORE UPDATE OF balance_msat, held_msat ON agent_balances
WHEN NEW.balance_msat < NEW.held_msat
BEGIN
  SELECT RAISE(ABORT, 'agent_balance_available_nonnegative');
END;

CREATE INDEX IF NOT EXISTS idx_agent_balances_sweep_available
  ON agent_balances (sweep_enabled, balance_msat, held_msat);

CREATE TABLE IF NOT EXISTS labor_escrows (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('reserved', 'released_to_provider', 'refunded')
  ),
  funding_source TEXT NOT NULL DEFAULT 'ledger_balance' CHECK (
    funding_source IN ('ledger_balance', 'external_invoice_pending')
  ),
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_labor_escrows_work_request
  ON labor_escrows(work_request_id)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_labor_escrows_state_created
  ON labor_escrows(state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS labor_escrow_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  escrow_id TEXT NOT NULL REFERENCES labor_escrows(id) ON DELETE CASCADE,
  idempotency_key TEXT NOT NULL UNIQUE,
  transition_kind TEXT NOT NULL CHECK (
    transition_kind IN ('reserve', 'release', 'refund')
  ),
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_ref TEXT,
  state_after TEXT NOT NULL CHECK (
    state_after IN ('reserved', 'released_to_provider', 'refunded')
  ),
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_labor_escrow_receipts_once
  ON labor_escrow_receipts(escrow_id, transition_kind);

CREATE INDEX IF NOT EXISTS idx_labor_escrow_receipts_work_request
  ON labor_escrow_receipts(work_request_id, created_at DESC);
