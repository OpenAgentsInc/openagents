PRAGMA defer_foreign_keys=ON;

ALTER TABLE business_service_promises
  RENAME TO business_service_promises_0275_old;

CREATE TABLE business_service_promises (
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

INSERT INTO business_service_promises (
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
SELECT
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
FROM business_service_promises_0275_old;

ALTER TABLE business_fulfillment_motion_receipts
  RENAME TO business_fulfillment_motion_receipts_0275_old;

CREATE TABLE business_fulfillment_motion_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  promise_id TEXT NOT NULL
    REFERENCES business_service_promises(id) ON DELETE CASCADE,
  promise_ref TEXT NOT NULL,
  motion_date TEXT NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  agent_definition_ref TEXT NOT NULL,
  crm_state_ref TEXT NOT NULL,
  stakeholder_refs_json TEXT NOT NULL DEFAULT '[]',
  stakeholder_flag_refs_json TEXT NOT NULL DEFAULT '[]',
  cadence TEXT NOT NULL DEFAULT 'daily' CHECK (cadence IN ('daily', 'weekly')),
  forward_motion_ref TEXT NOT NULL,
  customer_workroom_update_ref TEXT NOT NULL,
  client_comms_ledger_ref TEXT NOT NULL,
  client_comms_draft_ref TEXT NOT NULL,
  approval_gate_ref TEXT NOT NULL,
  outbound_allowed INTEGER NOT NULL DEFAULT 0 CHECK (outbound_allowed IN (0, 1)),
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  UNIQUE (promise_id, cadence, motion_date)
);

INSERT INTO business_fulfillment_motion_receipts (
  id,
  promise_id,
  promise_ref,
  motion_date,
  receipt_ref,
  agent_definition_ref,
  crm_state_ref,
  stakeholder_refs_json,
  stakeholder_flag_refs_json,
  cadence,
  forward_motion_ref,
  customer_workroom_update_ref,
  client_comms_ledger_ref,
  client_comms_draft_ref,
  approval_gate_ref,
  outbound_allowed,
  blocker_refs_json,
  source_refs_json,
  created_at
)
SELECT
  id,
  promise_id,
  promise_ref,
  motion_date,
  receipt_ref,
  agent_definition_ref,
  crm_state_ref,
  stakeholder_refs_json,
  stakeholder_flag_refs_json,
  'daily',
  forward_motion_ref,
  'workroom_update.business_fulfillment.customer_visible.legacy',
  'email_ledger.business_fulfillment.daily.client_comms.legacy',
  client_comms_draft_ref,
  approval_gate_ref,
  outbound_allowed,
  blocker_refs_json,
  source_refs_json,
  created_at
FROM business_fulfillment_motion_receipts_0275_old;

DROP TABLE business_fulfillment_motion_receipts_0275_old;
DROP TABLE business_service_promises_0275_old;

CREATE INDEX IF NOT EXISTS idx_business_service_promises_due
  ON business_service_promises(state, next_motion_due_at, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_business_fulfillment_motion_receipts_promise
  ON business_fulfillment_motion_receipts(promise_id, created_at DESC);

PRAGMA defer_foreign_keys=OFF;
