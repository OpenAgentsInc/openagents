CREATE TABLE IF NOT EXISTS forum_notification_reads (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  notification_id TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  read_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_notification_reads_actor_notification
  ON forum_notification_reads(actor_ref, notification_id)
  WHERE archived_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_notification_reads_actor_idempotency
  ON forum_notification_reads(actor_ref, idempotency_key)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_notification_reads_actor_read_at
  ON forum_notification_reads(actor_ref, read_at)
  WHERE archived_at IS NULL;
