-- Add the validator-only forfeitable terminal state to labor escrow.
-- This is still the credit ledger: it moves held claims inside agent_balances
-- and does not create Lightning/on-chain settlement evidence.
PRAGMA defer_foreign_keys=ON;

CREATE TABLE labor_escrows_0261_new (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  state TEXT NOT NULL CHECK (
    state IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  funding_source TEXT NOT NULL DEFAULT 'ledger_balance' CHECK (
    funding_source IN ('ledger_balance', 'external_invoice_pending')
  ),
  job_event_id TEXT NOT NULL,
  acceptance_event_ref TEXT,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  release_receipt_ref TEXT UNIQUE,
  refund_receipt_ref TEXT UNIQUE,
  forfeit_receipt_ref TEXT UNIQUE,
  forfeit_destination TEXT CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref TEXT,
  forfeit_condition_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  released_at TEXT,
  refunded_at TEXT,
  forfeited_at TEXT,
  archived_at TEXT
);

INSERT INTO labor_escrows_0261_new (
  id,
  idempotency_key,
  work_request_id,
  requester_actor_ref,
  provider_actor_ref,
  amount_msat,
  state,
  funding_source,
  job_event_id,
  acceptance_event_ref,
  reserve_receipt_ref,
  release_receipt_ref,
  refund_receipt_ref,
  forfeit_receipt_ref,
  forfeit_destination,
  forfeit_destination_actor_ref,
  forfeit_condition_ref,
  public_projection_json,
  created_at,
  updated_at,
  released_at,
  refunded_at,
  forfeited_at,
  archived_at
)
SELECT
  id,
  idempotency_key,
  work_request_id,
  requester_actor_ref,
  provider_actor_ref,
  amount_msat,
  state,
  funding_source,
  job_event_id,
  acceptance_event_ref,
  reserve_receipt_ref,
  release_receipt_ref,
  refund_receipt_ref,
  NULL,
  NULL,
  NULL,
  NULL,
  public_projection_json,
  created_at,
  updated_at,
  released_at,
  refunded_at,
  NULL,
  archived_at
FROM labor_escrows;

DROP TABLE labor_escrows;

ALTER TABLE labor_escrows_0261_new RENAME TO labor_escrows;

CREATE INDEX IF NOT EXISTS idx_labor_escrows_work_request
  ON labor_escrows(work_request_id)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_labor_escrows_state_created
  ON labor_escrows(state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE labor_escrow_receipts_0261_new (
  id TEXT PRIMARY KEY NOT NULL,
  escrow_id TEXT NOT NULL REFERENCES labor_escrows(id) ON DELETE CASCADE,
  idempotency_key TEXT NOT NULL UNIQUE,
  transition_kind TEXT NOT NULL CHECK (
    transition_kind IN ('reserve', 'release', 'refund', 'forfeit')
  ),
  work_request_id TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT,
  amount_msat INTEGER NOT NULL CHECK (amount_msat > 0),
  receipt_ref TEXT NOT NULL UNIQUE,
  evidence_ref TEXT,
  state_after TEXT NOT NULL CHECK (
    state_after IN ('reserved', 'released_to_provider', 'refunded', 'forfeited')
  ),
  forfeit_destination TEXT CHECK (
    forfeit_destination IS NULL OR forfeit_destination IN ('counterparty', 'burn')
  ),
  forfeit_destination_actor_ref TEXT,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

INSERT INTO labor_escrow_receipts_0261_new (
  id,
  escrow_id,
  idempotency_key,
  transition_kind,
  work_request_id,
  requester_actor_ref,
  provider_actor_ref,
  amount_msat,
  receipt_ref,
  evidence_ref,
  state_after,
  forfeit_destination,
  forfeit_destination_actor_ref,
  public_projection_json,
  created_at
)
SELECT
  id,
  escrow_id,
  idempotency_key,
  transition_kind,
  work_request_id,
  requester_actor_ref,
  provider_actor_ref,
  amount_msat,
  receipt_ref,
  evidence_ref,
  state_after,
  NULL,
  NULL,
  public_projection_json,
  created_at
FROM labor_escrow_receipts;

DROP TABLE labor_escrow_receipts;

ALTER TABLE labor_escrow_receipts_0261_new RENAME TO labor_escrow_receipts;

CREATE UNIQUE INDEX IF NOT EXISTS idx_labor_escrow_receipts_once
  ON labor_escrow_receipts(escrow_id, transition_kind);

CREATE INDEX IF NOT EXISTS idx_labor_escrow_receipts_work_request
  ON labor_escrow_receipts(work_request_id, created_at DESC);

PRAGMA defer_foreign_keys=OFF;
