CREATE TABLE IF NOT EXISTS business_service_promises (
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
  cadence TEXT NOT NULL CHECK (cadence IN ('daily')),
  next_motion_due_at TEXT,
  last_motion_receipt_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_service_promises_due
  ON business_service_promises(state, next_motion_due_at, updated_at DESC);

CREATE TABLE IF NOT EXISTS business_fulfillment_motion_receipts (
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
  forward_motion_ref TEXT NOT NULL,
  client_comms_draft_ref TEXT NOT NULL,
  approval_gate_ref TEXT NOT NULL,
  outbound_allowed INTEGER NOT NULL DEFAULT 0 CHECK (outbound_allowed IN (0, 1)),
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  UNIQUE (promise_id, motion_date)
);

CREATE INDEX IF NOT EXISTS idx_business_fulfillment_motion_receipts_promise
  ON business_fulfillment_motion_receipts(promise_id, created_at DESC);
