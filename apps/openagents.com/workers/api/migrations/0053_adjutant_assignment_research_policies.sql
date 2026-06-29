CREATE TABLE IF NOT EXISTS adjutant_assignment_research_policies (
  assignment_id TEXT PRIMARY KEY NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  policy_mode TEXT NOT NULL CHECK (
    policy_mode IN (
      'research_required',
      'research_optional',
      'research_not_applicable',
      'research_bypassed_by_operator'
    )
  ),
  reason TEXT NOT NULL CHECK (length(reason) > 0 AND length(reason) <= 1000),
  customer_safe_summary TEXT NOT NULL CHECK (length(customer_safe_summary) > 0 AND length(customer_safe_summary) <= 500),
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  source_authority_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS adjutant_assignment_research_policies_mode_updated_idx
  ON adjutant_assignment_research_policies(policy_mode, updated_at DESC)
  WHERE archived_at IS NULL;
