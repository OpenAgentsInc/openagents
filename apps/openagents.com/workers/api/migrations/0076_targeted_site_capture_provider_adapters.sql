CREATE TABLE IF NOT EXISTS targeted_site_capture_provider_adapter_runs (
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
  rendered_capture_run_id TEXT
    REFERENCES targeted_site_rendered_capture_runs(id) ON DELETE SET NULL,
  provider_kind TEXT NOT NULL CHECK (
    provider_kind IN (
      'first_party_worker',
      'browser_run',
      'firecrawl',
      'browserless',
      'browserbase',
      'apify',
      'container'
    )
  ),
  state TEXT NOT NULL CHECK (
    state IN (
      'requested',
      'approved_fallback',
      'benchmark',
      'denied',
      'failed',
      'partial',
      'succeeded',
      'manual_review',
      'archived'
    )
  ),
  reason TEXT NOT NULL CHECK (
    reason IN (
      'first_party_default',
      'static_insufficient',
      'rendered_insufficient',
      'paid_escalation_approved',
      'benchmark_quality_check',
      'cost_not_approved',
      'provider_unavailable',
      'provider_error',
      'manual_review',
      'policy_not_fetchable',
      'bot_protection_or_login'
    )
  ),
  paid_escalation_ref TEXT,
  provider_request_ref TEXT,
  provider_receipt_ref TEXT,
  output_pack_ref TEXT,
  usage_ref TEXT,
  cost_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  requested_at TEXT NOT NULL,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_provider_campaign
  ON targeted_site_capture_provider_adapter_runs(
    campaign_id,
    provider_kind,
    state,
    requested_at DESC
  )
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_provider_prospect
  ON targeted_site_capture_provider_adapter_runs(prospect_id, requested_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_capture_provider_domain
  ON targeted_site_capture_provider_adapter_runs(
    normalized_domain,
    provider_kind,
    requested_at DESC
  )
  WHERE archived_at IS NULL;
