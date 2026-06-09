CREATE TABLE IF NOT EXISTS targeted_site_quality_audits (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  static_capture_run_id TEXT
    REFERENCES targeted_site_static_capture_runs(id) ON DELETE SET NULL,
  rendered_capture_run_id TEXT
    REFERENCES targeted_site_rendered_capture_runs(id) ON DELETE SET NULL,
  provider_adapter_run_id TEXT
    REFERENCES targeted_site_capture_provider_adapter_runs(id) ON DELETE SET NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'draft',
      'ready',
      'manual_review',
      'blocked',
      'archived'
    )
  ),
  recommendation TEXT NOT NULL CHECK (
    recommendation IN (
      'skip',
      'monitor',
      'remake_candidate',
      'manual_review',
      'blocked'
    )
  ),
  overall_score NUMERIC NOT NULL CHECK (
    overall_score >= 0 AND overall_score <= 100
  ),
  legal_sensitive INTEGER NOT NULL CHECK (legal_sensitive IN (0, 1)),
  dimensions_json TEXT NOT NULL DEFAULT '{}',
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  audited_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_quality_audits_campaign
  ON targeted_site_quality_audits(campaign_id, recommendation, audited_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_quality_audits_prospect
  ON targeted_site_quality_audits(prospect_id, audited_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_quality_audits_domain
  ON targeted_site_quality_audits(normalized_domain, audited_at DESC)
  WHERE archived_at IS NULL;
