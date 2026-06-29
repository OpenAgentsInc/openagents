CREATE TABLE IF NOT EXISTS targeted_site_capture_policy_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (
    decision IN (
      'allowed',
      'disallowed',
      'blocked',
      'manual_review',
      'customer_owned',
      'suppressed',
      'paid_escalation'
    )
  ),
  fetchable INTEGER NOT NULL CHECK (
    fetchable IN (0, 1) AND (
      fetchable = 0 OR decision IN ('allowed', 'paid_escalation')
    )
  ),
  reason TEXT NOT NULL CHECK (
    reason IN (
      'robots_allowed',
      'robots_disallowed',
      'robots_unavailable',
      'sitemap_available',
      'suppression_match',
      'customer_owned_domain',
      'contact_suppressed',
      'operator_manual_review',
      'paid_provider_required',
      'bot_protection_or_login',
      'unsupported_scheme',
      'unsafe_domain',
      'policy_override'
    )
  ),
  robots_ref TEXT,
  sitemap_ref TEXT,
  suppression_ref TEXT,
  customer_authority_ref TEXT,
  paid_escalation_ref TEXT,
  operator_actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  operator_note_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  decided_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_policy_campaign
  ON targeted_site_capture_policy_events(
    campaign_id,
    decision,
    decided_at DESC
  )
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_policy_prospect
  ON targeted_site_capture_policy_events(prospect_id, decided_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_policy_domain
  ON targeted_site_capture_policy_events(
    normalized_domain,
    decision,
    decided_at DESC
  )
  WHERE archived_at IS NULL;
