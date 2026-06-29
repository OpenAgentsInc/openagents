CREATE TABLE IF NOT EXISTS order_fulfillment_artifacts (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
  kind TEXT NOT NULL CHECK (
    kind IN (
      'pull_request',
      'branch',
      'commit',
      'diff',
      'preview',
      'notes',
      'attachment'
    )
  ),
  title TEXT NOT NULL CHECK (length(title) > 0 AND length(title) <= 240),
  summary TEXT NOT NULL CHECK (length(summary) > 0 AND length(summary) <= 1200),
  url TEXT,
  repository_full_name TEXT,
  source_branch TEXT,
  target_branch TEXT,
  commit_sha TEXT,
  status TEXT NOT NULL CHECK (
    status IN (
      'draft',
      'customer_review_ready',
      'customer_accepted',
      'superseded',
      'rejected'
    )
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS order_fulfillment_artifacts_order_created_idx
  ON order_fulfillment_artifacts(software_order_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS order_fulfillment_artifacts_order_status_idx
  ON order_fulfillment_artifacts(software_order_id, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS order_fulfillment_feedback (
  id TEXT PRIMARY KEY NOT NULL,
  software_order_id TEXT NOT NULL REFERENCES software_orders(id) ON DELETE CASCADE,
  artifact_id TEXT REFERENCES order_fulfillment_artifacts(id) ON DELETE SET NULL,
  author_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  body TEXT NOT NULL CHECK (length(body) > 0 AND length(body) <= 4000),
  status TEXT NOT NULL CHECK (
    status IN (
      'submitted',
      'queued',
      'running',
      'addressed',
      'closed',
      'rejected'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'customer_order_ui',
      'operator',
      'agent'
    )
  ),
  visibility TEXT NOT NULL CHECK (visibility IN ('private', 'team', 'public')),
  adjutant_assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL,
  adjutant_adjustment_id TEXT REFERENCES adjutant_adjustment_requests(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS order_fulfillment_feedback_order_created_idx
  ON order_fulfillment_feedback(software_order_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS order_fulfillment_feedback_artifact_created_idx
  ON order_fulfillment_feedback(artifact_id, created_at DESC)
  WHERE artifact_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS order_fulfillment_feedback_status_updated_idx
  ON order_fulfillment_feedback(status, updated_at DESC)
  WHERE archived_at IS NULL;
