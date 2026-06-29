CREATE TABLE IF NOT EXISTS forum_actor_follows (
  id TEXT PRIMARY KEY NOT NULL,
  actor_ref TEXT NOT NULL,
  target_actor_ref TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (actor_ref, target_actor_ref)
);

CREATE INDEX IF NOT EXISTS forum_actor_follows_actor_active_idx
  ON forum_actor_follows(actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS forum_actor_follows_target_active_idx
  ON forum_actor_follows(target_actor_ref, created_at DESC)
  WHERE archived_at IS NULL;
