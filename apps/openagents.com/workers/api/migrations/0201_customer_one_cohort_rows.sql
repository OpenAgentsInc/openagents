CREATE TABLE IF NOT EXISTS customer_one_cohort_rows (
  team_cohort_ref TEXT PRIMARY KEY,
  state TEXT NOT NULL,
  candidate_ref TEXT,
  invite_ref TEXT,
  vertical_ref TEXT,
  template_ref TEXT,
  workspace_ref TEXT,
  routing_ref TEXT,
  run_ref TEXT,
  artifact_ref TEXT,
  review_ref TEXT,
  verification_ref TEXT,
  completion_bundle_ref TEXT,
  privacy_review_ref TEXT,
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_customer_one_cohort_rows_updated_at
  ON customer_one_cohort_rows (updated_at DESC, team_cohort_ref ASC);
