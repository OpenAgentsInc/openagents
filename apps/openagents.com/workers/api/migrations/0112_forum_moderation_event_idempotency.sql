ALTER TABLE forum_moderation_events
  ADD COLUMN idempotency_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_moderation_events_idempotency
  ON forum_moderation_events(idempotency_key)
  WHERE idempotency_key IS NOT NULL
    AND archived_at IS NULL;
