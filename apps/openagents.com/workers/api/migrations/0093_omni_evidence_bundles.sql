CREATE TABLE IF NOT EXISTS omni_evidence_bundles (
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
    status IN (
      'draft',
      'ready',
      'redaction_required',
      'superseded',
      'archived'
    )
  ),
  legal_sensitive INTEGER NOT NULL DEFAULT 0,
  summary_ref TEXT NOT NULL,
  source_authority_caveat_ref TEXT,
  entries_json TEXT NOT NULL DEFAULT '[]',
  public_receipt_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_evidence_bundles_workroom_updated
  ON omni_evidence_bundles(workroom_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_evidence_bundles_kind_status
  ON omni_evidence_bundles(work_kind, status, updated_at DESC)
  WHERE archived_at IS NULL;
