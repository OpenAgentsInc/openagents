CREATE TABLE IF NOT EXISTS omni_route_scorecards (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL REFERENCES omni_workrooms(id) ON DELETE CASCADE,
  work_kind TEXT NOT NULL CHECK (
    work_kind IN (
      'site',
      'coding',
      'adjustment',
      'existing_project_import',
      'business',
      'legal_sensitive'
    )
  ),
  selected_route_ref TEXT NOT NULL,
  selected_provider_ref TEXT NOT NULL,
  selected_account_ref TEXT,
  selected_model_ref TEXT NOT NULL,
  selected_runtime_ref TEXT NOT NULL,
  rejected_candidates_json TEXT NOT NULL DEFAULT '[]',
  decision_reason_refs_json TEXT NOT NULL DEFAULT '[]',
  observed_result_kind TEXT NOT NULL CHECK (
    observed_result_kind IN ('success', 'partial', 'failure', 'unavailable')
  ),
  observed_result_ref TEXT NOT NULL,
  post_closeout_score INTEGER CHECK (
    post_closeout_score IS NULL OR
    (post_closeout_score >= 0 AND post_closeout_score <= 100)
  ),
  cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (cost_cents >= 0),
  latency_ms INTEGER NOT NULL DEFAULT 0 CHECK (latency_ms >= 0),
  privacy_tier TEXT NOT NULL CHECK (
    privacy_tier IN ('public', 'customer', 'team', 'operator', 'private')
  ),
  trust_tier TEXT NOT NULL CHECK (
    trust_tier IN ('verified', 'reviewed', 'unverified', 'blocked')
  ),
  public_caveat_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_route_scorecards_workroom_updated
  ON omni_route_scorecards(workroom_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_route_scorecards_kind_result
  ON omni_route_scorecards(work_kind, observed_result_kind, updated_at DESC)
  WHERE archived_at IS NULL;
