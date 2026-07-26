-- FORGE-08 (#9250): D1 control-plane mirror of exact-tip merge receipts.
-- The private Git service uses the Cloud SQL counterpart as its transport
-- receipt store.  Keep this schema identical for control-plane reconciliation.

CREATE TABLE IF NOT EXISTS forge_git_merge_outcome_receipts (
  receipt_ref TEXT PRIMARY KEY,
  tenant_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  maintainer_binding_ref TEXT NOT NULL,
  target_ref TEXT NOT NULL,
  old_object_id TEXT NOT NULL,
  new_object_id TEXT NOT NULL,
  authority_generation INTEGER NOT NULL,
  policy_version TEXT NOT NULL,
  proposal_event_ids_json TEXT NOT NULL DEFAULT '[]',
  gate_results_json TEXT NOT NULL DEFAULT '[]',
  state TEXT NOT NULL CHECK (state IN ('prepared', 'finalized', 'refused')),
  decided_at TEXT NOT NULL,
  state_event_id TEXT UNIQUE,
  state_author_pubkey TEXT,
  state_signature TEXT,
  finalized_at TEXT,
  refused_at TEXT,
  refusal_code TEXT,
  CHECK (
    (state = 'prepared' AND state_event_id IS NULL AND finalized_at IS NULL)
    OR (state = 'finalized' AND state_event_id IS NOT NULL AND finalized_at IS NOT NULL)
    OR state = 'refused'
  ),
  UNIQUE (tenant_ref, repository_ref, target_ref, old_object_id, new_object_id, authority_generation)
);
CREATE INDEX IF NOT EXISTS idx_forge_git_merge_receipts_prepared
  ON forge_git_merge_outcome_receipts(tenant_ref, repository_ref, target_ref)
  WHERE state = 'prepared';
