CREATE TABLE IF NOT EXISTS order_github_write_authority_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  repository_full_name TEXT NOT NULL,
  repository_private INTEGER NOT NULL CHECK (repository_private IN (0, 1)),
  requested_operation TEXT NOT NULL CHECK (
    requested_operation IN (
      'create_branch',
      'push_commit',
      'open_pull_request',
      'open_fork_pull_request'
    )
  ),
  decision TEXT NOT NULL CHECK (decision IN ('allowed', 'blocked')),
  authority_mode TEXT CHECK (
    authority_mode IS NULL OR authority_mode IN (
      'customer_grant',
      'openagents_fork',
      'openagents_app'
    )
  ),
  blocked_reason TEXT CHECK (
    blocked_reason IS NULL OR blocked_reason IN (
      'explicit_approval_required',
      'source_access_required',
      'github_write_connection_required',
      'github_write_grant_required',
      'github_write_grant_expired',
      'github_write_grant_not_issued',
      'github_write_connection_unusable',
      'github_write_permission_missing',
      'openagents_app_not_configured',
      'unsupported_repository'
    )
  ),
  connection_ref TEXT,
  grant_ref TEXT,
  approval_source TEXT CHECK (
    approval_source IS NULL OR approval_source IN (
      'customer_action',
      'operator_action',
      'system_policy'
    )
  ),
  approved_at TEXT,
  customer_message TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS order_github_write_authority_receipts_order_created_idx
  ON order_github_write_authority_receipts(software_order_id, created_at DESC);

CREATE INDEX IF NOT EXISTS order_github_write_authority_receipts_assignment_created_idx
  ON order_github_write_authority_receipts(assignment_id, created_at DESC)
  WHERE assignment_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS order_github_write_authority_receipts_decision_created_idx
  ON order_github_write_authority_receipts(decision, created_at DESC);
