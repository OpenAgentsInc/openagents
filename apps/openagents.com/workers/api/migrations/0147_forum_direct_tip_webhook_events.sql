CREATE TABLE IF NOT EXISTS forum_direct_tip_webhook_events (
  id TEXT PRIMARY KEY,
  provider_event_ref TEXT NOT NULL UNIQUE,
  direct_tip_attempt_id TEXT NOT NULL REFERENCES forum_direct_tip_attempts(id) ON DELETE CASCADE,
  provider_ref TEXT NOT NULL,
  external_ref TEXT NOT NULL,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  payment_event_status TEXT NOT NULL CHECK (
    payment_event_status IN ('confirmed', 'failed', 'observed', 'refunded', 'replayed', 'reversed')
  ),
  redacted_evidence_ref TEXT NOT NULL,
  event_body_digest_ref TEXT NOT NULL,
  signature_binding_ref TEXT NOT NULL,
  reconciliation_status TEXT NOT NULL CHECK (
    reconciliation_status IN ('settled', 'failed', 'recovery_pending')
  ),
  reconciliation_result TEXT NOT NULL,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  delivery_count INTEGER NOT NULL DEFAULT 1 CHECK (delivery_count > 0),
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_direct_tip_webhook_events_attempt
  ON forum_direct_tip_webhook_events(direct_tip_attempt_id, first_seen_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_direct_tip_webhook_events_status
  ON forum_direct_tip_webhook_events(reconciliation_status, last_seen_at DESC)
  WHERE archived_at IS NULL;
