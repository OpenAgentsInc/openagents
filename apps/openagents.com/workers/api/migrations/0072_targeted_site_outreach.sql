CREATE TABLE IF NOT EXISTS targeted_site_campaigns (
  id TEXT PRIMARY KEY NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  operator_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  vertical TEXT,
  geography TEXT,
  source_authority_ref TEXT NOT NULL,
  budget_cap_ref TEXT,
  suppression_policy_ref TEXT,
  operator_state TEXT NOT NULL CHECK (
    operator_state IN (
      'draft',
      'active',
      'paused',
      'reviewing',
      'completed',
      'archived'
    )
  ),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_campaigns_owner
  ON targeted_site_campaigns(owner_user_id, operator_state, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_campaigns_operator
  ON targeted_site_campaigns(operator_user_id, operator_state, updated_at DESC)
  WHERE operator_user_id IS NOT NULL AND archived_at IS NULL;

CREATE TABLE IF NOT EXISTS targeted_site_prospects (
  id TEXT PRIMARY KEY NOT NULL,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  idempotency_key TEXT NOT NULL UNIQUE,
  normalized_domain TEXT NOT NULL,
  origin_url TEXT,
  company_name TEXT,
  site_name TEXT,
  contact_refs_json TEXT NOT NULL DEFAULT '[]',
  vertical TEXT,
  geography TEXT,
  source_ref TEXT NOT NULL,
  discovery_confidence NUMERIC NOT NULL DEFAULT 0 CHECK (
    discovery_confidence >= 0 AND discovery_confidence <= 1
  ),
  suppression_state TEXT NOT NULL CHECK (
    suppression_state IN (
      'unknown',
      'clear',
      'suppressed',
      'manual_review'
    )
  ),
  capture_state TEXT NOT NULL CHECK (
    capture_state IN (
      'not_started',
      'policy_pending',
      'allowed',
      'blocked',
      'captured',
      'archived'
    )
  ),
  review_state TEXT NOT NULL CHECK (
    review_state IN (
      'pending',
      'ready',
      'approved',
      'skipped',
      'archived'
    )
  ),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  discovered_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(campaign_id, normalized_domain)
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_prospects_campaign
  ON targeted_site_prospects(campaign_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_prospects_domain
  ON targeted_site_prospects(normalized_domain, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_prospects_suppression
  ON targeted_site_prospects(campaign_id, suppression_state, updated_at DESC)
  WHERE archived_at IS NULL;
