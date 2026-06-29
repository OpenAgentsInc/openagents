CREATE TABLE IF NOT EXISTS blueprint_action_submissions (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  action_kind TEXT NOT NULL,
  approval_policy_ref TEXT NOT NULL,
  approval_receipt_ref TEXT,
  approval_state TEXT NOT NULL,
  approved_by_ref TEXT,
  content_redacted INTEGER NOT NULL DEFAULT 1,
  context_pack_refs_json TEXT NOT NULL DEFAULT '[]',
  direct_execution INTEGER NOT NULL DEFAULT 0,
  direct_program_run_execution_allowed INTEGER NOT NULL DEFAULT 0,
  dry_run_receipt_ref TEXT,
  dry_run_required INTEGER NOT NULL DEFAULT 1,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  execution_receipt_ref TEXT,
  failure_ref TEXT,
  model_confidence_bypass_disabled INTEGER NOT NULL DEFAULT 1,
  program_run_authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  proposal_only INTEGER NOT NULL DEFAULT 1,
  proposed_by_program_run_id TEXT NOT NULL,
  proposed_effect_ref TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL DEFAULT '[]',
  source_authority_refs_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL,
  summary_ref TEXT NOT NULL,
  tool_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_blueprint_action_submissions_program_run
  ON blueprint_action_submissions(proposed_by_program_run_id, archived_at);

CREATE INDEX IF NOT EXISTS idx_blueprint_action_submissions_status
  ON blueprint_action_submissions(status, archived_at);

CREATE INDEX IF NOT EXISTS idx_blueprint_action_submissions_action_kind
  ON blueprint_action_submissions(action_kind, archived_at);
