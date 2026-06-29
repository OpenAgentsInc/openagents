CREATE TABLE IF NOT EXISTS omni_workrooms (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  software_order_id TEXT NOT NULL
    REFERENCES software_orders(id) ON DELETE CASCADE,
  accepted_outcome_contract_id TEXT
    REFERENCES omni_accepted_outcome_contracts(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  work_kind TEXT NOT NULL CHECK (
    work_kind IN (
      'site',
      'coding',
      'adjustment',
      'existing_project_import',
      'business',
      'legal_sensitive'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN (
      'queued',
      'active',
      'blocked',
      'waiting_review',
      'completed',
      'unavailable',
      'archived'
    )
  ),
  visibility TEXT NOT NULL CHECK (
    visibility IN ('private', 'customer', 'team', 'public')
  ),
  customer_intent_ref TEXT NOT NULL,
  task_packet_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  email_refs_json TEXT NOT NULL DEFAULT '[]',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  public_receipt_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_omni_workrooms_order_active
  ON omni_workrooms(software_order_id)
  WHERE archived_at IS NULL
    AND status != 'archived';

CREATE INDEX IF NOT EXISTS idx_omni_workrooms_site_updated
  ON omni_workrooms(site_id, updated_at DESC)
  WHERE site_id IS NOT NULL
    AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_workrooms_assignment_updated
  ON omni_workrooms(assignment_id, updated_at DESC)
  WHERE assignment_id IS NOT NULL
    AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_workrooms_kind_status
  ON omni_workrooms(work_kind, status, updated_at DESC)
  WHERE archived_at IS NULL;
