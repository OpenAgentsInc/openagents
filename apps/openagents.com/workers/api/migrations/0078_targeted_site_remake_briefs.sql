CREATE TABLE IF NOT EXISTS targeted_site_remake_briefs (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  quality_audit_id TEXT NOT NULL
    REFERENCES targeted_site_quality_audits(id) ON DELETE RESTRICT,
  static_capture_run_id TEXT
    REFERENCES targeted_site_static_capture_runs(id) ON DELETE SET NULL,
  rendered_capture_run_id TEXT
    REFERENCES targeted_site_rendered_capture_runs(id) ON DELETE SET NULL,
  provider_adapter_run_id TEXT
    REFERENCES targeted_site_capture_provider_adapter_runs(id) ON DELETE SET NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'draft',
      'ready_for_operator_review',
      'approved_for_generation',
      'rejected',
      'blocked',
      'archived'
    )
  ),
  legal_sensitive INTEGER NOT NULL CHECK (legal_sensitive IN (0, 1)),
  source_authority_pack_json TEXT NOT NULL DEFAULT '{}',
  audit_finding_refs_json TEXT NOT NULL DEFAULT '[]',
  original_screenshot_refs_json TEXT NOT NULL DEFAULT '[]',
  copied_text_refs_json TEXT NOT NULL DEFAULT '[]',
  copied_image_refs_json TEXT NOT NULL DEFAULT '[]',
  generation_constraints_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  prepared_at TEXT NOT NULL,
  reviewed_at TEXT,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_briefs_campaign
  ON targeted_site_remake_briefs(campaign_id, state, prepared_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_briefs_prospect
  ON targeted_site_remake_briefs(prospect_id, prepared_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_briefs_domain
  ON targeted_site_remake_briefs(normalized_domain, prepared_at DESC)
  WHERE archived_at IS NULL;
