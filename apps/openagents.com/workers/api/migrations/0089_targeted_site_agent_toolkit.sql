CREATE TABLE IF NOT EXISTS targeted_site_agent_toolkit_grants (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  agent_ref TEXT NOT NULL,
  scopes_json TEXT NOT NULL DEFAULT '[]',
  dry_run_default INTEGER NOT NULL DEFAULT 1 CHECK (dry_run_default IN (0, 1)),
  spend_cap_cents INTEGER NOT NULL DEFAULT 0 CHECK (spend_cap_cents >= 0),
  daily_send_cap INTEGER NOT NULL DEFAULT 0 CHECK (daily_send_cap >= 0),
  suppression_policy_ref TEXT,
  approval_policy TEXT NOT NULL CHECK (
    approval_policy IN (
      'operator_approval',
      'owner_approval',
      'auto_dry_run_only'
    )
  ),
  status TEXT NOT NULL CHECK (status IN ('active', 'revoked', 'expired')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT,
  revoked_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_agent_toolkit_grants_campaign
  ON targeted_site_agent_toolkit_grants(campaign_id, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_agent_toolkit_grants_owner
  ON targeted_site_agent_toolkit_grants(owner_user_id, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS targeted_site_agent_toolkit_actions (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  grant_id TEXT NOT NULL
    REFERENCES targeted_site_agent_toolkit_grants(id) ON DELETE CASCADE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  agent_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL CHECK (
    action_kind IN (
      'discover_prospects',
      'capture_site',
      'audit_site',
      'generate_preview',
      'send_outreach_request',
      'record_metric',
      'propose_reward'
    )
  ),
  dry_run INTEGER NOT NULL CHECK (dry_run IN (0, 1)),
  requested_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (requested_cost_cents >= 0),
  requested_send_count INTEGER NOT NULL DEFAULT 0 CHECK (requested_send_count >= 0),
  suppression_state TEXT NOT NULL CHECK (
    suppression_state IN ('unknown', 'clear', 'suppressed', 'manual_review')
  ),
  approval_state TEXT NOT NULL CHECK (
    approval_state IN (
      'not_required',
      'requested',
      'approved',
      'rejected'
    )
  ),
  result_state TEXT NOT NULL CHECK (
    result_state IN ('accepted', 'blocked', 'rejected')
  ),
  receipt_ref TEXT NOT NULL,
  reason TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_agent_toolkit_actions_grant
  ON targeted_site_agent_toolkit_actions(grant_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_agent_toolkit_actions_campaign
  ON targeted_site_agent_toolkit_actions(campaign_id, action_kind, created_at DESC)
  WHERE archived_at IS NULL;
