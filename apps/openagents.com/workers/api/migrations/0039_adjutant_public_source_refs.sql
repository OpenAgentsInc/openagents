CREATE TABLE IF NOT EXISTS adjutant_public_source_refs (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (
    kind IN (
      'github_repository',
      'github_profile',
      'personal_site',
      'linkedin_profile',
      'x_profile',
      'generic_url'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN (
      'proposed',
      'approved',
      'rejected',
      'internal_only',
      'public_safe'
    )
  ),
  url TEXT NOT NULL CHECK (length(url) > 0 AND length(url) <= 2048),
  normalized_domain TEXT NOT NULL CHECK (length(normalized_domain) > 0 AND length(normalized_domain) <= 255),
  label TEXT CHECK (label IS NULL OR length(label) <= 240),
  public_safe INTEGER NOT NULL DEFAULT 0 CHECK (public_safe IN (0, 1)),
  proposed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  reviewed_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  review_reason TEXT CHECK (review_reason IS NULL OR length(review_reason) <= 500),
  approved_at TEXT,
  rejected_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS adjutant_public_source_refs_assignment_created_idx
  ON adjutant_public_source_refs(assignment_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_public_source_refs_order_created_idx
  ON adjutant_public_source_refs(software_order_id, created_at DESC)
  WHERE software_order_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS adjutant_public_source_refs_status_updated_idx
  ON adjutant_public_source_refs(status, updated_at DESC)
  WHERE archived_at IS NULL;
