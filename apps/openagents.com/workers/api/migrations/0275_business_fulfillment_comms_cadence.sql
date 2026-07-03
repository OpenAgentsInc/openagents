-- BF-5.3: per-promise client comms cadence and customer-visible workroom
-- update refs. Outbound email remains approval-gated; this records the
-- ledger refs needed for customers to see forward motion without asking.

PRAGMA foreign_keys = OFF;

CREATE TABLE business_service_promises_0275 (
  id TEXT PRIMARY KEY NOT NULL,
  promise_ref TEXT NOT NULL UNIQUE,
  accepted_outcome_contract_id TEXT
    REFERENCES omni_accepted_outcome_contracts(id) ON DELETE SET NULL,
  workspace_ref TEXT NOT NULL,
  crm_state_ref TEXT NOT NULL,
  stakeholder_refs_json TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL CHECK (
    state IN ('active', 'paused', 'blocked', 'closed')
  ),
  cadence TEXT NOT NULL CHECK (cadence IN ('daily', 'weekly')),
  next_motion_due_at TEXT,
  last_motion_receipt_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO business_service_promises_0275 (
  id,
  promise_ref,
  accepted_outcome_contract_id,
  workspace_ref,
  crm_state_ref,
  stakeholder_refs_json,
  state,
  cadence,
  next_motion_due_at,
  last_motion_receipt_ref,
  source_refs_json,
  metadata_json,
  created_at,
  updated_at
)
SELECT id,
       promise_ref,
       accepted_outcome_contract_id,
       workspace_ref,
       crm_state_ref,
       stakeholder_refs_json,
       state,
       cadence,
       next_motion_due_at,
       last_motion_receipt_ref,
       source_refs_json,
       metadata_json,
       created_at,
       updated_at
  FROM business_service_promises;

DROP TABLE business_service_promises;
ALTER TABLE business_service_promises_0275 RENAME TO business_service_promises;

CREATE INDEX IF NOT EXISTS idx_business_service_promises_due
  ON business_service_promises(state, next_motion_due_at, updated_at DESC);

PRAGMA foreign_keys = ON;

ALTER TABLE business_fulfillment_motion_receipts
  ADD COLUMN cadence TEXT NOT NULL DEFAULT 'daily'
    CHECK (cadence IN ('daily', 'weekly'));

ALTER TABLE business_fulfillment_motion_receipts
  ADD COLUMN client_comms_email_ledger_ref TEXT;

ALTER TABLE business_fulfillment_motion_receipts
  ADD COLUMN customer_visible_workroom_update_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_business_fulfillment_motion_receipts_cadence
  ON business_fulfillment_motion_receipts(cadence, created_at DESC);
