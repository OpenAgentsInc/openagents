CREATE TABLE IF NOT EXISTS forum_post_revisions (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  post_id TEXT NOT NULL REFERENCES forum_posts(id) ON DELETE CASCADE,
  actor_ref TEXT NOT NULL,
  action_kind TEXT NOT NULL CHECK (action_kind IN ('edit', 'tombstone')),
  previous_body_text TEXT,
  next_body_text TEXT,
  previous_state TEXT NOT NULL CHECK (
    previous_state IN ('visible', 'edited', 'tombstoned', 'held_for_review', 'hidden')
  ),
  next_state TEXT NOT NULL CHECK (
    next_state IN ('visible', 'edited', 'tombstoned', 'held_for_review', 'hidden')
  ),
  reason_ref TEXT,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_post_revisions_post_created
  ON forum_post_revisions(post_id, created_at DESC)
  WHERE archived_at IS NULL;
