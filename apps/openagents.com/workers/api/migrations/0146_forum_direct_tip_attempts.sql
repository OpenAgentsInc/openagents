CREATE TABLE IF NOT EXISTS forum_direct_tip_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  payer_actor_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  target_topic_id TEXT NOT NULL REFERENCES forum_topics(id) ON DELETE CASCADE,
  target_post_id TEXT NOT NULL REFERENCES forum_posts(id) ON DELETE CASCADE,
  target_post_permalink TEXT,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  redacted_evidence_ref TEXT NOT NULL,
  payment_mode TEXT NOT NULL CHECK (payment_mode IN ('live', 'sandbox', 'signet', 'unknown')),
  payment_event_status TEXT NOT NULL CHECK (
    payment_event_status IN (
      'confirmed',
      'failed',
      'observed',
      'refunded',
      'replayed',
      'reversed'
    )
  ),
  status TEXT NOT NULL CHECK (status IN ('settled', 'failed', 'recovery_pending')),
  receipt_ref TEXT REFERENCES forum_receipts(receipt_ref) ON DELETE SET NULL,
  payment_event_id TEXT REFERENCES forum_payment_events(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_ref)
);

CREATE INDEX IF NOT EXISTS idx_forum_direct_tip_attempts_target
  ON forum_direct_tip_attempts(target_post_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_direct_tip_attempts_recipient
  ON forum_direct_tip_attempts(recipient_actor_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_direct_tip_attempts_status
  ON forum_direct_tip_attempts(status, updated_at DESC)
  WHERE archived_at IS NULL;
