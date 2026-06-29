CREATE TABLE IF NOT EXISTS forum_tip_settlement_claims (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  receipt_id TEXT NOT NULL REFERENCES forum_receipts(id) ON DELETE CASCADE,
  receipt_ref TEXT NOT NULL,
  recipient_actor_ref TEXT NOT NULL,
  settlement_ref TEXT NOT NULL,
  settlement_evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  source_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (receipt_id)
);

CREATE INDEX IF NOT EXISTS idx_forum_tip_settlement_claims_receipt_ref
  ON forum_tip_settlement_claims (receipt_ref)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_forum_tip_settlement_claims_recipient_actor_ref
  ON forum_tip_settlement_claims (recipient_actor_ref, created_at)
  WHERE archived_at IS NULL;
