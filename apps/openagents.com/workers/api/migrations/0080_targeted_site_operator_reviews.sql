CREATE TABLE IF NOT EXISTS targeted_site_operator_review_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  campaign_id TEXT NOT NULL
    REFERENCES targeted_site_campaigns(id) ON DELETE CASCADE,
  prospect_id TEXT
    REFERENCES targeted_site_prospects(id) ON DELETE SET NULL,
  normalized_domain TEXT NOT NULL,
  remake_brief_id TEXT NOT NULL
    REFERENCES targeted_site_remake_briefs(id) ON DELETE RESTRICT,
  preview_generation_id TEXT NOT NULL
    REFERENCES targeted_site_remake_preview_generations(id) ON DELETE RESTRICT,
  decision TEXT NOT NULL CHECK (
    decision IN (
      'approve_preview',
      'reject_preview',
      'request_regeneration',
      'skip_target',
      'approve_outreach',
      'block_target',
      'archive'
    )
  ),
  previous_state TEXT NOT NULL,
  next_state TEXT NOT NULL CHECK (
    next_state IN (
      'preview_approved',
      'preview_rejected',
      'regeneration_requested',
      'target_skipped',
      'outreach_approved',
      'target_blocked',
      'archived'
    )
  ),
  operator_actor_user_id TEXT NOT NULL,
  operator_note_ref TEXT,
  outreach_draft_ref TEXT,
  meeting_cta_ref TEXT,
  suppression_state TEXT NOT NULL CHECK (
    suppression_state IN (
      'unknown',
      'clear',
      'suppressed',
      'manual_review'
    )
  ),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  decided_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_targeted_site_operator_review_campaign
  ON targeted_site_operator_review_events(campaign_id, decided_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_operator_review_preview
  ON targeted_site_operator_review_events(preview_generation_id, decided_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_targeted_site_operator_review_domain
  ON targeted_site_operator_review_events(normalized_domain, decided_at DESC)
  WHERE archived_at IS NULL;
