CREATE TABLE IF NOT EXISTS targeted_site_static_capture_runs (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  capture_policy_event_id TEXT NOT NULL
    REFERENCES targeted_site_capture_policy_events(id) ON DELETE RESTRICT,
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
      'homepage_fetched',
      'partial_pages',
      'network_error',
      'invalid_url',
      'cross_origin_url',
      'response_too_large',
      'unsupported_content_type',
      'robots_changed',
      'manual_review',
      'source_pack_ready'
    )
  ),
  homepage_url TEXT NOT NULL,
  homepage_ref TEXT,
  robots_ref TEXT,
  sitemap_ref TEXT,
  source_pack_ref TEXT,
  source_hash TEXT,
  page_refs_json TEXT NOT NULL DEFAULT '[]',
  asset_refs_json TEXT NOT NULL DEFAULT '[]',
  response_summary_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  started_at TEXT NOT NULL,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  CHECK (
    state IN ('blocked', 'manual_review', 'failed')
    OR homepage_ref IS NOT NULL
    OR source_pack_ref IS NOT NULL
  )
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_static_capture_campaign
  ON targeted_site_static_capture_runs(campaign_id, state, started_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_static_capture_prospect
  ON targeted_site_static_capture_runs(prospect_id, started_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_static_capture_domain
  ON targeted_site_static_capture_runs(normalized_domain, state, started_at DESC)
  WHERE archived_at IS NULL;
