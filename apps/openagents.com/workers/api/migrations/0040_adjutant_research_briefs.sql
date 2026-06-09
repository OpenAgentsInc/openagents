CREATE TABLE IF NOT EXISTS adjutant_research_briefs (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  enrichment_run_id TEXT REFERENCES exa_enrichment_runs(id) ON DELETE SET NULL,
  status TEXT NOT NULL CHECK (
    status IN ('draft', 'needs_review', 'approved', 'rejected', 'stale')
  ),
  summary TEXT NOT NULL CHECK (length(summary) > 0 AND length(summary) <= 1200),
  grounded_facts_json TEXT NOT NULL DEFAULT '[]' CHECK (length(grounded_facts_json) <= 4000),
  suggested_sections_json TEXT NOT NULL DEFAULT '[]' CHECK (length(suggested_sections_json) <= 2400),
  unknowns_json TEXT NOT NULL DEFAULT '[]' CHECK (length(unknowns_json) <= 2400),
  claims_needing_review_json TEXT NOT NULL DEFAULT '[]' CHECK (length(claims_needing_review_json) <= 2400),
  source_cards_json TEXT NOT NULL DEFAULT '[]' CHECK (length(source_cards_json) <= 5000),
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  review_reason TEXT CHECK (review_reason IS NULL OR length(review_reason) <= 500),
  approved_at TEXT,
  rejected_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS adjutant_research_briefs_assignment_updated_idx
  ON adjutant_research_briefs(assignment_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_research_briefs_assignment_status_idx
  ON adjutant_research_briefs(assignment_id, status, updated_at DESC)
  WHERE archived_at IS NULL;
