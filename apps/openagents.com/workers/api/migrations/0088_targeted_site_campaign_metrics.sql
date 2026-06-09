CREATE TABLE IF NOT EXISTS targeted_site_campaign_metric_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT,
  event_kind TEXT NOT NULL CHECK (
    event_kind IN (
      'capture_cost',
      'preview_generated',
      'outreach_sent',
      'email_bounced',
      'email_replied',
      'meeting_booked',
      'customer_converted',
      'accepted_outcome',
      'refund',
      'complaint',
      'suppressed',
      'blocked'
    )
  ),
  quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity >= 0),
  cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (cost_cents >= 0),
  public_ref TEXT,
  source_ref TEXT NOT NULL,
  related_event_id TEXT REFERENCES targeted_site_campaign_metric_events(id) ON DELETE SET NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  occurred_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    event_kind NOT IN ('refund', 'complaint')
    OR related_event_id IS NOT NULL
  )
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_campaign_metric_campaign
  ON targeted_site_campaign_metric_events(campaign_id, occurred_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_campaign_metric_prospect
  ON targeted_site_campaign_metric_events(prospect_id, occurred_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_campaign_metric_kind
  ON targeted_site_campaign_metric_events(campaign_id, event_kind, occurred_at DESC)
  WHERE archived_at IS NULL;
