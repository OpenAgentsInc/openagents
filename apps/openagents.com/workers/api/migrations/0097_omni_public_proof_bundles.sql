CREATE TABLE IF NOT EXISTS omni_public_proof_bundles (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL REFERENCES omni_workrooms(id) ON DELETE CASCADE,
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
    status IN ('draft', 'ready', 'blocked', 'superseded', 'archived')
  ),
  legal_sensitive INTEGER NOT NULL DEFAULT 0,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  review_state_ref TEXT NOT NULL,
  acceptance_state_ref TEXT NOT NULL,
  economics_caveat_ref TEXT NOT NULL,
  legal_caveat_ref TEXT,
  privacy_caveat_ref TEXT NOT NULL,
  public_receipt_ref TEXT NOT NULL,
  no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_public_proof_bundles_workroom_updated
  ON omni_public_proof_bundles(workroom_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_public_proof_bundles_kind_status
  ON omni_public_proof_bundles(work_kind, status, updated_at DESC)
  WHERE archived_at IS NULL;
