CREATE TABLE IF NOT EXISTS omni_accepted_outcome_contracts (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
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
  subject_ref TEXT NOT NULL,
  customer_ref TEXT,
  expected_artifacts_json TEXT NOT NULL DEFAULT '[]',
  review_policy TEXT NOT NULL CHECK (
    review_policy IN (
      'operator_review',
      'customer_review',
      'dual_review',
      'owner_review',
      'no_review'
    )
  ),
  acceptance_state TEXT NOT NULL CHECK (
    acceptance_state IN (
      'draft',
      'pending_review',
      'provisionally_accepted',
      'accepted',
      'rejected',
      'revision_requested',
      'reopened',
      'unavailable'
    )
  ),
  proof_policy TEXT NOT NULL CHECK (
    proof_policy IN (
      'private_receipt',
      'customer_safe_summary',
      'public_safe_proof',
      'legal_sensitive_private'
    )
  ),
  economic_state TEXT NOT NULL CHECK (
    economic_state IN (
      'free_beta',
      'paid_required',
      'credits_required',
      'sats_required',
      'internal_only'
    )
  ),
  closeout_requirements_json TEXT NOT NULL DEFAULT '[]',
  legal_sensitive INTEGER NOT NULL DEFAULT 0 CHECK (legal_sensitive IN (0, 1)),
  public_receipt_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_accepted_outcome_contracts_subject
  ON omni_accepted_outcome_contracts(subject_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_accepted_outcome_contracts_work_kind
  ON omni_accepted_outcome_contracts(work_kind, acceptance_state, updated_at DESC)
  WHERE archived_at IS NULL;
