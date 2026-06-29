CREATE TABLE IF NOT EXISTS targeted_site_remake_preview_generations (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  remake_brief_id TEXT NOT NULL
    REFERENCES targeted_site_remake_briefs(id) ON DELETE RESTRICT,
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
      'requested',
      'generating',
      'generated',
      'failed',
      'blocked',
      'archived'
    )
  ),
  preview_url TEXT,
  concept_slug TEXT NOT NULL,
  source_authority_pack_ref TEXT NOT NULL,
  generated_artifact_ref TEXT,
  generated_source_ref TEXT,
  candidate_site_project_ref TEXT,
  candidate_site_version_ref TEXT,
  generation_receipt_ref TEXT,
  failure_ref TEXT,
  legal_sensitive INTEGER NOT NULL CHECK (legal_sensitive IN (0, 1)),
  generation_constraints_json TEXT NOT NULL DEFAULT '{}',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  requested_at TEXT NOT NULL,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_preview_campaign
  ON targeted_site_remake_preview_generations(campaign_id, state, requested_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_preview_prospect
  ON targeted_site_remake_preview_generations(prospect_id, requested_at DESC)
  WHERE prospect_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_remake_preview_domain
  ON targeted_site_remake_preview_generations(normalized_domain, requested_at DESC)
  WHERE archived_at IS NULL;
