CREATE TABLE IF NOT EXISTS omni_workroom_lifecycle_decisions (
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
  actor_kind TEXT NOT NULL CHECK (
    actor_kind IN ('customer', 'operator', 'system')
  ),
  decision_kind TEXT NOT NULL CHECK (
    decision_kind IN (
      'accept',
      'reject',
      'provisionally_accept',
      'reopen',
      'request_revision',
      'mark_unavailable'
    )
  ),
  resulting_state TEXT NOT NULL CHECK (
    resulting_state IN (
      'accepted',
      'rejected',
      'provisionally_accepted',
      'reopened',
      'revision_requested',
      'unavailable'
    )
  ),
  customer_safe_explanation_ref TEXT NOT NULL,
  receipt_ref TEXT NOT NULL,
  site_revision_feedback_ref TEXT,
  followup_request_ref TEXT,
  artifact_ref TEXT,
  no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_workroom_lifecycle_workroom_created
  ON omni_workroom_lifecycle_decisions(workroom_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_workroom_lifecycle_state_created
  ON omni_workroom_lifecycle_decisions(resulting_state, created_at DESC)
  WHERE archived_at IS NULL;
