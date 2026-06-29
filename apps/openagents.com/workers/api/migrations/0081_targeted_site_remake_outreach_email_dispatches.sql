CREATE TABLE IF NOT EXISTS targeted_site_remake_outreach_email_dispatches (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  preview_generation_id TEXT NOT NULL
    REFERENCES targeted_site_remake_preview_generations(id) ON DELETE RESTRICT,
  operator_review_event_id TEXT NOT NULL
    REFERENCES targeted_site_operator_review_events(id) ON DELETE RESTRICT,
  email_message_id TEXT,
  recipient_ref TEXT NOT NULL,
  template_slug TEXT NOT NULL,
  suppression_state TEXT NOT NULL CHECK (
    suppression_state IN (
      'unknown',
      'clear',
      'suppressed',
      'manual_review'
    )
  ),
  dispatch_state TEXT NOT NULL CHECK (
    dispatch_state IN (
      'accepted',
      'failed',
      'blocked',
      'skipped'
    )
  ),
  error_name TEXT,
  error_message TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  dispatched_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_outreach_email_campaign
  ON targeted_site_remake_outreach_email_dispatches(
    campaign_id,
    dispatch_state,
    dispatched_at DESC
  )
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_outreach_email_preview
  ON targeted_site_remake_outreach_email_dispatches(preview_generation_id, dispatched_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_outreach_email_domain
  ON targeted_site_remake_outreach_email_dispatches(normalized_domain, dispatched_at DESC)
  WHERE archived_at IS NULL;
