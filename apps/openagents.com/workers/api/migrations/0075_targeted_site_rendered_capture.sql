CREATE TABLE IF NOT EXISTS targeted_site_rendered_capture_runs (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  capture_policy_event_id TEXT NOT NULL
    REFERENCES targeted_site_capture_policy_events(id) ON DELETE RESTRICT,
  static_capture_run_id TEXT
    REFERENCES targeted_site_static_capture_runs(id) ON DELETE SET NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'planned',
      'succeeded',
      'partial',
      'failed',
      'blocked',
      'manual_review',
      'archived'
    )
  ),
  reason TEXT NOT NULL CHECK (
    reason IN (
      'policy_fetchable',
      'policy_not_fetchable',
      'static_capture_insufficient',
      'screenshot_ready',
      'rendered_source_ready',
      'crawl_ready',
      'usage_limit',
      'network_error',
      'provider_error',
      'bot_protection_or_login',
      'manual_review'
    )
  ),
  target_url TEXT NOT NULL,
  provider_ref TEXT NOT NULL,
  screenshot_ref TEXT,
  rendered_html_ref TEXT,
  markdown_ref TEXT,
  links_ref TEXT,
  structured_json_ref TEXT,
  crawl_ref TEXT,
  viewport_ref TEXT,
  device_ref TEXT,
  usage_summary_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  started_at TEXT NOT NULL,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    state IN ('planned', 'blocked', 'manual_review', 'failed')
    OR screenshot_ref IS NOT NULL
    OR rendered_html_ref IS NOT NULL
    OR markdown_ref IS NOT NULL
    OR links_ref IS NOT NULL
    OR structured_json_ref IS NOT NULL
    OR crawl_ref IS NOT NULL
  )
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_rendered_capture_campaign
  ON targeted_site_rendered_capture_runs(campaign_id, state, started_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_rendered_capture_prospect
  ON targeted_site_rendered_capture_runs(prospect_id, started_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_rendered_capture_domain
  ON targeted_site_rendered_capture_runs(normalized_domain, state, started_at DESC)
  WHERE archived_at IS NULL;
