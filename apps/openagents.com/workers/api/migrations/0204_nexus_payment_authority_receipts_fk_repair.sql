-- openagents #5232 follow-up to 0203. When 0203 renamed the parent payout
-- tables to `*_old` before recreating them, SQLite's default (non-legacy)
-- ALTER TABLE RENAME rewrote the foreign-key references inside the dependent
-- `nexus_payment_authority_receipts` table to point at the now-dropped `*_old`
-- tables. That leaves the receipts table with dangling FK targets, which would
-- break the final `settlement_recorded` receipt insert.
--
-- Rebuild `nexus_payment_authority_receipts` with its original (correct) FK
-- targets. It is a leaf table (nothing references it), so the rebuild is safe.
-- FK enforcement is disabled for the rebuild; all rows are copied verbatim.

PRAGMA foreign_keys = OFF;

ALTER TABLE nexus_payment_authority_receipts
  RENAME TO nexus_payment_authority_receipts_old;

CREATE TABLE nexus_payment_authority_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  payout_attempt_ref TEXT,
  event_ref TEXT,
  receipt_kind TEXT NOT NULL CHECK (
    receipt_kind IN (
      'attempt_recorded',
      'confirmation_recorded',
      'dispatch_recorded',
      'intent_created',
      'pause_recorded',
      'policy_rejected',
      'settlement_recorded',
      'verification_recorded'
    )
  ),
  audience TEXT NOT NULL CHECK (
    audience IN ('agent', 'customer', 'operator', 'public')
  ),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (payout_intent_ref)
    REFERENCES nexus_treasury_payout_intents(payout_intent_ref),
  FOREIGN KEY (payout_attempt_ref)
    REFERENCES nexus_treasury_payout_attempts(payout_attempt_ref),
  FOREIGN KEY (event_ref)
    REFERENCES nexus_treasury_payout_reconciliation_events(event_ref)
);

INSERT INTO nexus_payment_authority_receipts (
  id, receipt_ref, payout_intent_ref, payout_attempt_ref, event_ref,
  receipt_kind, audience, metadata_refs_json, public_projection_json,
  created_at, archived_at
)
SELECT
  id, receipt_ref, payout_intent_ref, payout_attempt_ref, event_ref,
  receipt_kind, audience, metadata_refs_json, public_projection_json,
  created_at, archived_at
FROM nexus_payment_authority_receipts_old;

DROP TABLE nexus_payment_authority_receipts_old;

CREATE INDEX IF NOT EXISTS nexus_payment_authority_receipts_intent_idx
  ON nexus_payment_authority_receipts(payout_intent_ref, created_at DESC)
  WHERE archived_at IS NULL;
