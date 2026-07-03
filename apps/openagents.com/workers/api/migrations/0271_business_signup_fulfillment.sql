-- Business signup fulfillment ledger (BF-1.1 / issue #8074).
--
-- A /business signup must not silently stop at the intake row. This migration
-- adds a small state projection on the signup row plus a receipt table for the
-- follow-up chain: enrichment hook, private project/workspace seed, invite, and
-- email ledger send. Parked is explicit and operator-visible.

ALTER TABLE business_signup_requests
  ADD COLUMN fulfillment_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (fulfillment_status IN ('pending', 'invited', 'operator_parked'));

ALTER TABLE business_signup_requests
  ADD COLUMN fulfillment_ref TEXT;

ALTER TABLE business_signup_requests
  ADD COLUMN fulfillment_reason TEXT;

CREATE INDEX IF NOT EXISTS business_signup_requests_fulfillment_status_idx
  ON business_signup_requests(fulfillment_status, updated_at DESC);

CREATE TABLE IF NOT EXISTS business_signup_fulfillments (
  id TEXT PRIMARY KEY NOT NULL,
  business_signup_request_id TEXT NOT NULL UNIQUE
    REFERENCES business_signup_requests(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('invited', 'operator_parked')),
  reason TEXT,
  enrichment_ref TEXT NOT NULL,
  team_id TEXT,
  project_id TEXT,
  workspace_id TEXT,
  invite_id TEXT,
  email_message_id TEXT,
  email_delivery_status TEXT NOT NULL CHECK (
    email_delivery_status IN (
      'accepted',
      'disabled',
      'failed',
      'missing_config',
      'not_attempted'
    )
  ),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE SET NULL,
  FOREIGN KEY (project_id) REFERENCES team_projects(id) ON DELETE SET NULL,
  FOREIGN KEY (workspace_id) REFERENCES prefilled_workspaces(id) ON DELETE SET NULL,
  FOREIGN KEY (invite_id) REFERENCES team_workspace_invites(id) ON DELETE SET NULL,
  FOREIGN KEY (email_message_id) REFERENCES email_messages(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS business_signup_fulfillments_status_idx
  ON business_signup_fulfillments(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS business_signup_fulfillments_workspace_idx
  ON business_signup_fulfillments(workspace_id)
  WHERE workspace_id IS NOT NULL;
